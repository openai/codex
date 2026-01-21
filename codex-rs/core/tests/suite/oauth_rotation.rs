use codex_core::AuthManager;
use codex_core::ContentItem;
use codex_core::ModelClient;
use codex_core::ModelProviderInfo;
use codex_core::Prompt;
use codex_core::ResponseEvent;
use codex_core::ResponseItem;
use codex_core::WireApi;
use codex_core::config::Config;
use codex_core::auth::AuthCredentialsStoreMode;
use codex_core::auth::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR;
use codex_core::auth::add_oauth_account;
use codex_core::auth::list_oauth_accounts;
use codex_core::built_in_model_providers;
use codex_core::error::CodexErr;
use codex_core::token_data::IdTokenInfo;
use codex_core::token_data::TokenData;
use codex_core::models_manager::manager::ModelsManager;
use codex_otel::OtelManager;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use core_test_support::load_default_config_for_test;
use core_test_support::load_sse_fixture_with_id;
use futures::StreamExt;
use httpdate::fmt_http_date;
use http::StatusCode;
use base64::Engine;
use serial_test::serial;
use std::ffi::OsString;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn sse_completed(id: &str) -> String {
    load_sse_fixture_with_id("../fixtures/completed_template.json", id)
}

fn token_data(access: &str, refresh: &str) -> TokenData {
    let mut id_token = IdTokenInfo::default();
    id_token.raw_jwt = minimal_jwt();
    TokenData {
        id_token,
        access_token: access.to_string(),
        refresh_token: refresh.to_string(),
        account_id: None,
    }
}

fn minimal_jwt() -> String {
    #[derive(serde::Serialize)]
    struct Header {
        alg: &'static str,
        typ: &'static str,
    }
    let header = Header { alg: "none", typ: "JWT" };
    let payload = serde_json::json!({ "sub": "user-123" });

    fn b64(data: &[u8]) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
    }

    let header_b64 = b64(&serde_json::to_vec(&header).expect("serialize header"));
    let payload_b64 = b64(&serde_json::to_vec(&payload).expect("serialize payload"));
    let signature_b64 = b64(b"sig");
    format!("{header_b64}.{payload_b64}.{signature_b64}")
}

async fn build_client_session(
    codex_home: &TempDir,
    provider: ModelProviderInfo,
    auth_manager: Arc<AuthManager>,
) -> codex_core::ModelClientSession {
    build_client_session_with_config(codex_home, provider, auth_manager, |_| {}).await
}

async fn build_client_session_with_config<F>(
    codex_home: &TempDir,
    provider: ModelProviderInfo,
    auth_manager: Arc<AuthManager>,
    configure: F,
) -> codex_core::ModelClientSession
where
    F: FnOnce(&mut Config),
{
    let mut config = load_default_config_for_test(codex_home).await;
    configure(&mut config);
    config.model_provider_id = provider.name.clone();
    config.model_provider = provider.clone();
    let effort = config.model_reasoning_effort;
    let summary = config.model_reasoning_summary;
    let model = ModelsManager::get_model_offline(config.model.as_deref());
    config.model = Some(model.clone());
    let config = Arc::new(config);
    let model_info = ModelsManager::construct_model_info_offline(model.as_str(), &config);
    let conversation_id = ThreadId::new();
    let otel_manager = OtelManager::new(
        conversation_id,
        model.as_str(),
        model_info.slug.as_str(),
        None,
        Some("test@test.com".to_string()),
        auth_manager.get_auth_mode(),
        false,
        "test".to_string(),
        SessionSource::Exec,
    );

    ModelClient::new(
        Arc::clone(&config),
        Some(auth_manager),
        model_info,
        otel_manager,
        provider,
        effort,
        summary,
        conversation_id,
        SessionSource::Exec,
    )
    .new_session()
}

