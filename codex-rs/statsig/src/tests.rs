use super::*;
use base64::Engine;
use codex_login::AuthCredentialsStoreMode;
use codex_login::CodexAuth;
use pretty_assertions::assert_eq;
use statsig_rust::log_event_payload::LogEventPayload;
use statsig_rust::networking::NetworkError;
use statsig_rust::networking::ResponseData;
use std::fs;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

#[test]
fn builds_workspace_chatgpt_user_from_available_metadata() {
    let user = make_statsig_user(StatsigUserMetadata {
        auth_status: StatsigUserAuthStatus::LoggedIn,
        user_id: Some("user-123".to_string()),
        email: Some("dev@openai.com".to_string()),
        account_id: Some("account-123".to_string()),
        plan_type: Some("business".to_string()),
        is_workspace_account: true,
        app_version: Some("1.2.3".to_string()),
        user_agent: Some("codex-cli/test".to_string()),
        statsig_environment: Some(HashMap::from([(
            "tier".to_string(),
            "production".to_string(),
        )])),
    });

    assert_eq!(user.get_user_id(), Some("user-123"));
    assert_eq!(user.get_email(), None);
    assert_eq!(user.get_app_version(), Some("1.2.3"));
    assert_eq!(user.get_user_agent(), Some("codex-cli/test"));
    assert_eq!(
        user.get_custom_ids(),
        Some(HashMap::from([
            (custom_id_keys::ACCOUNT_ID, "account-123"),
            (custom_id_keys::WORKSPACE_ID, "account-123"),
        ]))
    );
    assert_eq!(
        user.get_statsig_environment(),
        Some(HashMap::from([("tier", "production")]))
    );
    assert_eq!(
        user.get_custom(),
        Some(&HashMap::from([
            (
                custom_keys::AUTH_STATUS.to_string(),
                dyn_value!("logged_in"),
            ),
            (
                custom_keys::EMAIL_DOMAIN_TYPE.to_string(),
                dyn_value!("professional"),
            ),
            (custom_keys::PLAN_TYPE.to_string(), dyn_value!("business")),
            (
                custom_keys::ACCOUNT_ID.to_string(),
                dyn_value!("account-123")
            ),
            (
                custom_keys::WORKSPACE_ID.to_string(),
                dyn_value!("account-123")
            ),
            (
                custom_keys::USER_AGENT.to_string(),
                dyn_value!("codex-cli/test"),
            ),
        ]))
    );
    assert_eq!(user.get_private_attributes(), None);
}

#[test]
fn default_metadata_keeps_always_on_traits() {
    let user = make_statsig_user(StatsigUserMetadata::default());

    assert_eq!(user.get_user_id(), None);
    assert_eq!(user.get_email(), None);
    assert_eq!(user.get_custom_ids(), None);
    assert_eq!(user.get_private_attributes(), None);
    assert_eq!(
        user.get_custom(),
        Some(&HashMap::from([
            (
                custom_keys::AUTH_STATUS.to_string(),
                dyn_value!("logged_out"),
            ),
            (
                custom_keys::EMAIL_DOMAIN_TYPE.to_string(),
                dyn_value!("missing"),
            ),
        ]))
    );
}

#[test]
fn personal_account_sets_account_ids_without_workspace_custom_trait() {
    let user = make_statsig_user(StatsigUserMetadata {
        auth_status: StatsigUserAuthStatus::LoggedIn,
        user_id: Some("user-123".to_string()),
        email: Some("user@gmail.com".to_string()),
        account_id: Some("account-123".to_string()),
        plan_type: Some("free".to_string()),
        ..StatsigUserMetadata::default()
    });

    assert_eq!(
        user.get_custom_ids(),
        Some(HashMap::from([
            (custom_id_keys::ACCOUNT_ID, "account-123"),
            (custom_id_keys::WORKSPACE_ID, "account-123"),
        ]))
    );
    assert_eq!(
        user.get_custom(),
        Some(&HashMap::from([
            (
                custom_keys::AUTH_STATUS.to_string(),
                dyn_value!("logged_in"),
            ),
            (
                custom_keys::EMAIL_DOMAIN_TYPE.to_string(),
                dyn_value!("social"),
            ),
            (custom_keys::PLAN_TYPE.to_string(), dyn_value!("free")),
            (
                custom_keys::ACCOUNT_ID.to_string(),
                dyn_value!("account-123")
            ),
        ]))
    );
}

