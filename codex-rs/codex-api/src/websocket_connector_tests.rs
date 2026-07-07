use std::sync::Arc;

use pretty_assertions::assert_eq;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use super::*;

#[tokio::test]
async fn proxy_route_establishes_connect_tunnel_before_websocket_handshake() {
    let target_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("target listener should bind");
    let target_addr = target_listener
        .local_addr()
        .expect("target listener should have an address");
    let target_task = tokio::spawn(async move {
        let (stream, _) = target_listener
            .accept()
            .await
            .expect("target should accept");
        let mut websocket = accept_async(stream)
            .await
            .expect("target websocket handshake should succeed");
        let _ = websocket.close(None).await;
    });

    let proxy_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("proxy listener should bind");
    let proxy_addr = proxy_listener
        .local_addr()
        .expect("proxy listener should have an address");
    let connect_request = Arc::new(Mutex::new(None));
    let proxy_connect_request = Arc::clone(&connect_request);
    let proxy_task = tokio::spawn(async move {
        let (mut client, _) = proxy_listener.accept().await.expect("proxy should accept");
        let mut request = Vec::new();
        let mut byte = [0_u8; 1];
        while !request.ends_with(b"\r\n\r\n") {
            client
                .read_exact(&mut byte)
                .await
                .expect("proxy should read CONNECT request");
            request.push(byte[0]);
        }
        *proxy_connect_request.lock().await =
            Some(String::from_utf8(request).expect("CONNECT request should contain valid UTF-8"));

        let mut target = tokio::net::TcpStream::connect(target_addr)
            .await
            .expect("proxy should connect to target");
        client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await
            .expect("proxy should acknowledge CONNECT");
        let _ = tokio::io::copy_bidirectional(&mut client, &mut target).await;
    });

    let request = format!("ws://{target_addr}/v1/responses")
        .into_client_request()
        .expect("websocket request should build");
    let (mut websocket, _) = connect(
        request,
        /*config*/ None,
        /*connector*/ None,
        OutboundProxyRoute::Proxy {
            url: format!("http://{proxy_addr}"),
        },
    )
    .await
    .expect("proxied websocket handshake should succeed");
    let _ = websocket.close(None).await;
    drop(websocket);

    target_task.await.expect("target task should finish");
    proxy_task.await.expect("proxy task should finish");
    let request = connect_request
        .lock()
        .await
        .clone()
        .expect("proxy should record CONNECT request");
    let expected_request_line = format!("CONNECT {target_addr} HTTP/1.1");
    assert_eq!(request.lines().next(), Some(expected_request_line.as_str()));
}
