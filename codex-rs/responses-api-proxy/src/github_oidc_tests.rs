use super::*;
use crate::auth::tests::direct_token_client;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::io::Cursor;
use std::io::Read;
use std::thread;
use std::time::Duration;
use tiny_http::Header;
use tiny_http::Server;
use tiny_http::StatusCode;
use tokio::runtime::Runtime;

#[test]
fn stdin_accepts_bounded_bearer_tokens_in_short_reads_and_rejects_injection() {
    let token = format!("{}.payload.sig", "a".repeat(2000));
    let input = format!("{token}\r\n");
    let mut cursor = Cursor::new(input.as_bytes());
    let header = read_request_auth(|buf| {
        let len = buf.len().min(3);
        cursor.read(&mut buf[..len])
    })
    .unwrap();
    assert_eq!(header.to_str().unwrap(), format!("Bearer {token}"));
    assert!(header.is_sensitive());
    for token in ["", "a\rb", "a b", "a\0b", "x:y", "💥", "a\r\r", "a\r\r\n"] {
        let mut cursor = Cursor::new(token.as_bytes());
        assert!(read_request_auth(|buf| cursor.read(buf)).is_err());
    }
    for suffix in ["", "\n", "\r\n"] {
        let max_token = "a".repeat(MAX_TOKEN_BYTES);
        let input = format!("{max_token}{suffix}");
        let mut cursor = Cursor::new(input.as_bytes());
        let header = read_request_auth(|buf| cursor.read(buf)).unwrap();
        assert_eq!(header.to_str().unwrap(), format!("Bearer {max_token}"));

        let input = format!("a{max_token}{suffix}");
        let mut cursor = Cursor::new(input.as_bytes());
        assert!(read_request_auth(|buf| cursor.read(buf)).is_err());
    }
}

#[test]
fn stdin_read_error_cannot_echo_a_github_credential() {
    let mut sent = false;
    let error = read_request_auth(|buf| {
        if sent {
            return Err(std::io::Error::other("private.github.request"));
        }
        sent = true;
        buf[..3].copy_from_slice(b"abc");
        Ok(3)
    })
    .unwrap_err();
    assert_eq!(
        format!("{error:#}"),
        "could not read GitHub OIDC request credential from stdin"
    );
}

fn server() -> (Server, Url) {
    let server = Server::http("127.0.0.1:0").unwrap();
    let url = Url::parse(&format!("http://{}", server.server_addr())).unwrap();
    (server, url)
}

fn source_at(url: &Url) -> GithubOidc {
    let mut cursor = Cursor::new(b"private.github.request");
    GithubOidc {
        request_url: url.join("/github").unwrap(),
        request_auth: read_request_auth(|buf| cursor.read(buf)).unwrap(),
    }
}

fn client() -> (TokenClient, Runtime) {
    let client = direct_token_client();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    (client, runtime)
}

fn recv(server: &Server) -> tiny_http::Request {
    server
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .expect("expected HTTP request")
}

#[test]
fn obtains_fresh_subject_tokens_using_github_request_credential() {
    let (server, url) = server();
    let worker = thread::spawn(move || {
        for token in ["github.first.jwt", "github.second.jwt"] {
            let req = recv(&server);
            assert_eq!(req.url(), "/github");
            assert_eq!(req.method(), &tiny_http::Method::Get);
            assert!(req.headers().iter().any(|h| h.field.equiv("Authorization")
                && h.value.as_str() == "Bearer private.github.request"));
            req.respond(
                tiny_http::Response::from_string(json!({"value": token}).to_string())
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap()),
            )
            .unwrap();
        }
    });
    let mut source: Box<dyn SubjectTokenSource> = Box::new(source_at(&url));
    let (client, runtime) = client();
    runtime.block_on(async {
        assert_eq!(
            source.subject_token(&client).await.unwrap().as_str(),
            "github.first.jwt"
        );
        assert_eq!(
            source.subject_token(&client).await.unwrap().as_str(),
            "github.second.jwt"
        );
    });
    worker.join().unwrap();
}

#[test]
#[tracing_test::traced_test]
fn rejected_subject_does_not_log_response_or_follow_redirect() {
    tracing::debug!(target: "codex_http_client::client", "credential log capture sentinel");
    let (server, url) = server();
    let target = url.join("/credential-leak").unwrap();
    let worker = thread::spawn(move || {
        let req = recv(&server);
        assert_eq!(req.url(), "/github?opaque=private.github.url");
        req.respond(
            tiny_http::Response::from_string("private.github.request")
                .with_status_code(StatusCode(302))
                .with_header(Header::from_bytes("Location", target.as_str()).unwrap()),
        )
        .unwrap();
        assert!(
            server
                .recv_timeout(Duration::from_millis(200))
                .unwrap()
                .is_none()
        );
    });
    let (client, runtime) = client();
    let mut source = source_at(&url);
    source
        .request_url
        .set_query(Some("opaque=private.github.url"));
    let error = format!(
        "{:#}",
        runtime.block_on(source.subject_token(&client)).unwrap_err()
    );
    assert!(error.contains("302"), "{error}");
    assert!(!error.contains("private.github.request"), "{error}");
    assert!(!error.contains("private.github.url"), "{error}");
    assert!(!error.contains(url.as_str()), "{error}");
    worker.join().unwrap();
    assert!(logs_contain("credential log capture sentinel"));
    assert!(!logs_contain("private.github.url"));
    assert!(!logs_contain("private.github.request"));
}

#[test]
fn malformed_json_cannot_echo_a_secret_in_diagnostics() {
    let (server, url) = server();
    let worker = thread::spawn(move || {
        recv(&server)
            .respond(tiny_http::Response::from_string(
                r#"{"value": private.github.request}"#,
            ))
            .unwrap();
    });
    let (client, runtime) = client();
    let error = format!(
        "{:#}",
        runtime
            .block_on(source_at(&url).subject_token(&client))
            .unwrap_err()
    );
    assert!(error.contains("invalid token response"), "{error}");
    assert!(!error.contains("private.github.request"), "{error}");
    worker.join().unwrap();
}
