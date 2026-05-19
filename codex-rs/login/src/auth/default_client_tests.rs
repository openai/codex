use super::sanitize_user_agent;
use super::*;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serial_test::serial;

#[test]
fn test_get_codex_user_agent() {
    let originator = Originator::process_default();
    let user_agent = get_codex_user_agent(&originator);
    let prefix = format!("{}/", originator.value());
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

#[tokio::test]
async fn test_create_client_sets_default_headers() {
    skip_if_no_network!();

    set_default_client_residency_requirement(Some(ResidencyRequirement::Us));

    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    let originator = Originator::process_default();
    let client = create_client(&originator);

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
    assert_eq!(originator_header.to_str().unwrap(), originator.value());

    // User-Agent matches the computed Codex UA for that originator
    let expected_ua = get_codex_user_agent(&originator);
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

#[test]
fn app_server_originator_builds_explicit_headers() {
    let originator =
        Originator::from_app_server_client("codex_ios".to_string(), "1.2.3".to_string())
            .expect("originator should be valid");

    let headers = default_headers(&originator);
    assert_eq!(
        headers
            .get("originator")
            .expect("originator header missing")
            .to_str()
            .expect("originator should be valid"),
        "codex_ios"
    );
    assert_eq!(
        originator.app_server_client().map(AppServerClient::name),
        Some("codex_ios")
    );
    assert_eq!(
        originator.app_server_client().map(AppServerClient::version),
        Some("1.2.3")
    );
    assert!(
        headers
            .get("user-agent")
            .expect("user-agent header missing")
            .to_str()
            .expect("user-agent should be valid")
            .starts_with("codex_ios/")
    );
    assert!(get_codex_user_agent(&originator).contains("(codex_ios; 1.2.3)"));
}

#[test]
fn process_originator_does_not_add_user_agent_suffix() {
    let originator =
        Originator::for_process("codex_exec".to_string()).expect("originator should be valid");

    assert_eq!(originator.value(), "codex_exec");
    assert_eq!(originator.app_server_client(), None);
    assert!(!get_codex_user_agent(&originator).contains("(codex_exec"));
}

#[test]
#[serial(originator_env)]
fn process_default_reads_originator_override() {
    let _guard = EnvVarGuard::set(CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR, "codex_override");

    let originator = Originator::process_default();

    assert_eq!(originator.value(), "codex_override");
}

#[test]
#[serial(originator_env)]
fn app_server_originator_ignores_originator_override() {
    let _guard = EnvVarGuard::set(CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR, "codex_override");

    let originator =
        Originator::from_app_server_client("codex_ios".to_string(), "1.2.3".to_string())
            .expect("originator should be valid");

    assert_eq!(originator.value(), "codex_ios");
}

#[test]
#[serial(originator_env)]
fn invalid_process_default_override_falls_back() {
    let _guard = EnvVarGuard::set(CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR, "bad\rvalue");

    let originator = Originator::process_default();

    assert_eq!(originator.value(), DEFAULT_ORIGINATOR);
}

#[test]
fn invalid_originator_values_are_rejected() {
    assert_eq!(
        Originator::for_process("bad\rvalue".to_string()),
        Err(InvalidOriginator::InvalidHeaderValue)
    );
    assert_eq!(
        Originator::from_app_server_client("bad\rvalue".to_string(), "1.2.3".to_string()),
        Err(InvalidOriginator::InvalidHeaderValue)
    );
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
    let originator = Originator::process_default();
    let user_agent = get_codex_user_agent(&originator);
    let originator = regex_lite::escape(originator.value());
    let re = Regex::new(&format!(
        r"^{originator}/\d+\.\d+\.\d+ \(Mac OS \d+\.\d+\.\d+; (x86_64|arm64)\) (\S+)$"
    ))
    .unwrap();
    assert!(re.is_match(&user_agent));
}

struct EnvVarGuard {
    key: &'static str,
    original: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let original = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.original {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }
}
