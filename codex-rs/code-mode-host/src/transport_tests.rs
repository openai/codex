use std::net::SocketAddr;

use axum::serve::Listener;
use pretty_assertions::assert_eq;
use tokio::net::TcpStream;

use super::BulkConnectionRegistry;
use super::ListenTransport;
use super::MAX_PENDING_BULK_CONNECTIONS;
use super::bind_websocket_listener;
use super::parse_listen_url;

#[tokio::test]
async fn websocket_listener_disables_nagle() {
    let bind_address = "127.0.0.1:0"
        .parse()
        .expect("websocket test listener should have a valid bind address");
    let mut listener = bind_websocket_listener(bind_address)
        .await
        .expect("websocket test listener should bind");
    let local_addr = listener
        .local_addr()
        .expect("websocket test listener should have a local address");
    let _client = TcpStream::connect(local_addr)
        .await
        .expect("websocket test client should connect");
    let (stream, _) = listener.accept().await;

    let stream = stream
        .nodelay()
        .expect("accepted websocket socket should expose TCP_NODELAY");

    assert!(stream);
}

#[test]
fn bulk_connection_registration_cleans_up_when_dropped() {
    let registry = BulkConnectionRegistry::default();
    let registration = registry.reserve().expect("bulk connection registration");
    let token = registration.token();

    drop(registration);

    assert!(registry.remove(token).is_none());
}

#[test]
fn bulk_connection_registrations_are_bounded_and_released() {
    let registry = BulkConnectionRegistry::default();
    let mut registrations = (0..MAX_PENDING_BULK_CONNECTIONS)
        .map(|_| registry.reserve().expect("bulk connection registration"))
        .collect::<Vec<_>>();

    assert!(registry.reserve().is_none());
    drop(registrations.pop());
    assert!(registry.reserve().is_some());
}

#[test]
fn parse_listen_url_accepts_stdio_transports() {
    assert_eq!(
        parse_listen_url("stdio").expect("stdio listen URL should parse"),
        ListenTransport::Stdio
    );
    assert_eq!(
        parse_listen_url("stdio://").expect("stdio URL should parse"),
        ListenTransport::Stdio
    );
}

#[test]
fn parse_listen_url_accepts_websocket_addresses() {
    assert_eq!(
        parse_listen_url("ws://127.0.0.1:0").expect("websocket listen URL should parse"),
        ListenTransport::WebSocket(
            "127.0.0.1:0"
                .parse::<SocketAddr>()
                .expect("valid socket address")
        )
    );
    assert_eq!(
        parse_listen_url("ws://[::1]:9000").expect("IPv6 websocket listen URL should parse"),
        ListenTransport::WebSocket(
            "[::1]:9000"
                .parse::<SocketAddr>()
                .expect("valid IPv6 socket address")
        )
    );
}

#[test]
fn parse_listen_url_rejects_invalid_transports() {
    let invalid_address = parse_listen_url("ws://localhost:9000")
        .expect_err("websocket listener requires an IP address");
    assert!(
        invalid_address
            .to_string()
            .contains("expected `ws://IP:PORT`")
    );

    let unsupported =
        parse_listen_url("http://127.0.0.1:9000").expect_err("HTTP is not a listen transport");
    assert!(unsupported.to_string().contains("unsupported --listen URL"));
}
