use futures::StreamExt;
use pretty_assertions::assert_eq;
use tokio::net::TcpStream;

use super::ListenTransport;
use super::parse_listen_url;
use crate::grpc_transport::bind_tcp_listener;

#[tokio::test]
async fn grpc_listener_disables_nagle() {
    let bind_address = "127.0.0.1:0"
        .parse()
        .expect("gRPC test listener should have a valid bind address");
    let mut listener = bind_tcp_listener(bind_address)
        .await
        .expect("gRPC test listener should bind");
    let local_addr = listener
        .local_addr()
        .expect("gRPC test listener should have a local address");
    let _client = TcpStream::connect(local_addr)
        .await
        .expect("gRPC test client should connect");
    let nodelay = listener
        .next()
        .await
        .expect("gRPC test listener should accept a connection")
        .expect("gRPC test listener should return a valid socket")
        .nodelay()
        .expect("accepted gRPC socket should expose TCP_NODELAY");

    assert!(nodelay);
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
    assert_eq!(
        parse_listen_url("grpc+stdio://").expect("gRPC stdio URL should parse"),
        ListenTransport::GrpcStdio
    );
}

#[test]
fn parse_listen_url_rejects_invalid_transports() {
    let invalid_address = parse_listen_url("grpc://localhost:9000")
        .expect_err("gRPC listener requires an IP address");
    assert!(
        invalid_address
            .to_string()
            .contains("expected `grpc://IP:PORT`")
    );

    let unsupported =
        parse_listen_url("http://127.0.0.1:9000").expect_err("HTTP is not a listen transport");
    assert!(unsupported.to_string().contains("unsupported --listen URL"));
}
