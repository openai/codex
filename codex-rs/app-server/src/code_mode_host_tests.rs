use super::AppServerCodeModeHostArgs;
use super::CodeModeHostTransport;
use super::parse_host_url;
use pretty_assertions::assert_eq;
use url::Url;

#[test]
fn websocket_host_accepts_local_and_secure_endpoints() {
    for endpoint in [
        "ws://127.0.0.1:8765",
        "wss://example.test/code-mode",
        "ws://alice:secret@example.test/code-mode",
        "wss://alice:secret@example.test/code-mode",
    ] {
        assert_eq!(
            parse_host_url(endpoint),
            Ok(Url::parse(endpoint).expect("test endpoint should parse"))
        );
    }
}

#[test]
fn grpc_host_accepts_local_and_secure_endpoints() {
    for endpoint in ["http://127.0.0.1:8765", "https://example.test"] {
        assert_eq!(
            parse_host_url(endpoint),
            Ok(Url::parse(endpoint).expect("test endpoint should parse"))
        );
    }
}

#[test]
fn grpc_host_rejects_credentials_without_disclosing_them() {
    for endpoint in [
        "http://alice:secret@example.test",
        "https://alice:secret@example.test",
        "https://alice@example.test",
        "https://:secret@example.test",
    ] {
        let error = parse_host_url(endpoint).expect_err("gRPC credentials should be rejected");

        assert!(error.contains("must not contain credentials"));
        assert!(!error.contains("alice"));
        assert!(!error.contains("secret"));
    }
}

#[test]
fn code_mode_host_rejects_invalid_endpoints() {
    for endpoint in [
        "ftp://127.0.0.1:8765",
        "ws://",
        "not a host endpoint",
        "wss://example.test/code-mode#fragment",
        "https://example.test/code-mode#fragment",
        "https://example.test/code-mode",
        "http://example.test/?token=secret",
    ] {
        assert!(
            parse_host_url(endpoint).is_err(),
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

#[test]
fn explicit_grpc_host_selects_remote_transport() {
    let url = Url::parse("https://example.test").expect("test endpoint should parse");

    assert_eq!(
        CodeModeHostTransport::from(AppServerCodeModeHostArgs {
            code_mode_host: Some(url.clone()),
        }),
        CodeModeHostTransport::Grpc(url)
    );
}