fn sample_prompt() -> Prompt {
    let mut prompt = Prompt::default();
    prompt.input = vec![ResponseItem::Message {
        id: None,
        role: "user".into(),
        content: vec![ContentItem::InputText {
            text: "hello".into(),
        }],
    }];
    prompt
}

async fn drain_stream(mut stream: codex_core::ResponseStream) {
    while let Some(event) = stream.next().await {
        if matches!(event, Ok(ResponseEvent::Completed { .. })) {
            break;
        }
    }
}

fn openai_provider(server: &MockServer) -> ModelProviderInfo {
    let mut provider = built_in_model_providers()["openai"].clone();
    provider.base_url = Some(format!("{}/v1", server.uri()));
    provider.request_max_retries = Some(0);
    provider.stream_max_retries = Some(0);
    provider.stream_idle_timeout_ms = Some(5_000);
    provider.wire_api = WireApi::Responses;
    provider
}

struct EnvGuard {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: String) -> Self {
        let original = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, &value);
        }
        Self { key, original }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.original {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rotates_on_429_and_uses_next_account() {
    let server = MockServer::start().await;

    let a1_access = "access-1";
    let a2_access = "access-2";

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", format!("Bearer {a1_access}")))
        .respond_with(
            ResponseTemplate::new(429).insert_header("Retry-After", "1"),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", format!("Bearer {a2_access}")))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            sse_completed("resp2"),
            "text/event-stream",
        ))
        .expect(1)
        .mount(&server)
        .await;

    let codex_home = TempDir::new().expect("temp dir");
    let record1 = add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data(a1_access, "refresh-1"),
        None,
        None,
    )
    .expect("add account 1");
    let record2 = add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data(a2_access, "refresh-2"),
        None,
        None,
    )
    .expect("add account 2");

    let auth_manager = AuthManager::shared(
        codex_home.path().to_path_buf(),
        false,
        AuthCredentialsStoreMode::File,
    );
    let provider = openai_provider(&server);
    let mut client_session =
        build_client_session(&codex_home, provider, Arc::clone(&auth_manager)).await;

    let prompt = sample_prompt();
    let stream = client_session.stream(&prompt).await.expect("stream");
    drain_stream(stream).await;

    let accounts = list_oauth_accounts(codex_home.path(), AuthCredentialsStoreMode::File)
        .expect("list accounts");
    let ordered_ids: Vec<String> = accounts.iter().map(|a| a.record_id.clone()).collect();
    assert_eq!(ordered_ids, vec![record2, record1]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(auth_refresh)]
async fn refreshes_on_401_and_reuses_same_account() {
    let server = MockServer::start().await;
    let _env = EnvGuard::set(
        REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR,
        format!("{}/oauth/token", server.uri()),
    );

    let bad_access = "access-bad";
    let refreshed_access = "access-refreshed";

    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": refreshed_access,
            "refresh_token": "refresh-refreshed",
            "id_token": minimal_jwt(),
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", format!("Bearer {bad_access}")))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", format!("Bearer {refreshed_access}")))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            sse_completed("resp1"),
            "text/event-stream",
        ))
        .expect(1)
        .mount(&server)
        .await;

    let codex_home = TempDir::new().expect("temp dir");
    let record1 = add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data(bad_access, "refresh-1"),
        None,
        None,
    )
    .expect("add account 1");
    let record2 = add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data("access-2", "refresh-2"),
        None,
        None,
    )
    .expect("add account 2");

    let auth_manager = AuthManager::shared(
        codex_home.path().to_path_buf(),
        false,
        AuthCredentialsStoreMode::File,
    );
    let provider = openai_provider(&server);
    let mut client_session =
        build_client_session(&codex_home, provider, Arc::clone(&auth_manager)).await;

    let prompt = sample_prompt();
    let stream = client_session.stream(&prompt).await.expect("stream");
    drain_stream(stream).await;

    let accounts = list_oauth_accounts(codex_home.path(), AuthCredentialsStoreMode::File)
        .expect("list accounts");
    let ordered_ids: Vec<String> = accounts.iter().map(|a| a.record_id.clone()).collect();
    assert_eq!(ordered_ids, vec![record1.clone(), record2]);

    let refreshed_auth = auth_manager
        .auth_for_record(&record1)
        .expect("record should exist");
    let refreshed_tokens = refreshed_auth
        .get_token_data()
        .expect("token data should exist");
    assert_eq!(refreshed_tokens.access_token, refreshed_access);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(auth_refresh)]
