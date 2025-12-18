use crate::config::NetworkMode;
use crate::policy::normalize_host;
use crate::state::AppState;
use crate::state::BlockedRequest;
use anyhow::Result;
use anyhow::anyhow;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::copy_bidirectional;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tracing::error;
use tracing::info;
use tracing::warn;

pub async fn run_socks5(state: Arc<AppState>, addr: SocketAddr) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!(addr = %addr, "SOCKS5 proxy listening");
    match state.network_mode().await {
        Ok(NetworkMode::Limited) => {
            info!(
                mode = "limited",
                "SOCKS5 is blocked in limited mode; set mode=\"full\" to allow SOCKS5"
            );
        }
        Ok(NetworkMode::Full) => {}
        Err(err) => {
            warn!(error = %err, "failed to read network mode");
        }
    }
    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_socks5_client(stream, peer_addr, state).await {
                warn!(error = %err, "SOCKS5 session ended with error");
            }
        });
    }
}

async fn handle_socks5_client(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    state: Arc<AppState>,
) -> Result<()> {
    let mut header = [0u8; 2];
    stream.read_exact(&mut header).await?;
    if header[0] != 0x05 {
        return Err(anyhow!("invalid SOCKS version"));
    }
    let nmethods = header[1] as usize;
    let mut methods = vec![0u8; nmethods];
    stream.read_exact(&mut methods).await?;
    stream.write_all(&[0x05, 0x00]).await?;

    let mut req_header = [0u8; 4];
    stream.read_exact(&mut req_header).await?;
    if req_header[0] != 0x05 {
        return Err(anyhow!("invalid SOCKS request version"));
    }
    let cmd = req_header[1];
    if cmd != 0x01 {
        stream
            .write_all(&[0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
            .await?;
        return Err(anyhow!("unsupported SOCKS command"));
    }
    let atyp = req_header[3];
    let host = match atyp {
        0x01 => {
            let mut addr = [0u8; 4];
            stream.read_exact(&mut addr).await?;
            format!("{}.{}.{}.{}", addr[0], addr[1], addr[2], addr[3])
        }
        0x03 => {
            let mut len_buf = [0u8; 1];
            stream.read_exact(&mut len_buf).await?;
            let len = len_buf[0] as usize;
            let mut domain = vec![0u8; len];
            stream.read_exact(&mut domain).await?;
            String::from_utf8_lossy(&domain).to_string()
        }
        0x04 => {
            stream
                .write_all(&[0x05, 0x08, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;
            return Err(anyhow!("ipv6 not supported"));
        }
        _ => {
            stream
                .write_all(&[0x05, 0x08, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;
            return Err(anyhow!("unknown address type"));
        }
    };

    let mut port_buf = [0u8; 2];
    stream.read_exact(&mut port_buf).await?;
    let port = u16::from_be_bytes(port_buf);
    let normalized_host = normalize_host(&host);

    match state.network_mode().await {
        Ok(NetworkMode::Limited) => {
            let _ = state
                .record_blocked(BlockedRequest::new(
                    normalized_host.clone(),
                    "method_not_allowed".to_string(),
                    Some(peer_addr.to_string()),
                    None,
                    Some(NetworkMode::Limited),
                    "socks5".to_string(),
                ))
                .await;
            warn!(
                client = %peer_addr,
                host = %normalized_host,
                mode = "limited",
                allowed_methods = "GET, HEAD, OPTIONS",
                "SOCKS blocked by method policy"
            );
            stream
                .write_all(&[0x05, 0x02, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;
            return Ok(());
        }
        Ok(NetworkMode::Full) => {}
        Err(err) => {
            error!(error = %err, "failed to evaluate method policy");
            stream
                .write_all(&[0x05, 0x01, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;
            return Ok(());
        }
    }

    match state.host_blocked(&normalized_host).await {
        Ok((true, reason)) => {
            let _ = state
                .record_blocked(BlockedRequest::new(
                    normalized_host.clone(),
                    reason.clone(),
                    Some(peer_addr.to_string()),
                    None,
                    None,
                    "socks5".to_string(),
                ))
                .await;
            warn!(client = %peer_addr, host = %normalized_host, reason = %reason, "SOCKS blocked");
            stream
                .write_all(&[0x05, 0x02, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;
            return Ok(());
        }
        Ok((false, _)) => {
            info!(
                client = %peer_addr,
                host = %normalized_host,
                port = port,
                "SOCKS allowed"
            );
        }
        Err(err) => {
            error!(error = %err, "failed to evaluate host");
            stream
                .write_all(&[0x05, 0x01, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;
            return Ok(());
        }
    }

    let target = format!("{host}:{port}");
    let mut upstream = match TcpStream::connect(&target).await {
        Ok(stream) => stream,
        Err(err) => {
            warn!(error = %err, "SOCKS connect failed");
            stream
                .write_all(&[0x05, 0x04, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;
            return Ok(());
        }
    };

    stream
        .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await?;

    let _ = copy_bidirectional(&mut stream, &mut upstream).await;
    Ok(())
}
