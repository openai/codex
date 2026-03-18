use pretty_assertions::assert_eq;

use super::ExecServerTransport;

#[test]
fn exec_server_transport_parses_default_websocket_listen_url() {
    let transport = ExecServerTransport::from_listen_url(ExecServerTransport::DEFAULT_LISTEN_URL)
        .expect("default listen URL should parse");
    assert_eq!(
        transport,
        ExecServerTransport::WebSocket {
            bind_address: "127.0.0.1:0".parse().expect("valid socket address"),
        }
    );
}

#[test]
fn exec_server_transport_parses_stdio_listen_url() {
    let transport =
        ExecServerTransport::from_listen_url("stdio://").expect("stdio listen URL should parse");
    assert_eq!(transport, ExecServerTransport::Stdio);
}

#[test]
fn exec_server_transport_parses_websocket_listen_url() {
    let transport = ExecServerTransport::from_listen_url("ws://127.0.0.1:1234")
        .expect("websocket listen URL should parse");
    assert_eq!(
        transport,
        ExecServerTransport::WebSocket {
            bind_address: "127.0.0.1:1234".parse().expect("valid socket address"),
        }
    );
}

#[test]
fn exec_server_transport_rejects_invalid_websocket_listen_url() {
    let err = ExecServerTransport::from_listen_url("ws://localhost:1234")
        .expect_err("hostname bind address should be rejected");
    assert_eq!(
        err.to_string(),
        "invalid websocket --listen URL `ws://localhost:1234`; expected `ws://IP:PORT`"
    );
}

#[test]
fn exec_server_transport_rejects_unsupported_listen_url() {
    let err = ExecServerTransport::from_listen_url("http://127.0.0.1:1234")
        .expect_err("unsupported scheme should fail");
    assert_eq!(
        err.to_string(),
        "unsupported --listen URL `http://127.0.0.1:1234`; expected `stdio://` or `ws://IP:PORT`"
    );
}