async fn refreshes_on_403_and_reuses_same_account() {
    let server = MockServer::start().await;
    let _env = EnvGuard::set(
        REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR,
        format!("{}/oauth/token", server.uri()),
    );

    let bad_access = "access-forbidden";
    let refreshed_access = "access-refreshed-403";

    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": refreshed_access,
            "refresh_token": "refresh-refreshed-403",
            "id_token": minimal_jwt(),
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", format!("Bearer {bad_access}")))
        .respond_with(ResponseTemplate::new(403))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", format!("Bearer {refreshed_access}")))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            sse_completed("resp403"),
            "text/event-stream",
        ))
        .expect(1)
        .mount(&server)
        .await;

    let codex_home = TempDir::new().expect("temp dir");
    let record1 = add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data(bad_access, "refresh-403"),
        None,
        None,
    )
    .expect("add account 1");
    add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data("access-2", "refresh-2"),
        None,
        None,
    )
    .expect("add account 2");

    let auth_manager = AuthManager::shared(
        codex_home.path().to_path_buf(),
        false,
        AuthCredentialsStoreMode::File,
    );
    let provider = openai_provider(&server);
    let mut client_session =
        build_client_session(&codex_home, provider, Arc::clone(&auth_manager)).await;

    let prompt = sample_prompt();
    let stream = client_session.stream(&prompt).await.expect("stream");
    drain_stream(stream).await;

    let accounts = list_oauth_accounts(codex_home.path(), AuthCredentialsStoreMode::File)
        .expect("list accounts");
    let ordered_ids: Vec<String> = accounts.iter().map(|a| a.record_id.clone()).collect();
    assert_eq!(ordered_ids.first().map(String::as_str), Some(record1.as_str()));

    let refreshed_auth = auth_manager
        .auth_for_record(&record1)
        .expect("record should exist");
    let refreshed_tokens = refreshed_auth
        .get_token_data()
        .expect("token data should exist");
    assert_eq!(refreshed_tokens.access_token, refreshed_access);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(auth_refresh)]
