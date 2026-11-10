#![cfg(test)]

use crate::NetworkMode;
use crate::NetworkProxyConfig;
use crate::socks5::socks5_proxy_service;
use crate::state::network_proxy_state_for_policy;
use pretty_assertions::assert_eq;
use rama_core::Service;
use std::io::ErrorKind;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

#[tokio::test]
async fn socks_tcp_waits_for_http_connect_and_preserves_the_tunnel() {
    let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let ingress = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut config = NetworkProxyConfig {
        mode: NetworkMode::Full,
        allow_local_binding: Some(true),
        allow_upstream_proxy: true,
        ..Default::default()
    };
    config.set_allowed_domains(vec!["db.invalid".into()]);
    let mut state = network_proxy_state_for_policy(config);
    state.set_tcp_upstream_proxy(upstream.local_addr().unwrap().to_string().parse().unwrap());
    let service = socks5_proxy_service(
        Arc::new(state),
        /*policy_decider*/ None,
        /*environment_id*/ None,
        /*enable_socks5_udp*/ false,
    );

    for success in [false, true] {
        let exercise = async {
            let mut client = TcpStream::connect(ingress.local_addr().unwrap())
                .await
                .unwrap();
            let (socket, _) = ingress.accept().await.unwrap();
            let peer = async {
                let (stream, _) = upstream.accept().await.unwrap();
                let mut proxy = BufReader::new(stream);
                let mut line = String::new();
                proxy.read_line(&mut line).await.unwrap();
                assert_eq!(line, "CONNECT db.invalid:5432 HTTP/1.1\r\n");
                loop {
                    line.clear();
                    assert_ne!(proxy.read_line(&mut line).await.unwrap(), 0);
                    if line == "\r\n" {
                        break;
                    }
                }
                assert!(proxy.buffer().is_empty());
                // Exercise read-ahead and a non-200 success; no payload may escape on refusal.
                let response = if success {
                    b"HTTP/1.1 201 Created\r\n\r\nhello".as_slice()
                } else {
                    b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n"
                };
                proxy.get_mut().write_all(response).await.unwrap();
                let mut received = Vec::new();
                proxy.read_to_end(&mut received).await.unwrap();
                assert_eq!(received, if success { b"private".as_slice() } else { &[] });
            };
            let caller = async {
                client.write_all(&[5, 1, 0]).await.unwrap();
                let mut greeting = [0; 2];
                client.read_exact(&mut greeting).await.unwrap();
                assert_eq!(greeting, [5, 0]);
                // Pipeline the payload with CONNECT, before the upstream has replied.
                client
                    .write_all(b"\x05\x01\x00\x03\x0adb.invalid\x15\x38private")
                    .await
                    .unwrap();
                let mut reply = [0; 10];
                match client.read_exact(&mut reply).await {
                    Ok(_) => assert_eq!(reply[1] == 0, success),
                    // Closing a socket with unread pipelined data can reset it on Windows.
                    Err(err) if !success => assert_eq!(err.kind(), ErrorKind::ConnectionReset),
                    Err(err) => panic!("missing SOCKS reply: {err}"),
                }
                if success {
                    client.shutdown().await.unwrap();
                    let mut bytes = Vec::new();
                    client.read_to_end(&mut bytes).await.unwrap();
                    assert_eq!(bytes, b"hello");
                }
            };
            let (result, (), ()) = tokio::join!(service.serve(socket.into()), peer, caller);
            assert_eq!(result.is_ok(), success);
        };
        tokio::time::timeout(Duration::from_secs(5), exercise)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn remote_dns_requires_policy_independent_of_the_resolved_ip() {
    for (allow_local, deny, allow_remote_dns) in [
        (false, "denied.example", false),
        (true, "denied.example", true),
        (true, "0177.0.0.1", false),
        (true, "10.*", false),
        (true, "*.de", true),
        (true, "*.10", false),
        (true, "[::ffff:192.0.2.1]", false),
    ] {
        let mut config = NetworkProxyConfig {
            allow_local_binding: Some(allow_local),
            ..Default::default()
        };
        config.set_denied_domains(vec![deny.into()]);
        let state = network_proxy_state_for_policy(config);
        assert_eq!(
            state.permits_remote_tcp_dns().await.unwrap(),
            allow_remote_dns,
            "{deny}"
        );
    }
}
