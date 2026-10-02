//! Confirmed conflicts and pre-write auth outages permit replay; ambiguous results remain terminal.

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_api::AuthProvider;
use codex_http_client::RouteAwareRequestError;
use http::HeaderMap;
use pretty_assertions::assert_eq;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tokio::time::advance;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio_util::task::AbortOnDropHandle;
use tracing::instrument::WithSubscriber;
use tracing_subscriber::prelude::*;

use super::EnvironmentRegistryClient;
use super::ExecServerError;
use crate::NoiseChannelIdentity;

const ENVIRONMENT_ID: &str = "registration-retry-test";
const CONFLICT_BODY: &str =
    r#"{"error":{"code":"registration_conflict","message":"registration unavailable"}}"#;
const CONFLICT_WITH_STRING_DETAIL_BODY: &str = r#"{"error":{"code":"registration_conflict","message":"registration unavailable"},"detail":"additional diagnostics"}"#;
const CONFLICT_WITH_STRUCTURED_DETAIL_BODY: &str = r#"{"error":{"code":"registration_conflict","message":"registration unavailable"},"detail":{"code":"registration_denied","message":"additional diagnostics"}}"#;
const AUTH_UNAVAILABLE_BODY: &str = r#"{"detail":{"code":"authentication_service_unavailable","message":"Authentication service unavailable"}}"#;
const ERROR_BODY: &str =
    r#"{"error":{"code":"registration_denied","message":"registration unavailable"}}"#;
const ERROR_WITH_AUTH_DETAIL_BODY: &str = r#"{"error":{"code":"registration_denied","message":"registration unavailable"},"detail":{"code":"authentication_service_unavailable","message":"Authentication service unavailable"}}"#;
const SUCCESS_BODY: &str = r#"{"environment_id":"registration-retry-test","url":"ws://localhost/relay","security_profile":"noise_hybrid_ik_v1","executor_registration_id":"committed-registration"}"#;

#[derive(Debug, Default)]
struct RegistrationAuthProvider {
    calls: AtomicUsize,
}

impl AuthProvider for RegistrationAuthProvider {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) {}

    fn resolve_auth_headers(&self) -> codex_api::AuthHeadersFuture<'_> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Box::pin(async { Ok(HeaderMap::new()) })
    }
}

struct RetryObserved(Arc<Notify>);

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for RetryObserved {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        if event.metadata().target() == "codex_exec_server::remote::registration_retry"
            && event.metadata().fields().field("retry_attempt").is_some()
        {
            self.0.notify_one();
        }
    }
}

