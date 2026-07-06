use super::BindConnectionAttribution;
use super::write_attribution_frame;
use crate::config::NetworkProxySettings;
use crate::runtime::network_proxy_state_for_policy;
use crate::state::NetworkProxyState;
use pretty_assertions::assert_eq;
use rama_core::Service;
use rama_core::error::BoxError;
use rama_core::extensions::ExtensionsRef;
use rama_core::service::service_fn;
use rama_tcp::TcpStream as RamaTcpStream;
use std::io;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

#[test]
fn attribution_frame_has_bounded_binary_prefix() -> io::Result<()> {
    let mut frame = Vec::new();
    write_attribution_frame(&mut frame, "token-1")?;

    assert_eq!(&frame[..8], b"\0CDXPXY1");
    assert_eq!(u16::from_be_bytes([frame[8], frame[9]]), 7);
    assert_eq!(&frame[10..], b"token-1");
    Ok(())
}

#[tokio::test]
async fn framed_connection_receives_registered_execution_state() -> Result<(), BoxError> {
    let state = Arc::new(network_proxy_state_for_policy(
        NetworkProxySettings::default(),
    ));
    state.register_execution("token-1", "local", "execution-1");

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let client = tokio::spawn(async move {
        let mut stream = TcpStream::connect(addr).await?;
        let mut frame = Vec::new();
        write_attribution_frame(&mut frame, "token-1")?;
        stream.write_all(&frame).await
    });

    let (stream, _) = listener.accept().await?;
    let service = BindConnectionAttribution::new(
        service_fn(|stream: RamaTcpStream| async move {
            let state = stream.extensions().get::<Arc<NetworkProxyState>>().cloned();
            Ok::<_, io::Error>(state)
        }),
        state,
        Some("local".to_string()),
    );
    let actual = service
        .serve(RamaTcpStream::new(stream))
        .await?
        .expect("connection state");
    client.await??;

    assert_eq!(actual.environment_id(), Some("local"));
    assert_eq!(actual.execution_id().as_deref(), Some("execution-1"));
    Ok(())
}
