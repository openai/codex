use super::sanitize_user_agent;
use super::*;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use std::io;
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;
use tracing_subscriber::layer::SubscriberExt;

#[derive(Clone)]
struct TestLogWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

struct TestLogSink {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for TestLogWriter {
    type Writer = TestLogSink;

    fn make_writer(&'a self) -> Self::Writer {
        TestLogSink {
            buffer: Arc::clone(&self.buffer),
        }
    }
}

impl Write for TestLogSink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.lock().expect("log buffer lock").extend(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn test_get_codex_user_agent() {
    let user_agent = get_codex_user_agent();
    let originator = originator().value;
    let prefix = format!("{originator}/");
    assert!(user_agent.starts_with(&prefix));
}

#[test]
fn is_first_party_originator_matches_known_values() {
    assert_eq!(is_first_party_originator(DEFAULT_ORIGINATOR), true);
    assert_eq!(is_first_party_originator("codex-tui"), true);
    assert_eq!(is_first_party_originator("codex_vscode"), true);
    assert_eq!(is_first_party_originator("Codex Something Else"), true);
    assert_eq!(is_first_party_originator("codex_cli"), false);
    assert_eq!(is_first_party_originator("Other"), false);
}

#[test]
fn is_first_party_chat_originator_matches_known_values() {
    assert_eq!(is_first_party_chat_originator("codex_atlas"), true);
    assert_eq!(
        is_first_party_chat_originator("codex_chatgpt_desktop"),
        true
    );
    assert_eq!(is_first_party_chat_originator(DEFAULT_ORIGINATOR), false);
    assert_eq!(is_first_party_chat_originator("codex_vscode"), false);
}

#[test]
fn add_originator_header_inserts_non_default_originator() {
    let default_originator = originator();
    let thread_originator = if default_originator.value == "chatgpt_cca" {
        "codex_work_cca"
    } else {
        "chatgpt_cca"
    };
    let mut headers = HeaderMap::new();

    add_originator_header(&mut headers, thread_originator);

    assert_eq!(
        headers
            .get("originator")
            .and_then(|value| value.to_str().ok()),
        Some(thread_originator)
    );
}

#[test]
fn add_originator_header_preserves_provider_default() {
    let default_originator = originator();
    let mut headers = HeaderMap::new();
    headers.insert(
        "originator",
        HeaderValue::from_static("provider-originator"),
    );

    add_originator_header(&mut headers, &default_originator.value);

    assert_eq!(
        headers
            .get("originator")
            .and_then(|value| value.to_str().ok()),
        Some("provider-originator")
    );
}

#[test]
fn add_originator_header_omits_invalid_originator() {
    let mut headers = HeaderMap::new();

    add_originator_header(&mut headers, "invalid\noriginator");

    assert!(headers.is_empty());
}

#[tokio::test]
async fn test_create_client_sets_default_headers() {
    skip_if_no_network!();

    set_default_client_residency_requirement(Some(ResidencyRequirement::Us));

    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    let client = create_client();

    // Spin up a local mock server and capture a request.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let resp = client
        .get(server.uri())
        .send()
        .await
        .expect("failed to send request");
    assert!(resp.status().is_success());

    let requests = server
        .received_requests()
        .await
        .expect("failed to fetch received requests");
    assert!(!requests.is_empty());
    let headers = &requests[0].headers;

    // originator header is set to the provided value
    let originator_header = headers
        .get("originator")
        .expect("originator header missing");
    assert_eq!(originator_header.to_str().unwrap(), originator().value);

    // User-Agent matches the computed Codex UA for that originator
    let expected_ua = get_codex_user_agent();
    let ua_header = headers
        .get("user-agent")
        .expect("user-agent header missing");
    assert_eq!(ua_header.to_str().unwrap(), expected_ua);

    let residency_header = headers
        .get(RESIDENCY_HEADER_NAME)
        .expect("residency header missing");
    assert_eq!(residency_header.to_str().unwrap(), "us");

    set_default_client_residency_requirement(/*enforce_residency*/ None);
}

#[tokio::test]
async fn raw_auth_client_does_not_log_sensitive_request_or_response_data() {
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-sensitive-response", "response-secret-value"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let authority = server
        .uri()
        .strip_prefix("http://")
        .expect("wiremock URI should use HTTP")
        .to_string();
    let endpoint = format!(
        "http://auth-user:password-secret-value@{authority}/token?client_secret=query-secret-value"
    );
    let client = create_raw_auth_client(&endpoint, /*auth_route_config*/ None)
        .expect("raw auth client should build");
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(TestLogWriter {
                buffer: Arc::clone(&buffer),
            }),
    );
    let _guard = tracing::subscriber::set_default(subscriber);
    tracing::debug!("log capture sentinel");

    let response = client
        .post(&endpoint)
        .header("x-sensitive-request", "request-header-secret-value")
        .body("request-body-secret-value")
        .send()
        .await
        .expect("raw auth request should succeed");
    assert!(response.status().is_success());

    let unresponsive_listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("unresponsive local listener should bind");
    let unresponsive_addr = unresponsive_listener
        .local_addr()
        .expect("unresponsive local address should be available");
    let unresponsive_endpoint = format!(
        "http://auth-user:failure-password-secret-value@{unresponsive_addr}/token?client_secret=failure-query-secret-value"
    );
    let unresponsive_client =
        create_raw_auth_client(&unresponsive_endpoint, /*auth_route_config*/ None)
            .expect("raw auth client should build");
    let error = unresponsive_client
        .post(&unresponsive_endpoint)
        .header("x-sensitive-request", "failure-request-header-secret-value")
        .body("failure-request-body-secret-value")
        .timeout(std::time::Duration::from_secs(1))
        .send()
        .await
        .expect_err("request to an unresponsive local listener should time out");
    assert!(error.is_timeout());

    let logs = String::from_utf8(buffer.lock().expect("log buffer lock").clone())
        .expect("logs should be UTF-8");
    assert!(logs.contains("log capture sentinel"));
    assert!(!logs.contains("password-secret-value"));
    assert!(!logs.contains("query-secret-value"));
    assert!(!logs.contains("request-header-secret-value"));
    assert!(!logs.contains("request-body-secret-value"));
    assert!(!logs.contains("response-secret-value"));
    assert!(!logs.contains("failure-password-secret-value"));
    assert!(!logs.contains("failure-query-secret-value"));
    assert!(!logs.contains("failure-request-header-secret-value"));
    assert!(!logs.contains("failure-request-body-secret-value"));
}

#[test]
fn test_invalid_suffix_is_sanitized() {
    let prefix = "codex_cli_rs/0.0.0";
    let suffix = "bad\rsuffix";

    assert_eq!(
        sanitize_user_agent(format!("{prefix} ({suffix})"), prefix),
        "codex_cli_rs/0.0.0 (bad_suffix)"
    );
}

#[test]
fn test_invalid_suffix_is_sanitized2() {
    let prefix = "codex_cli_rs/0.0.0";
    let suffix = "bad\0suffix";

    assert_eq!(
        sanitize_user_agent(format!("{prefix} ({suffix})"), prefix),
        "codex_cli_rs/0.0.0 (bad_suffix)"
    );
}

#[test]
#[cfg(target_os = "macos")]
fn test_macos() {
    use regex_lite::Regex;
    let user_agent = get_codex_user_agent();
    let originator = regex_lite::escape(originator().value.as_str());
    let re = Regex::new(&format!(
        r"^{originator}/\d+\.\d+\.\d+ \(Mac OS \d+\.\d+\.\d+; (x86_64|arm64)\) (\S+)$"
    ))
    .unwrap();
    assert!(re.is_match(&user_agent));
}
