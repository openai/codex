//! Tests credential-safe diagnostics, federated exchanges, expiry, and transport.

use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use tiny_http::Header;
use tiny_http::Server;

pub(crate) fn direct_token_client() -> TokenClient {
    TokenClient(TokenClient::builder().build_direct().unwrap())
}

#[test]
fn debug_redacts_api_keys_in_both_formats() {
    let auth = Auth::ApiKey("Bearer test-secret-for-proxy-auth");

    for output in [format!("{auth:?}"), format!("{auth:#?}")] {
        assert!(!output.contains("test-secret-for-proxy-auth"));
        assert!(output.contains("ApiKey"));
        assert!(output.contains("[REDACTED]"));
    }
}

// A non-GitHub issuer: each renewal must ask the source for a fresh subject.
struct RotatingSource {
    calls: usize,
}

impl SubjectTokenSource for RotatingSource {
    fn subject_token<'a>(&'a mut self, _client: &'a TokenClient) -> SubjectTokenFuture<'a> {
        Box::pin(async move {
            self.calls += 1;
            Ok(Zeroizing::new(format!("issuer.subject.{}", self.calls)))
        })
    }
}

fn server() -> (Server, Url) {
    let server = Server::http("127.0.0.1:0").unwrap();
    let url = Url::parse(&format!("http://{}", server.server_addr())).unwrap();
    (server, url)
}

fn auth_at(url: &Url) -> Auth {
    let auth = Auth::federated(
        RotatingSource { calls: 0 },
        "ip-test".to_owned(),
        "sa-test".to_owned(),
    )
    .unwrap();
    let Auth::Federated { state, .. } = &auth else {
        unreachable!()
    };
    let mut state = state.lock().unwrap();
    state.token_url = url.join("/oauth/token").unwrap();
    // Fixtures must not use a developer's system or environment proxy.
    state.client = direct_token_client();
    drop(state);
    auth
}

fn recv(server: &Server) -> tiny_http::Request {
    server
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .expect("expected HTTP request")
}

fn recv_exchange(server: &Server, subject: &str) -> tiny_http::Request {
    let mut req = recv(server);
    assert_eq!(req.url(), "/oauth/token");
    assert_eq!(req.method(), &tiny_http::Method::Post);
    assert!(!req.headers().iter().any(|h| h.field.equiv("Authorization")));
    assert_eq!(
        req.headers()
            .iter()
            .find(|h| h.field.equiv("Content-Type"))
            .map(|h| h.value.as_str()),
        Some("application/json")
    );
    let mut body = String::new();
    req.as_reader().read_to_string(&mut body).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap(),
        json!({
            "grant_type": "urn:ietf:params:oauth:grant-type:token-exchange",
            "subject_token_type": "urn:ietf:params:oauth:token-type:jwt",
            "subject_token": subject,
            "identity_provider_id": "ip-test",
            "service_account_id": "sa-test",
        })
    );
    req
}

fn serve_exchange(server: &Server, subject: &str, token: &str, expires_in: u64, expires_at: u64) {
    respond_exchange(
        recv_exchange(server, subject),
        token,
        expires_in,
        expires_at,
    );
}

fn respond_exchange(req: tiny_http::Request, token: &str, expires_in: u64, expires_at: u64) {
    req.respond(
        tiny_http::Response::from_string(
            json!({
                "access_token": token, "token_type": "Bearer",
                "issued_token_type": "urn:ietf:params:oauth:token-type:access_token",
                "expires_in": expires_in, "expires_at": expires_at,
            })
            .to_string(),
        )
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap()),
    )
    .unwrap();
}

fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[test]
fn oversized_serialized_exchange_request_is_rejected_before_sending() {
    let (server, url) = server();
    let auth = auth_at(&url);
    let Auth::Federated { state, .. } = &auth else {
        unreachable!()
    };
    // Fits unescaped, but JSON escaping makes the serialized request exceed the limit.
    state.lock().unwrap().provider = "\0".repeat(MAX_EXCHANGE_REQUEST_BYTES / 6 + 1);

    assert_eq!(
        auth.header().unwrap_err().to_string(),
        format!(
            "OpenAI workload identity exchange request exceeds {MAX_EXCHANGE_REQUEST_BYTES} bytes"
        )
    );
    assert!(state.lock().unwrap().cached.is_none());
    assert!(server.try_recv().unwrap().is_none());
}

