use super::AppServerCodeModeHostArgs;
use super::CodeModeHostTransport;
use super::parse_websocket_url;
use pretty_assertions::assert_eq;
use url::Url;

#[test]
fn websocket_host_accepts_local_and_secure_endpoints() {
    for endpoint in ["ws://127.0.0.1:8765", "wss://example.test/code-mode"] {
        assert_eq!(
            parse_websocket_url(endpoint),
            Ok(Url::parse(endpoint).expect("test endpoint should parse"))
        );
    }
}

#[test]
fn websocket_host_rejects_invalid_endpoints() {
    for endpoint in [
        "http://127.0.0.1:8765",
        "https://example.test/code-mode",
        "ws://",
        "not a websocket",
        "wss://example.test/code-mode#fragment",
    ] {
        assert!(
            parse_websocket_url(endpoint).is_err(),
            "invalid code-mode host endpoint should be rejected: {endpoint}"
        );
    }
}

#[test]
fn omitted_websocket_host_selects_local_transport() {
    assert_eq!(
        CodeModeHostTransport::from(AppServerCodeModeHostArgs::default()),
        CodeModeHostTransport::Local
    );
}

#[test]
fn explicit_websocket_host_selects_remote_transport() {
    let url = Url::parse("wss://example.test/code-mode").expect("test endpoint should parse");

    assert_eq!(
        CodeModeHostTransport::from(AppServerCodeModeHostArgs {
            code_mode_host: Some(url.clone()),
        }),
        CodeModeHostTransport::WebSocket(url)
    );
}