#[test_case::test_case(200, "not JSON", Duration::ZERO; "malformed_success")]
#[test_case::test_case(200, SUCCESS_BODY, Duration::from_secs(60); "committed_success_body_timeout")]
#[test_case::test_case(503, CONFLICT_BODY, Duration::ZERO; "confirmed_conflict_backoff_is_cancellable")]
#[test_case::test_case(503, CONFLICT_WITH_STRING_DETAIL_BODY, Duration::ZERO; "conflict_with_string_detail")]
#[test_case::test_case(503, CONFLICT_WITH_STRUCTURED_DETAIL_BODY, Duration::ZERO; "conflict_with_structured_detail")]
#[test_case::test_case(503, CONFLICT_BODY, Duration::from_secs(60); "conflict_code_not_received")]
#[test_case::test_case(503, "not JSON", Duration::ZERO; "malformed_unavailable")]
#[test_case::test_case(503, "{}", Duration::ZERO; "missing_conflict_code")]
#[test_case::test_case(503, ERROR_BODY, Duration::ZERO; "different_error_code")]
#[test_case::test_case(502, CONFLICT_BODY, Duration::ZERO; "gateway_error")]
#[test_case::test_case(502, AUTH_UNAVAILABLE_BODY, Duration::ZERO; "auth_outage_backoff_is_cancellable")]
#[test_case::test_case(502, ERROR_WITH_AUTH_DETAIL_BODY, Duration::ZERO; "canonical_error_overrides_auth_detail")]
#[test_case::test_case(502, AUTH_UNAVAILABLE_BODY, Duration::from_secs(60); "auth_outage_code_not_received")]
#[test_case::test_case(502, "not JSON", Duration::ZERO; "unconfirmed_gateway_error")]
#[test_case::test_case(502, r#"{"detail":"Authentication service unavailable"}"#, Duration::ZERO; "legacy_auth_outage")]
#[test_case::test_case(503, AUTH_UNAVAILABLE_BODY, Duration::ZERO; "auth_outage_wrong_status")]
#[test_case::test_case(408, CONFLICT_BODY, Duration::ZERO; "request_timeout")]
#[test_case::test_case(429, CONFLICT_BODY, Duration::ZERO; "too_many_requests")]
#[test_case::test_case(401, ERROR_BODY, Duration::from_secs(60); "unauthorized_stalled_body")]
#[test_case::test_case(403, ERROR_BODY, Duration::from_secs(60); "forbidden_stalled_body")]
#[test_case::test_case(404, ERROR_BODY, Duration::from_secs(60); "environment_deleted_stalled_body")]
#[test_case::test_case(401, ERROR_BODY, Duration::from_millis(50); "delayed_unauthorized_details")]
#[test_case::test_case(403, ERROR_BODY, Duration::from_millis(50); "delayed_forbidden_details")]
#[test_case::test_case(404, ERROR_BODY, Duration::from_millis(50); "delayed_error_details")]
#[tokio::test]
async fn registration_requires_a_confirmed_pre_write_failure_before_replay(
    status: u16,
    body: &'static str,
    body_delay: Duration,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let registry_url = format!("http://{}", listener.local_addr()?);
    let _server = AbortOnDropHandle::new(tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        // Drain the complete request before responding so unread bytes cannot cause a reset.
        let mut reader = BufReader::new(&mut stream);
        let mut content_length = 0;
        loop {
            let mut line = String::new();
            anyhow::ensure!(reader.read_line(&mut line).await? > 0, "missing headers");
            if line == "\r\n" {
                break;
            }
            if let Some((name, value)) = line.split_once(':')
                && name.eq_ignore_ascii_case("content-length")
            {
                content_length = value.trim().parse::<usize>()?;
            }
        }
        anyhow::ensure!(content_length <= 16_384, "registration request too large");
        reader.read_exact(&mut vec![0; content_length]).await?;
        drop(reader);

        let headers = format!(
            "HTTP/1.1 {status} Registration Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        stream.write_all(headers.as_bytes()).await?;
        sleep(body_delay).await;
        stream.write_all(body.as_bytes()).await?;
        std::future::pending::<()>().await;
        anyhow::Ok(())
    }));
    let auth = Arc::new(RegistrationAuthProvider::default());
    let mut client = EnvironmentRegistryClient::new(registry_url, auth.clone())?;
    client.connect_timeout = Duration::from_millis(500);
    let key = NoiseChannelIdentity::generate()?.public_key();
    let retry = Arc::new(Notify::new());
    let subscriber = tracing_subscriber::registry().with(RetryObserved(retry.clone()));
    let mut registration = Box::pin(
        client
            .register_environment_with_retry(ENVIRONMENT_ID, &key)
            .with_subscriber(subscriber),
    );

    if body_delay.is_zero()
        && ((status == 503
            && matches!(
                body,
                CONFLICT_BODY
                    | CONFLICT_WITH_STRING_DETAIL_BODY
                    | CONFLICT_WITH_STRUCTURED_DETAIL_BODY
            ))
            || (status == 502 && body == AUTH_UNAVAILABLE_BODY))
    {
        timeout(Duration::from_secs(1), async {
            tokio::select! {
                _ = retry.notified() => anyhow::Ok(()),
                _ = &mut registration => anyhow::bail!("a confirmed pre-write failure must enter backoff"),
            }
        })
        .await??;
        // The retry event fires in the same poll that enters backoff, without a timing guess.
        drop(registration);
        tokio::time::pause();
        advance(Duration::from_secs(60)).await;
        tokio::task::yield_now().await;
        assert_eq!(auth.calls.load(Ordering::Relaxed), 1);
        return Ok(());
    }

    let error = timeout(Duration::from_secs(1), registration)
        .await?
        .expect_err("an unconfirmed registration outcome must not be replayed");
    assert_eq!(auth.calls.load(Ordering::Relaxed), 1);

    match error {
        ExecServerError::Json(_) if status == 200 && body_delay.is_zero() => {}
        ExecServerError::EnvironmentRegistryRequest(RouteAwareRequestError::Timeout)
            if status == 200 && body_delay > client.connect_timeout => {}
        ExecServerError::EnvironmentRegistryAuth(message) if matches!(status, 401 | 403) => {
            if body_delay < client.connect_timeout {
                assert!(message.ends_with(": registration unavailable"));
            }
        }
        ExecServerError::EnvironmentRegistryHttp {
            status: actual,
            code,
            message,
        } if !matches!(status, 200 | 401 | 403) => {
            let expected_code = match (body_delay < client.connect_timeout, body) {
                (true, ERROR_BODY | ERROR_WITH_AUTH_DETAIL_BODY) => Some("registration_denied"),
                (true, CONFLICT_BODY) => Some("registration_conflict"),
                (true, AUTH_UNAVAILABLE_BODY) => Some("authentication_service_unavailable"),
                _ => None,
            };
            assert_eq!((actual.as_u16(), code.as_deref()), (status, expected_code));
            if expected_code == Some("authentication_service_unavailable") {
                assert_eq!(message, "Authentication service unavailable");
            } else if expected_code.is_some() {
                assert_eq!(message, "registration unavailable");
            }
        }
        _ => anyhow::bail!("unexpected registration failure kind"),
    }
    Ok(())
}