#[test]
fn account_metadata_and_missing_user_id_stays_logged_out() {
    let user = make_statsig_user(StatsigUserMetadata {
        email: Some("user@example.com".to_string()),
        account_id: Some("account-123".to_string()),
        plan_type: Some("mystery-tier".to_string()),
        ..StatsigUserMetadata::default()
    });

    assert_eq!(user.get_user_id(), None);
    assert_eq!(
        user.get_custom(),
        Some(&HashMap::from([
            (
                custom_keys::AUTH_STATUS.to_string(),
                dyn_value!("logged_out"),
            ),
            (
                custom_keys::EMAIL_DOMAIN_TYPE.to_string(),
                dyn_value!("professional"),
            ),
            (
                custom_keys::PLAN_TYPE.to_string(),
                dyn_value!("mystery-tier")
            ),
            (
                custom_keys::ACCOUNT_ID.to_string(),
                dyn_value!("account-123")
            ),
        ]))
    );
}

#[test]
fn account_without_plan_type_keeps_account_id() {
    let user = make_statsig_user(StatsigUserMetadata {
        account_id: Some("account-123".to_string()),
        ..StatsigUserMetadata::default()
    });

    assert_eq!(
        user.get_custom(),
        Some(&HashMap::from([
            (
                custom_keys::AUTH_STATUS.to_string(),
                dyn_value!("logged_out"),
            ),
            (
                custom_keys::EMAIL_DOMAIN_TYPE.to_string(),
                dyn_value!("missing"),
            ),
            (
                custom_keys::ACCOUNT_ID.to_string(),
                dyn_value!("account-123")
            ),
        ]))
    );
}

#[test]
fn auth_with_token_data_maps_available_claims() {
    let auth = chatgpt_auth_from_payload(
        serde_json::json!({
            "email": "Dev@OpenAI.COM",
            "https://api.openai.com/auth": {
                "chatgpt_user_id": "user-123",
                "chatgpt_account_id": "account-123",
                "chatgpt_plan_type": "business"
            }
        }),
        Some("account-123"),
    );

    assert_eq!(
        StatsigUserMetadata::from_auth(Some(&auth)),
        StatsigUserMetadata {
            auth_status: StatsigUserAuthStatus::LoggedIn,
            user_id: Some("user-123".to_string()),
            email: Some("Dev@OpenAI.COM".to_string()),
            account_id: Some("account-123".to_string()),
            plan_type: Some("business".to_string()),
            is_workspace_account: true,
            ..StatsigUserMetadata::default()
        }
    );
}

#[test]
fn auth_with_account_but_missing_user_id_stays_logged_out() {
    let auth = chatgpt_auth_from_payload(
        serde_json::json!({
            "email": "user@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "account-123",
                "chatgpt_plan_type": "business"
            }
        }),
        Some("account-123"),
    );

    let metadata = StatsigUserMetadata::from_auth(Some(&auth));
    assert_eq!(
        metadata,
        StatsigUserMetadata {
            auth_status: StatsigUserAuthStatus::LoggedOut,
            email: Some("user@example.com".to_string()),
            account_id: Some("account-123".to_string()),
            plan_type: Some("business".to_string()),
            is_workspace_account: true,
            ..StatsigUserMetadata::default()
        }
    );

    let user = make_statsig_user(metadata);
    assert_eq!(user.get_user_id(), None);
    assert_eq!(
        user.get_custom(),
        Some(&HashMap::from([
            (
                custom_keys::AUTH_STATUS.to_string(),
                dyn_value!("logged_out"),
            ),
            (
                custom_keys::EMAIL_DOMAIN_TYPE.to_string(),
                dyn_value!("professional"),
            ),
            (custom_keys::PLAN_TYPE.to_string(), dyn_value!("business")),
            (
                custom_keys::ACCOUNT_ID.to_string(),
                dyn_value!("account-123")
            ),
            (
                custom_keys::WORKSPACE_ID.to_string(),
                dyn_value!("account-123")
            ),
        ]))
    );
}