async fn rotates_when_refresh_fails() {
    let server = MockServer::start().await;
    let _env = EnvGuard::set(
        REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR,
        format!("{}/oauth/token", server.uri()),
    );

    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": { "code": "refresh_token_expired" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", "Bearer access-bad"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", "Bearer access-good"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            sse_completed("resp2"),
            "text/event-stream",
        ))
        .expect(1)
        .mount(&server)
        .await;

    let codex_home = TempDir::new().expect("temp dir");
    let record1 = add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data("access-bad", "refresh-1"),
        None,
        None,
    )
    .expect("add account 1");
    let record2 = add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data("access-good", "refresh-2"),
        None,
        None,
    )
    .expect("add account 2");

    let auth_manager = AuthManager::shared(
        codex_home.path().to_path_buf(),
        false,
        AuthCredentialsStoreMode::File,
    );
    let provider = openai_provider(&server);
    let mut client_session =
        build_client_session(&codex_home, provider, Arc::clone(&auth_manager)).await;

    let prompt = sample_prompt();
    let stream = client_session.stream(&prompt).await.expect("stream");
    drain_stream(stream).await;

    let accounts = list_oauth_accounts(codex_home.path(), AuthCredentialsStoreMode::File)
        .expect("list accounts");
    let ordered_ids: Vec<String> = accounts.iter().map(|a| a.record_id.clone()).collect();
    assert_eq!(ordered_ids, vec![record2, record1]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rotates_on_non_auth_failure() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", "Bearer access-1"))
        .respond_with(ResponseTemplate::new(402))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", "Bearer access-2"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            sse_completed("resp2"),
            "text/event-stream",
        ))
        .expect(1)
        .mount(&server)
        .await;

    let codex_home = TempDir::new().expect("temp dir");
    let record1 = add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data("access-1", "refresh-1"),
        None,
        None,
    )
    .expect("add account 1");
    let record2 = add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data("access-2", "refresh-2"),
        None,
        None,
    )
    .expect("add account 2");

    let auth_manager = AuthManager::shared(
        codex_home.path().to_path_buf(),
        false,
        AuthCredentialsStoreMode::File,
    );
    let provider = openai_provider(&server);
    let mut client_session =
        build_client_session(&codex_home, provider, Arc::clone(&auth_manager)).await;

    let prompt = sample_prompt();
    let stream = client_session.stream(&prompt).await.expect("stream");
    drain_stream(stream).await;

    let accounts = list_oauth_accounts(codex_home.path(), AuthCredentialsStoreMode::File)
        .expect("list accounts");
    let ordered_ids: Vec<String> = accounts.iter().map(|a| a.record_id.clone()).collect();
    assert_eq!(ordered_ids, vec![record2, record1]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn respects_retry_after_http_date() {
    let server = MockServer::start().await;

    let retry_at = SystemTime::now() + Duration::from_secs(5);
    let retry_header = fmt_http_date(retry_at);

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", "Bearer access-1"))
        .respond_with(
            ResponseTemplate::new(429).insert_header("Retry-After", retry_header),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", "Bearer access-2"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            sse_completed("resp2"),
            "text/event-stream",
        ))
        .expect(1)
        .mount(&server)
        .await;

    let codex_home = TempDir::new().expect("temp dir");
    let record1 = add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data("access-1", "refresh-1"),
        None,
        None,
    )
    .expect("add account 1");
    add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data("access-2", "refresh-2"),
        None,
        None,
    )
    .expect("add account 2");

    let auth_manager = AuthManager::shared(
        codex_home.path().to_path_buf(),
        false,
        AuthCredentialsStoreMode::File,
    );
    let provider = openai_provider(&server);
    let mut client_session =
        build_client_session(&codex_home, provider, Arc::clone(&auth_manager)).await;

    let prompt = sample_prompt();
    let stream = client_session.stream(&prompt).await.expect("stream");
    drain_stream(stream).await;

    let accounts = list_oauth_accounts(codex_home.path(), AuthCredentialsStoreMode::File)
        .expect("list accounts");
    let record = accounts
        .iter()
        .find(|account| account.record_id == record1)
        .expect("record should exist");
    let cooldown = record.health.cooldown_until.expect("cooldown should be set");

    let retry_at = chrono::DateTime::<chrono::Utc>::from(retry_at);
    let delta = cooldown - retry_at;
    assert!(
        delta.num_seconds().abs() <= 5,
        "expected cooldown to be near retry-at (delta {}s)",
        delta.num_seconds()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_retries_apply_per_account() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    drop(listener);

    let codex_home = TempDir::new().expect("temp dir");
    add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data("access-1", "refresh-1"),
        None,
        None,
    )
    .expect("add account 1");
    add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data("access-2", "refresh-2"),
        None,
        None,
    )
    .expect("add account 2");

    let auth_manager = AuthManager::shared(
        codex_home.path().to_path_buf(),
        false,
        AuthCredentialsStoreMode::File,
    );

    let mut provider = built_in_model_providers()["openai"].clone();
    provider.base_url = Some(format!("http://{addr}/v1"));
    provider.request_max_retries = Some(0);
    provider.stream_max_retries = Some(0);
    provider.stream_idle_timeout_ms = Some(1_000);
    provider.wire_api = WireApi::Responses;

    let mut client_session = build_client_session_with_config(
        &codex_home,
        provider,
        Arc::clone(&auth_manager),
        |config| {
            config.oauth_rotation.network_retry_attempts = Some(2);
            config.oauth_rotation.max_attempts = Some(2);
        },
    )
    .await;

    let prompt = sample_prompt();
    let err = match client_session.stream(&prompt).await {
        Ok(_) => panic!("expected error"),
        Err(err) => err,
    };
    match err {
        CodexErr::Stream(_, _)
        | CodexErr::ConnectionFailed(_)
        | CodexErr::ResponseStreamFailed(_)
        | CodexErr::Timeout => {}
        other => panic!("unexpected error: {other:?}"),
    }

    let accounts = list_oauth_accounts(codex_home.path(), AuthCredentialsStoreMode::File)
        .expect("list accounts");
    assert!(
        accounts
            .iter()
            .all(|account| account.health.failure_count == 3),
        "expected each account to record 3 failures, got {accounts:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn errors_when_all_accounts_rate_limited() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", "Bearer access-1"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "1"))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", "Bearer access-2"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "1"))
        .expect(1)
        .mount(&server)
        .await;

    let codex_home = TempDir::new().expect("temp dir");
    add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data("access-1", "refresh-1"),
        None,
        None,
    )
    .expect("add account 1");
    add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data("access-2", "refresh-2"),
        None,
        None,
    )
    .expect("add account 2");

    let auth_manager = AuthManager::shared(
        codex_home.path().to_path_buf(),
        false,
        AuthCredentialsStoreMode::File,
    );
    let provider = openai_provider(&server);
    let mut client_session =
        build_client_session(&codex_home, provider, Arc::clone(&auth_manager)).await;

    let prompt = sample_prompt();
    let err = match client_session.stream(&prompt).await {
        Ok(_) => panic!("expected error"),
        Err(err) => err,
    };
    match err {
        CodexErr::RetryLimit(limit) => {
            assert_eq!(limit.status, StatusCode::TOO_MANY_REQUESTS);
        }
        CodexErr::UsageLimitReached(_) => {}
        other => panic!("unexpected error: {other:?}"),
    }

    let accounts = list_oauth_accounts(codex_home.path(), AuthCredentialsStoreMode::File)
        .expect("list accounts");
    assert!(
        accounts.iter().all(|account| account.health.cooldown_until.is_some()),
        "expected all accounts to enter cooldown"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn single_account_fallback_does_not_set_cooldown() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", "Bearer access-1"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "1"))
        .expect(1)
        .mount(&server)
        .await;

    let codex_home = TempDir::new().expect("temp dir");
    let record1 = add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data("access-1", "refresh-1"),
        None,
        None,
    )
    .expect("add account 1");

    let auth_manager = AuthManager::shared(
        codex_home.path().to_path_buf(),
        false,
        AuthCredentialsStoreMode::File,
    );
    let provider = openai_provider(&server);
    let mut client_session =
        build_client_session(&codex_home, provider, Arc::clone(&auth_manager)).await;

    let prompt = sample_prompt();
    let err = match client_session.stream(&prompt).await {
        Ok(_) => panic!("expected error"),
        Err(err) => err,
    };
    match err {
        CodexErr::RetryLimit(limit) => {
            assert_eq!(limit.status, StatusCode::TOO_MANY_REQUESTS);
        }
        CodexErr::UsageLimitReached(_) => {}
        other => panic!("unexpected error: {other:?}"),
    }

    let accounts = list_oauth_accounts(codex_home.path(), AuthCredentialsStoreMode::File)
        .expect("list accounts");
    assert_eq!(accounts.len(), 1);
    let account = accounts
        .iter()
        .find(|account| account.record_id == record1)
        .expect("record should exist");
    assert!(account.health.cooldown_until.is_none());
    assert_eq!(account.health.failure_count, 0);
}
