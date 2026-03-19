use pretty_assertions::assert_eq;

use super::DEFAULT_LISTEN_URL;
use super::parse_listen_url;

#[test]
fn parse_listen_url_accepts_default_websocket_url() {
    let bind_address =
        parse_listen_url(DEFAULT_LISTEN_URL).expect("default listen URL should parse");
    assert_eq!(
        bind_address,
        super::ListenAddress::Websocket("127.0.0.1:0".parse().expect("valid socket address"))
    );
}

#[test]
fn parse_listen_url_accepts_stdio_url() {
    let listen_address = parse_listen_url("stdio://").expect("stdio listen URL should parse");
    assert_eq!(listen_address, super::ListenAddress::Stdio);
}

#[test]
fn parse_listen_url_accepts_websocket_url() {
    let bind_address =
        parse_listen_url("ws://127.0.0.1:1234").expect("websocket listen URL should parse");
    assert_eq!(
        bind_address,
        super::ListenAddress::Websocket("127.0.0.1:1234".parse().expect("valid socket address"))
    );
}

#[test]
fn parse_listen_url_rejects_invalid_websocket_url() {
    let err = parse_listen_url("ws://localhost:1234")
        .expect_err("hostname bind address should be rejected");
    assert_eq!(
        err.to_string(),
        "invalid websocket --listen URL `ws://localhost:1234`; expected `ws://IP:PORT`"
    );
}

#[test]
fn parse_listen_url_rejects_unsupported_url() {
    let err =
        parse_listen_url("http://127.0.0.1:1234").expect_err("unsupported scheme should fail");
    assert_eq!(
        err.to_string(),
        "unsupported --listen URL `http://127.0.0.1:1234`; expected `ws://IP:PORT`"
    );
}