#[test]
fn oversized_exchange_never_becomes_a_cached_credential() {
    let (server, url) = server();
    let worker = thread::spawn(move || {
        let body = json!({
            "access_token": "secret".repeat(MAX_RESPONSE_BYTES),
            "token_type": "Bearer",
            "issued_token_type": "urn:ietf:params:oauth:token-type:access_token",
            "expires_in": 300,
            "expires_at": epoch_seconds() + 300,
        });
        let _ = recv_exchange(&server, "issuer.subject.1").respond(
            tiny_http::Response::from_string(body.to_string())
                .with_header(Header::from_bytes("Content-Type", "application/json").unwrap()),
        );
    });
    let auth = auth_at(&url);
    let error = format!("{:#}", auth.header().unwrap_err());
    assert_eq!(
        error,
        "OpenAI workload identity returned an oversized token response"
    );
    let Auth::Federated { state, .. } = &auth else {
        unreachable!()
    };
    assert!(state.lock().unwrap().cached.is_none());
    worker.join().unwrap();
}

#[test]
fn exchanges_once_for_concurrent_requests_and_renews_with_a_new_subject() {
    let (server, url) = server();
    let auth = Arc::new(auth_at(&url));
    let (attempt_tx, attempt_rx) = mpsc::channel();
    let (in_flight_tx, in_flight_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let pending = recv_exchange(&server, "issuer.subject.1");
        in_flight_tx.send(()).unwrap();
        // Each other caller must observe the lock held during the first exchange.
        for _ in 0..7 {
            attempt_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        assert!(server.try_recv().unwrap().is_none());
        respond_exchange(
            pending,
            "openai.first.jwt",
            /*expires_in*/ 300,
            epoch_seconds() + 300,
        );
        serve_exchange(
            &server,
            "issuer.subject.2",
            "openai.second.jwt",
            /*expires_in*/ 300,
            epoch_seconds() + 300,
        );
    });
    let first = {
        let auth = Arc::clone(&auth);
        thread::spawn(move || auth.header().unwrap())
    };
    in_flight_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let callers: Vec<_> = (0..7)
        .map(|_| {
            let auth = Arc::clone(&auth);
            let attempted = attempt_tx.clone();
            thread::spawn(move || {
                let Auth::Federated { state } = auth.as_ref() else {
                    unreachable!()
                };
                assert!(matches!(
                    state.try_lock(),
                    Err(std::sync::TryLockError::WouldBlock)
                ));
                attempted.send(()).unwrap();
                auth.header().unwrap()
            })
        })
        .collect();
    for caller in std::iter::once(first).chain(callers) {
        let header = caller.join().unwrap();
        assert!(header.value.is_sensitive());
        assert_eq!(header.value.to_str().unwrap(), "Bearer openai.first.jwt");
    }
    let Auth::Federated { state, .. } = auth.as_ref() else {
        unreachable!()
    };
    state.lock().unwrap().cached.as_mut().unwrap().renew_at = Instant::now();
    assert_eq!(
        auth.header().unwrap().value.to_str().unwrap(),
        "Bearer openai.second.jwt"
    );
    worker.join().unwrap();
}

#[test]
fn exchange_is_never_cached_if_expired_or_lifetime_exceeds_the_contract() {
    for (expires_in, expires_at, expected_error) in [
        (
            3600,
            epoch_seconds() - 1,
            "OpenAI returned an expired workload identity token",
        ),
        (
            0,
            epoch_seconds() + 3600,
            "OpenAI returned an expired workload identity token",
        ),
        (
            3601,
            epoch_seconds() + 7200,
            "OpenAI returned an invalid token lifetime",
        ),
    ] {
        let (server, url) = server();
        let worker = thread::spawn(move || {
            serve_exchange(
                &server,
                "issuer.subject.1",
                "expired.secret.token",
                expires_in,
                expires_at,
            );
        });
        let auth = auth_at(&url);
        assert_eq!(auth.header().unwrap_err().to_string(), expected_error);
        let Auth::Federated { state, .. } = auth else {
            unreachable!()
        };
        assert!(state.lock().unwrap().cached.is_none());
        worker.join().unwrap();
    }
}