#[test]
fn email_domain_type_classification_normalizes_case() {
    let user = make_statsig_user(StatsigUserMetadata {
        email: Some("Dev@OpenAI.COM".to_string()),
        account_id: Some("account-123".to_string()),
        plan_type: Some("FREE".to_string()),
        ..StatsigUserMetadata::default()
    });

    assert_eq!(
        user.get_custom(),
        Some(&HashMap::from([
            (
                custom_keys::AUTH_STATUS.to_string(),
                dyn_value!("logged_out"),
            ),
            (
                custom_keys::EMAIL_DOMAIN_TYPE.to_string(),
                dyn_value!("professional"),
            ),
            (custom_keys::PLAN_TYPE.to_string(), dyn_value!("FREE")),
            (
                custom_keys::ACCOUNT_ID.to_string(),
                dyn_value!("account-123")
            ),
        ]))
    );
}

#[test]
fn classifies_email_domain_types() {
    assert_eq!(
        [
            get_email_domain_type(""),
            get_email_domain_type("missing-at-sign"),
            get_email_domain_type("user@gmail.com"),
            get_email_domain_type("user@agency.gov"),
            get_email_domain_type("user@students.example.edu"),
            get_email_domain_type("user@openai.com"),
        ],
        [
            "missing",
            "unknown",
            "social",
            "government",
            "edu",
            "professional",
        ]
    );
}

#[test]
fn statsig_options_use_ces_url_and_analytics_network_setting() {
    let enabled = make_statsig_options("secret-key", None, AnalyticsMode::Enabled);
    assert_eq!(
        enabled.log_event_url,
        Some(DEFAULT_CES_STATSIG_LOG_EVENT_URL.to_string())
    );
    assert_eq!(enabled.disable_network, Some(false));
    assert_eq!(enabled.disable_all_logging, Some(false));
    assert!(enabled.event_logging_adapter.is_some());

    let disabled = make_statsig_options("secret-key", None, AnalyticsMode::Disabled);
    assert_eq!(disabled.disable_network, Some(true));
    assert_eq!(disabled.disable_all_logging, Some(true));
    assert!(disabled.event_logging_adapter.is_some());
}

#[test]
fn ces_headers_include_chatgpt_auth_but_not_api_key_auth() {
    let auth = chatgpt_auth_from_payload(
        serde_json::json!({
            "email": "user@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_user_id": "user-123",
                "chatgpt_account_id": "account-123"
            }
        }),
        Some("account-123"),
    );
    let options = StatsigOptions::new();
    let chatgpt_headers = statsig_ces_headers("secret-key", &options, Some(&auth));
    assert_eq!(
        (
            chatgpt_headers.get(AUTHORIZATION_HEADER_NAME),
            chatgpt_headers.get(CHATGPT_ACCOUNT_ID_HEADER_NAME),
        ),
        (
            Some(&"Bearer test-access-token".to_string()),
            Some(&"account-123".to_string())
        )
    );

    let api_key_auth = CodexAuth::from_api_key("test-api-key");
    let api_key_headers = statsig_ces_headers("secret-key", &options, Some(&api_key_auth));
    assert_eq!(
        (
            api_key_headers.get(AUTHORIZATION_HEADER_NAME),
            api_key_headers.get(CHATGPT_ACCOUNT_ID_HEADER_NAME),
        ),
        (None, None)
    );
}

#[test]
fn ces_log_event_response_accepts_empty_success_body() {
    assert!(matches!(
        ensure_log_event_response_success(ResponseData::from_bytes(Vec::new())),
        Ok(())
    ));
}

#[test]
fn ces_log_event_response_rejects_success_false_body() {
    assert!(matches!(
        ensure_log_event_response_success(ResponseData::from_bytes(
            br#"{ "success": false }"#.to_vec()
        )),
        Err(StatsigErr::LogEventError(_))
    ));
}

