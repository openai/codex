use super::sanitize_user_agent;
use super::*;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serial_test::serial;

static RESIDENCY_TEST_LOCK: std::sync::LazyLock<std::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(()));

struct ResidencyGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl ResidencyGuard {
    fn set(enforce_residency: Option<ResidencyRequirement>) -> Self {
        let lock = RESIDENCY_TEST_LOCK
            .lock()
            .expect("residency test lock should not be poisoned");
        set_default_client_residency_requirement(enforce_residency);
        Self { _lock: lock }
    }
}

impl Drop for ResidencyGuard {
    fn drop(&mut self) {
        set_default_client_residency_requirement(/*enforce_residency*/ None);
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
fn explicit_client_identity_builds_expected_headers() {
    let _guard = ResidencyGuard::set(Some(ResidencyRequirement::Us));

    let identity = ClientIdentity::explicit(
        "codex_test_client".to_string(),
        Some("codex_test_client; 1.2.3".to_string()),
    )
    .expect("explicit identity should be valid");
    let headers = identity.headers_with_default_residency();
    let user_agent = identity.user_agent();

    assert_eq!(identity.originator_value(), "codex_test_client");
    assert!(user_agent.starts_with("codex_test_client/"));
    assert!(user_agent.ends_with(" (codex_test_client; 1.2.3)"));
    assert_eq!(
        headers
            .get("originator")
            .and_then(|value| value.to_str().ok()),
        Some("codex_test_client"),
    );
    assert_eq!(
        headers
            .get(reqwest::header::USER_AGENT)
            .and_then(|value| value.to_str().ok()),
        Some(user_agent.as_str()),
    );
    assert_eq!(
        headers
            .get(RESIDENCY_HEADER_NAME)
            .and_then(|value| value.to_str().ok()),
        Some("us"),
    );
}

#[test]
fn explicit_client_identity_rejects_invalid_originator() {
    assert!(matches!(
        ClientIdentity::explicit(
            "bad\noriginator".to_string(),
            /*user_agent_suffix*/ None
        ),
        Err(SetOriginatorError::InvalidHeaderValue)
    ));
}

#[test]
fn process_default_client_identity_matches_default_headers() {
    let identity = ClientIdentity::process_default();
    let identity_headers = identity.headers_with_default_residency();
    let default_headers = default_headers();

    assert_eq!(identity.user_agent(), get_codex_user_agent());
    assert_eq!(
        identity_headers
            .get("originator")
            .and_then(|value| value.to_str().ok()),
        default_headers
            .get("originator")
            .and_then(|value| value.to_str().ok()),
    );
    assert_eq!(
        identity_headers
            .get(reqwest::header::USER_AGENT)
            .and_then(|value| value.to_str().ok()),
        default_headers
            .get(reqwest::header::USER_AGENT)
            .and_then(|value| value.to_str().ok()),
    );
}

#[test]
#[serial(default_client_env)]
fn explicit_client_identity_ignores_originator_override_env_var() {
    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: this test is serialized with other tests that mutate this env var.
            unsafe {
                match &self.previous {
                    Some(value) => std::env::set_var(self.key, value),
                    None => std::env::remove_var(self.key),
                }
            }
        }
    }

    let _guard = EnvGuard {
        key: CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR,
        previous: std::env::var(CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR).ok(),
    };
    // SAFETY: this test is serialized with other tests that mutate this env var.
    unsafe {
        std::env::set_var(
            CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR,
            "codex_env_override",
        );
    }

    let identity = ClientIdentity::explicit(
        "codex_explicit_client".to_string(),
        /*user_agent_suffix*/ None,
    )
    .expect("explicit identity should be valid");

    assert_eq!(identity.originator_value(), "codex_explicit_client");
    assert!(identity.user_agent().starts_with("codex_explicit_client/"));
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

#[tokio::test]
async fn test_create_client_sets_default_headers() {
    skip_if_no_network!();

    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    let client = {
        let _guard = ResidencyGuard::set(Some(ResidencyRequirement::Us));
        create_client()
    };

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