#[test]
fn unsupported_or_missing_issued_token_type_is_never_cached_or_echoed() {
    for (issued_type, expected_error) in [
        (
            Some("urn:ietf:params:oauth:token-type:id_token"),
            "OpenAI returned an unsupported workload identity token type",
        ),
        (
            Some("echoed.secret.in.type"),
            "OpenAI returned an unsupported workload identity token type",
        ),
        (
            None,
            "OpenAI workload identity returned an invalid token response",
        ),
    ] {
        let (server, url) = server();
        let worker = thread::spawn(move || {
            let mut body = json!({
                "access_token": "openai.secret.jwt", "token_type": "Bearer",
                "expires_in": 300, "expires_at": epoch_seconds() + 300,
            });
            if let Some(issued_type) = issued_type {
                body["issued_token_type"] = json!(issued_type);
            }
            recv_exchange(&server, "issuer.subject.1")
                .respond(tiny_http::Response::from_string(body.to_string()))
                .unwrap();
        });
        let auth = auth_at(&url);
        assert_eq!(format!("{:#}", auth.header().unwrap_err()), expected_error);
        let Auth::Federated { state, .. } = auth else {
            unreachable!()
        };
        assert!(state.lock().unwrap().cached.is_none());
        worker.join().unwrap();
    }
}

#[test]
fn absolute_expiry_forces_renewal_and_discards_cached_credential_on_failure() {
    let (server, url) = server();
    let worker = thread::spawn(move || {
        serve_exchange(
            &server,
            "issuer.subject.1",
            "openai.first.jwt",
            /*expires_in*/ 300,
            // The issuer's clock is ahead. Accept the token, but don't cache it
            // past the shorter relative TTL, including across host suspension.
            epoch_seconds() + 7200,
        );
        serve_exchange(
            &server,
            "issuer.subject.2",
            "openai.second.jwt",
            /*expires_in*/ 300,
            epoch_seconds() + 300,
        );
        recv_exchange(&server, "issuer.subject.3")
            .respond(tiny_http::Response::empty(tiny_http::StatusCode(503)))
            .unwrap();
    });
    let auth = auth_at(&url);
    assert_eq!(
        auth.header().unwrap().value.to_str().unwrap(),
        "Bearer openai.first.jwt"
    );
    let Auth::Federated { state, .. } = &auth else {
        unreachable!()
    };
    {
        let mut state = state.lock().unwrap();
        let cached = state.cached.as_mut().unwrap();
        // Model a suspend: wall time has crossed expiry but Instant has not advanced.
        assert!(Instant::now() < cached.renew_at);
        assert!(cached.expires_at <= SystemTime::now() + Duration::from_secs(300));
        cached.expires_at = UNIX_EPOCH;
    }
    assert_eq!(
        auth.header().unwrap().value.to_str().unwrap(),
        "Bearer openai.second.jwt"
    );
    state.lock().unwrap().cached.as_mut().unwrap().expires_at = UNIX_EPOCH;
    assert_eq!(
        auth.header().unwrap_err().to_string(),
        "OpenAI workload identity token request rejected (HTTP 503)"
    );
    assert!(state.lock().unwrap().cached.is_none());
    worker.join().unwrap();
}

#[test]
fn invalid_subjects_are_rejected_before_exchange() {
    struct InvalidSource(String);
    impl SubjectTokenSource for InvalidSource {
        fn subject_token<'a>(&'a mut self, _client: &'a TokenClient) -> SubjectTokenFuture<'a> {
            Box::pin(async move { Ok(Zeroizing::new(self.0.clone())) })
        }
    }
    for token in [String::new(), "a".repeat(MAX_TOKEN_BYTES + 1)] {
        let auth = Auth::federated(
            InvalidSource(token),
            "ip-test".to_owned(),
            "sa-test".to_owned(),
        )
        .unwrap();
        assert_eq!(
            auth.header().unwrap_err().to_string(),
            "workload identity source returned an invalid subject token"
        );
    }
}
