//! Dedicated managed proxy routing regression coverage.
//! Each proxy must use distinct loopback listeners while preserving HTTP and SOCKS policy.

use super::*;
use crate::config::NetworkProxyConfig;
use crate::state::network_proxy_state_for_policy;
use pretty_assertions::assert_eq;
use std::net::Ipv4Addr;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

#[tokio::test]
async fn dedicated_listeners_preserve_http_and_socks_policy_without_restricted_tokens() -> Result<()>
{
    let origin = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let origin_port = origin.local_addr()?.port();
    let origin_task = tokio::spawn(async move {
        while let Ok((mut stream, _)) = origin.accept().await {
            tokio::spawn(async move {
                let mut request = [0; 1024];
                let _ = stream.read(&mut request).await;
                let _ = stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .await;
            });
        }
    });
    let mut handles = Vec::new();
    let mut addresses = Vec::new();
    for allowed in [true, false] {
        let mut config = NetworkProxyConfig {
            enabled: true,
            mode: crate::NetworkMode::Full,
            ..NetworkProxyConfig::default()
        };
        config.set_allowed_domains(vec![if allowed {
            "127.0.0.1".to_string()
        } else {
            "unreachable.invalid".to_string()
        }]);
        let proxy = NetworkProxy::builder()
            .state(Arc::new(network_proxy_state_for_policy(config)))
            .managed_proxy_routing(ManagedProxyRouting::DedicatedListeners)
            .build()
            .await?;
        handles.push(proxy.run().await?);
        let proxy = proxy.for_execution(
            "environment",
            "execution",
            "token".to_string(),
            /*environment_policy*/ None,
            /*fallback_policy_decider*/ None,
        )?;
        let prepared =
            proxy.prepare_for_optional_environment(HashMap::new(), /*environment_id*/ None)?;
        let http = prepared.env["HTTP_PROXY"]
            .trim_start_matches("http://")
            .parse::<SocketAddr>()?;
        let socks = prepared.env["ALL_PROXY"]
            .trim_start_matches("socks5h://")
            .parse::<SocketAddr>()?;
        let mut ports = vec![http.port(), socks.port()];
        ports.sort_unstable();
        assert_eq!(
            prepared.sandbox_context,
            ManagedNetworkSandboxContext {
                loopback_ports: ports,
                allow_local_binding: false,
                ..ManagedNetworkSandboxContext::default()
            }
        );
        assert_eq!(
            (http.ip(), socks.ip()),
            (Ipv4Addr::LOCALHOST.into(), Ipv4Addr::LOCALHOST.into())
        );
        for addr in [http, socks] {
            assert!(!addresses.contains(&addr));
            addresses.push(addr);
        }
        #[cfg(target_os = "windows")]
        {
            assert_eq!(
                proxy.network_proxy_restricting_sid(/*environment_id*/ None),
                None
            );
            assert!(
                !prepared
                    .env
                    .contains_key(WINDOWS_SANDBOX_PROXY_PORTS_ENV_KEY)
            );
        }
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut stream = TcpStream::connect(http).await?;
            stream.write_all(format!(
                "GET http://127.0.0.1:{origin_port}/ HTTP/1.1\r\nHost: 127.0.0.1:{origin_port}\r\nConnection: close\r\n\r\n"
            ).as_bytes()).await?;
            let mut response = Vec::new();
            stream.read_to_end(&mut response).await?;
            let status = if allowed { "200" } else { "403" };
            assert_eq!(String::from_utf8(response)?.split_whitespace().nth(1), Some(status));

            let mut stream = TcpStream::connect(socks).await?;
            stream.write_all(&[5, 1, 0]).await?;
            let mut greeting = [0; 2];
            stream.read_exact(&mut greeting).await?;
            assert_eq!(greeting, [5, 0]);
            let mut connect = vec![5, 1, 0, 1, 127, 0, 0, 1];
            connect.extend_from_slice(&origin_port.to_be_bytes());
            stream.write_all(&connect).await?;
            let mut reply = [0; 4];
            stream.read_exact(&mut reply).await?;
            assert_eq!(reply[1] == 0, allowed);
            Ok::<(), anyhow::Error>(())
        }).await??;
    }
    for handle in handles {
        handle.shutdown().await?;
    }
    origin_task.abort();
    Ok(())
}
