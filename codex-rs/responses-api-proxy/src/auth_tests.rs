//! Tests credential-safe diagnostics, federated exchanges, expiry, and transport.

use super::*;
use pretty_assertions::assert_eq;
use reqwest::blocking::Client;
use serde_json::json;
use std::sync::Arc;
use std::sync::Barrier;
use std::sync::mpsc;
use std::thread;
use tiny_http::Header;
use tiny_http::Server;
use tiny_http::StatusCode;

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

fn recv_response_request(
    server: &Server,
    expected_token: &str,
    expected_body: &str,
) -> tiny_http::Request {
    let mut req = recv(server);
    assert_eq!(req.url(), "/v1/responses");
    assert_eq!(req.method(), &tiny_http::Method::Post);
    assert!(req.headers().iter().any(|h| h.field.equiv("Authorization")
        && h.value.as_str() == format!("Bearer {expected_token}")));
    let mut body = String::new();
    req.as_reader().read_to_string(&mut body).unwrap();
    assert_eq!(body, expected_body);
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
fn rejected_renewal_discards_old_credential_and_does_not_log_or_follow_redirect() {
    let (server, url) = server();
    let target = url.join("/credential-leak").unwrap();
    let worker = thread::spawn(move || {
        serve_exchange(
            &server,
            "issuer.subject.1",
            "original.token",
            /*expires_in*/ 300,
            epoch_seconds() + 300,
        );
        recv_exchange(&server, "issuer.subject.2")
            .respond(
                tiny_http::Response::from_string("issuer.subject.2 original.token")
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
    let auth = auth_at(&url);
    assert_eq!(
        auth.header().unwrap().value.to_str().unwrap(),
        "Bearer original.token"
    );
    let Auth::Federated { ref state, .. } = auth else {
        unreachable!()
    };
    state.lock().unwrap().cached.as_mut().unwrap().renew_at = Instant::now();
    let error = format!("{:#}", auth.header().unwrap_err());
    assert!(error.contains("302"), "{error}");
    for secret in ["issuer.subject.2", "original.token", url.as_str()] {
        assert!(!error.contains(secret), "{error}");
    }
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

#[test]
fn forwards_401_without_replay_then_reacquires_but_keeps_token_on_403() {
    let (upstream, upstream_url) = server();
    let auth = auth_at(&upstream_url);
    let (proxy, proxy_url) = server();
    let upstream_worker = thread::spawn(move || {
        serve_exchange(
            &upstream,
            "issuer.subject.1",
            "openai.first.jwt",
            /*expires_in*/ 300,
            epoch_seconds() + 300,
        );
        recv_response_request(&upstream, "openai.first.jwt", "first")
            .respond(
                tiny_http::Response::from_string("rejected")
                    .with_status_code(StatusCode(401))
                    .with_header(Header::from_bytes("WWW-Authenticate", "Bearer").unwrap()),
            )
            .unwrap();
        // No Responses replay or speculative exchange: the caller submits a
        // distinct POST. A permission failure on that POST must not evict auth.
        serve_exchange(
            &upstream,
            "issuer.subject.2",
            "openai.second.jwt",
            /*expires_in*/ 300,
            epoch_seconds() + 300,
        );
        recv_response_request(&upstream, "openai.second.jwt", "second")
            .respond(
                tiny_http::Response::from_string("forbidden").with_status_code(StatusCode(403)),
            )
            .unwrap();
        recv_response_request(&upstream, "openai.second.jwt", "third")
            .respond(tiny_http::Response::from_string("success"))
            .unwrap();
    });
    let proxy_worker = thread::spawn(move || {
        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .redirect(auth.redirect_policy())
            .build()
            .unwrap();
        let config = crate::ForwardConfig {
            host_header: HeaderValue::from_str(upstream_url.authority()).unwrap(),
            upstream_url: upstream_url.join("/v1/responses").unwrap(),
        };
        for _ in 0..3 {
            crate::forward_request(
                &client,
                &auth,
                &config,
                /*dump_dir*/ None,
                recv(&proxy),
            )
            .unwrap();
        }
    });
    let client = Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let response_url = proxy_url.join("/v1/responses").unwrap();
    let response = client
        .post(response_url.clone())
        .body("first")
        .send()
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(
        response.headers().get("www-authenticate").unwrap(),
        "Bearer"
    );
    assert_eq!(response.text().unwrap(), "rejected");
    let response = client
        .post(response_url.clone())
        .body("second")
        .send()
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::FORBIDDEN);
    assert_eq!(response.text().unwrap(), "forbidden");
    let response = client.post(response_url).body("third").send().unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(response.text().unwrap(), "success");
    proxy_worker.join().unwrap();
    upstream_worker.join().unwrap();
}

#[test]
fn late_rejection_does_not_invalidate_a_newer_credential() {
    let (server, url) = server();
    let worker = thread::spawn(move || {
        serve_exchange(
            &server,
            "issuer.subject.1",
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
    let auth = auth_at(&url);
    let original = auth.header().unwrap();
    original.invalidate();
    let renewed = auth.header().unwrap();
    assert_eq!(renewed.value.to_str().unwrap(), "Bearer openai.second.jwt");
    worker.join().unwrap();
    // Model an in-flight request made with the old token completing after
    // the refresh. The server is closed, so an accidental third exchange fails.
    original.invalidate();
    assert_eq!(auth.header().unwrap().value, renewed.value);
}

#[test]
fn forwards_401_before_an_inflight_renewal_finishes() {
    let (upstream, upstream_url) = server();
    let auth = Arc::new(auth_at(&upstream_url));
    let acquiring = Arc::clone(&auth);
    let first = thread::spawn(move || acquiring.header().unwrap());
    serve_exchange(
        &upstream,
        "issuer.subject.1",
        "openai.first.jwt",
        /*expires_in*/ 300,
        epoch_seconds() + 300,
    );
    let original = first.join().unwrap();
    let (proxy, proxy_url) = server();
    let forwarding = Arc::clone(&auth);
    let config = crate::ForwardConfig {
        host_header: HeaderValue::from_str(upstream_url.authority()).unwrap(),
        upstream_url: upstream_url.join("/v1/responses").unwrap(),
    };
    let proxy_worker = thread::spawn(move || {
        let client = Client::builder()
            .no_proxy()
            .redirect(forwarding.redirect_policy())
            .build()
            .unwrap();
        crate::forward_request(
            &client,
            &forwarding,
            &config,
            /*dump_dir*/ None,
            recv(&proxy),
        )
        .unwrap();
    });
    let (sent, received) = mpsc::sync_channel(1);
    let caller = thread::spawn(move || {
        let response = Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .post(proxy_url.join("/v1/responses").unwrap())
            .body("first")
            .send()
            .unwrap();
        sent.send((response.status(), response.text().unwrap()))
            .unwrap();
    });
    // The request is already at upstream with the first credential. Hold its
    // response until a concurrent rejection has started a new exchange.
    let response_request = recv_response_request(&upstream, "openai.first.jwt", "first");
    original.invalidate();
    let acquiring = Arc::clone(&auth);
    let renewal = thread::spawn(move || acquiring.header().unwrap());
    let exchange = recv_exchange(&upstream, "issuer.subject.2");
    response_request
        .respond(tiny_http::Response::from_string("rejected").with_status_code(StatusCode(401)))
        .unwrap();
    // This must arrive before we let the exchange finish and release the mutex.
    // Always release it before asserting so a regression cannot strand workers.
    let response = received.recv_timeout(Duration::from_secs(5));
    respond_exchange(
        exchange,
        "openai.second.jwt",
        /*expires_in*/ 300,
        epoch_seconds() + 300,
    );
    let renewed = renewal.join().unwrap();
    caller.join().unwrap();
    proxy_worker.join().unwrap();
    assert_eq!(
        response.unwrap(),
        (reqwest::StatusCode::UNAUTHORIZED, "rejected".to_string())
    );
    assert_eq!(auth.header().unwrap().value, renewed.value);
}

fn header_at(auth: &Auth, now: Instant) -> Result<AuthHeader> {
    let Auth::Federated { state } = auth else {
        unreachable!()
    };
    state.lock().unwrap().header(|| now)
}

// Advance to the recorded retry deadline without waiting on real time.
fn elapse_backoff(auth: &Auth, now: &mut Instant) -> Duration {
    let Auth::Federated { state } = auth else {
        unreachable!()
    };
    let state = state.lock().unwrap();
    assert!(state.cached.is_none());
    let failure = state.renewal_failure.as_ref().unwrap();
    *now = failure.retry_at;
    failure.delay
}

#[test]
fn renewal_backoff_starts_after_a_slow_exchange_fails() {
    let (server, url) = server();
    let started = Instant::now();
    let failed = started + Duration::from_secs(10);
    let clock = Arc::new(Mutex::new(started));
    let upstream_clock = Arc::clone(&clock);
    let worker = thread::spawn(move || {
        serve_exchange(
            &server,
            "issuer.subject.1",
            "openai.first.jwt",
            /*expires_in*/ 300,
            epoch_seconds() + 300,
        );
        let exchange = recv_exchange(&server, "issuer.subject.2");
        // Simulate time spent waiting on the failed renewal, longer than its cooldown.
        *upstream_clock.lock().unwrap() = failed;
        exchange
            .respond(
                tiny_http::Response::from_string("temporarily unavailable")
                    .with_status_code(StatusCode(503)),
            )
            .unwrap();
    });
    let auth = auth_at(&url);
    auth.header().unwrap().invalidate();
    let Auth::Federated { state } = &auth else {
        unreachable!()
    };
    let error = state
        .lock()
        .unwrap()
        .header(|| *clock.lock().unwrap())
        .unwrap_err();
    worker.join().unwrap();
    assert_eq!(
        error.to_string(),
        "OpenAI workload identity token request rejected (HTTP 503)"
    );
    let retry_at = failed + Duration::from_secs(1);
    assert_eq!(
        state
            .lock()
            .unwrap()
            .renewal_failure
            .as_ref()
            .unwrap()
            .retry_at,
        retry_at
    );
    assert_eq!(
        header_at(&auth, retry_at - Duration::from_nanos(1))
            .unwrap_err()
            .to_string(),
        "workload identity renewal temporarily unavailable; retry later"
    );
}

#[test]
fn concurrent_renewal_failure_backs_off_caps_and_resets_after_success() {
    let (server, url) = server();
    let worker = thread::spawn(move || {
        serve_exchange(
            &server,
            "issuer.subject.1",
            "openai.first.jwt",
            /*expires_in*/ 300,
            epoch_seconds() + 300,
        );
        for subject in 2..=8 {
            recv_exchange(&server, &format!("issuer.subject.{subject}"))
                .respond(
                    tiny_http::Response::from_string("private-issuer-token openai.first.jwt")
                        .with_status_code(StatusCode(503)),
                )
                .unwrap();
        }
        serve_exchange(
            &server,
            "issuer.subject.9",
            "openai.recovered.jwt",
            /*expires_in*/ 300,
            epoch_seconds() + 300,
        );
        recv_exchange(&server, "issuer.subject.10")
            .respond(
                tiny_http::Response::from_string("private-issuer-token")
                    .with_status_code(StatusCode(503)),
            )
            .unwrap();
    });
    let auth = Arc::new(auth_at(&url));
    let original = auth.header().unwrap();
    let Auth::Federated { state, .. } = auth.as_ref() else {
        unreachable!()
    };
    let mut now = Instant::now();
    state.lock().unwrap().cached.as_mut().unwrap().renew_at = now;
    let barrier = Arc::new(Barrier::new(8));
    // Start every participant before joining any of them.
    let mut callers = Vec::with_capacity(8);
    for _ in 0..8 {
        let auth = Arc::clone(&auth);
        let barrier = Arc::clone(&barrier);
        callers.push(thread::spawn(move || {
            barrier.wait();
            format!("{:#}", header_at(&auth, now).unwrap_err())
        }));
    }
    let errors: Vec<_> = callers
        .into_iter()
        .map(|caller| caller.join().unwrap())
        .collect();
    assert_eq!(errors.iter().filter(|e| e.contains("503")).count(), 1);
    assert_eq!(
        errors.iter().filter(|e| e.contains("retry later")).count(),
        7
    );
    for error in &errors {
        for secret in ["private-issuer-token", "openai.first.jwt", url.as_str()] {
            assert!(!error.contains(secret), "{error}");
        }
    }
    // An older 401 arriving during backoff cannot trigger another renewal.
    original.invalidate();
    assert!(
        header_at(&auth, now)
            .unwrap_err()
            .to_string()
            .contains("retry later")
    );
    for seconds in [1, 2, 4, 8, 16, 30] {
        assert_eq!(
            elapse_backoff(&auth, &mut now),
            Duration::from_secs(seconds)
        );
        assert!(
            header_at(&auth, now)
                .unwrap_err()
                .to_string()
                .contains("503")
        );
        assert!(
            header_at(&auth, now)
                .unwrap_err()
                .to_string()
                .contains("retry later")
        );
    }
    assert_eq!(elapse_backoff(&auth, &mut now), Duration::from_secs(30));
    let renewed = header_at(&auth, now).unwrap();
    assert_eq!(
        renewed.value.to_str().unwrap(),
        "Bearer openai.recovered.jwt"
    );
    assert!(state.lock().unwrap().renewal_failure.is_none());
    renewed.invalidate();
    assert!(
        header_at(&auth, now)
            .unwrap_err()
            .to_string()
            .contains("503")
    );
    assert!(
        header_at(&auth, now)
            .unwrap_err()
            .to_string()
            .contains("retry later")
    );
    assert_eq!(elapse_backoff(&auth, &mut now), Duration::from_secs(1));
    worker.join().unwrap();
}