#[tokio::test]
async fn ces_adapter_posts_to_configured_url_with_chatgpt_auth_headers() {
    let auth = chatgpt_auth_from_payload(
        serde_json::json!({
            "email": "user@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_user_id": "user-123",
                "chatgpt_account_id": "account-123"
            }
        }),
        Some("account-123"),
    );
    let server = TestServer::start(204, "");

    let options = make_statsig_options_with_log_event_url(
        "secret-key",
        Some(&auth),
        AnalyticsMode::Enabled,
        server.url(),
    );
    let adapter = options
        .event_logging_adapter
        .as_ref()
        .expect("event logging adapter should be set");

    assert!(matches!(
        adapter.log_events(log_event_request()).await,
        Ok(true)
    ));
    let request = server.request();
    let lower_request = request.to_ascii_lowercase();
    assert!(request.starts_with("POST /v1/rgstr HTTP/1.1"));
    assert!(lower_request.contains("authorization: bearer test-access-token"));
    assert!(lower_request.contains("chatgpt-account-id: account-123"));
    assert!(lower_request.contains("statsig-api-key: secret-key"));
}

#[tokio::test]
async fn ces_adapter_respects_disabled_analytics_network_setting() {
    let server = TestServer::start(204, "");
    let options = make_statsig_options_with_log_event_url(
        "secret-key",
        None,
        AnalyticsMode::Disabled,
        server.url(),
    );
    let adapter = options
        .event_logging_adapter
        .as_ref()
        .expect("event logging adapter should be set");

    assert!(matches!(
        adapter.log_events(log_event_request()).await,
        Err(StatsigErr::NetworkError(NetworkError::DisableNetworkOn(_)))
    ));
    assert_eq!(server.try_request(), None);
}

#[test]
fn auth_without_token_data_maps_to_logged_out_metadata() {
    let auth = CodexAuth::from_api_key("test-api-key");

    assert_eq!(
        StatsigUserMetadata::from_auth(Some(&auth)),
        StatsigUserMetadata::default()
    );
}

fn chatgpt_auth_from_payload(payload: serde_json::Value, account_id: Option<&str>) -> CodexAuth {
    let temp_dir = tempfile::tempdir().expect("tempdir should be created");
    let fake_jwt = fake_jwt(payload);
    let auth_json = serde_json::json!({
        "tokens": {
            "id_token": fake_jwt,
            "access_token": "test-access-token",
            "refresh_token": "test-refresh-token",
            "account_id": account_id,
        },
        "last_refresh": "2026-01-01T00:00:00Z",
    });
    fs::write(
        temp_dir.path().join("auth.json"),
        serde_json::to_vec(&auth_json).expect("auth json should serialize"),
    )
    .expect("auth file should be written");

    CodexAuth::from_auth_storage(temp_dir.path(), AuthCredentialsStoreMode::File)
        .expect("auth storage should read")
        .expect("auth should exist")
}

fn log_event_request() -> LogEventRequest {
    LogEventRequest {
        payload: LogEventPayload {
            events: serde_json::json!([]),
            statsig_metadata: serde_json::json!({}),
        },
        event_count: 0,
        retries: 0,
    }
}

struct TestServer {
    url: String,
    request_rx: mpsc::Receiver<String>,
}

impl TestServer {
    fn start(status_code: u16, body: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
        listener
            .set_nonblocking(false)
            .expect("test server should stay blocking");
        let url = format!(
            "http://{}/v1/rgstr",
            listener.local_addr().expect("local addr")
        );
        let (request_tx, request_rx) = mpsc::channel();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("test request should connect");
            let mut buffer = [0; 8192];
            let bytes_read = stream.read(&mut buffer).expect("test request should read");
            request_tx
                .send(String::from_utf8_lossy(&buffer[..bytes_read]).to_string())
                .expect("test request should be sent");
            let response = format!(
                "HTTP/1.1 {status_code} OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("test response should write");
        });
        Self { url, request_rx }
    }

    fn url(&self) -> String {
        self.url.clone()
    }

    fn request(self) -> String {
        self.request_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("test request should be received")
    }

    fn try_request(self) -> Option<String> {
        self.request_rx
            .recv_timeout(std::time::Duration::from_millis(100))
            .ok()
    }
}

fn fake_jwt(payload: serde_json::Value) -> String {
    let header = serde_json::json!({
        "alg": "none",
        "typ": "JWT",
    });
    let b64url_no_pad =
        |bytes: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    let header_b64 =
        b64url_no_pad(&serde_json::to_vec(&header).expect("jwt header should serialize"));
    let payload_b64 =
        b64url_no_pad(&serde_json::to_vec(&payload).expect("jwt payload should serialize"));
    let signature_b64 = b64url_no_pad(b"sig");
    format!("{header_b64}.{payload_b64}.{signature_b64}")
}
