use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::ChatGptIdTokenClaims;
use app_test_support::TestAppServer;
use app_test_support::encode_id_token;
use app_test_support::write_chatgpt_auth;
use app_test_support::write_models_cache;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_config::types::AuthCredentialsStoreMode;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;

const TEST_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 30);
const INITIAL_ACCOUNT_ID: &str = "123e4567-e89b-42d3-a456-426614174000";
const NEXT_ACCOUNT_ID: &str = "123e4567-e89b-42d3-a456-426614174001";
const INITIAL_EMAIL: &str = "initial-workspace@example.com";
const NEXT_EMAIL: &str = "next-account@example.com";
const PARENT_TRACE_ID: &str = "4bf92f3577b34da6a3ce929d0e0e4736";
const PARENT_TRACEPARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

#[tokio::test]
async fn account_switch_reloads_telemetry_collectors_and_preserves_trace_context() -> Result<()> {
    let collector = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&collector)
        .await;

    let codex_home = TempDir::new()?;
    let initial_endpoint = format!("{}/initial", collector.uri());
    write_otel_config(codex_home.path(), &initial_endpoint)?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("initial-access-token")
            .account_id(INITIAL_ACCOUNT_ID)
            .chatgpt_account_id(INITIAL_ACCOUNT_ID)
            .chatgpt_user_id("initial-user")
            .plan_type("pro")
            .email(INITIAL_EMAIL),
        AuthCredentialsStoreMode::File,
    )?;
    write_models_cache(codex_home.path())?;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("TRACEPARENT", Some(PARENT_TRACEPARENT))])
        .with_json_logging("codex_app_server::otel_reloader=info")
        .build_initialized_with_timeout(TEST_TIMEOUT)
        .await?;
    app_server
        .start_thread(ThreadStartParams::default())
        .await?;

    let next_endpoint = format!("{}/next", collector.uri());
    write_otel_config(codex_home.path(), &next_endpoint)?;
    let access_token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .email(NEXT_EMAIL)
            .plan_type("pro")
            .chatgpt_account_id(NEXT_ACCOUNT_ID)
            .chatgpt_user_id("next-user"),
    )?;
    let request_id = app_server
        .send_chatgpt_auth_tokens_login_request(
            access_token,
            NEXT_ACCOUNT_ID.to_string(),
            Some("pro".to_string()),
        )
        .await?;
    let response: LoginAccountResponse =
        timeout(TEST_TIMEOUT, app_server.read_response(request_id)).await??;
    assert_eq!(response, LoginAccountResponse::ChatgptAuthTokens {});
    timeout(
        TEST_TIMEOUT,
        app_server.wait_for_json_log_event("codex.app_server.otel_reloaded"),
    )
    .await??;
    app_server
        .start_thread(ThreadStartParams::default())
        .await?;

    let status = timeout(TEST_TIMEOUT, app_server.shutdown_gracefully()).await??;
    assert!(status.success(), "app-server did not shut down cleanly");

    let requests = collector
        .received_requests()
        .await
        .context("collector did not record requests")?;
    let exported = |path: &str| {
        requests
            .iter()
            .filter(|request| request.url.path() == path)
            .map(|request| String::from_utf8_lossy(&request.body).into_owned())
            .collect::<Vec<_>>()
            .join("\n")
    };
    let initial_logs = exported("/initial/logs");
    let initial_traces = exported("/initial/traces");
    let next_logs = exported("/next/logs");
    let next_traces = exported("/next/traces");
    let next_metrics = exported("/next/metrics");

    assert!(
        initial_logs.contains(INITIAL_EMAIL),
        "the initial account's logs were not exported: {initial_logs}"
    );
    assert!(
        initial_traces.contains(PARENT_TRACE_ID),
        "the initial account's trace context was not propagated: {initial_traces}"
    );
    assert!(
        next_logs.contains(NEXT_EMAIL),
        "the next account's logs did not reach its collector: {next_logs}"
    );
    assert!(
        next_traces.contains(PARENT_TRACE_ID),
        "the next account's trace context was not propagated: {next_traces}"
    );
    assert!(
        next_metrics.contains("codex.thread.started"),
        "the next account's metrics did not reach its collector: {next_metrics}"
    );

    Ok(())
}

fn write_otel_config(codex_home: &Path, collector_endpoint: &str) -> Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"model = "mock-model"
model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider"
base_url = "http://127.0.0.1:1/v1"
wire_api = "responses"

[analytics]
enabled = true

[otel]
environment = "test"
exporter = {{ otlp-http = {{ endpoint = "{collector_endpoint}/logs", protocol = "json" }} }}
trace_exporter = {{ otlp-http = {{ endpoint = "{collector_endpoint}/traces", protocol = "json" }} }}
metrics_exporter = {{ otlp-http = {{ endpoint = "{collector_endpoint}/metrics", protocol = "json" }} }}
"#
        ),
    )?;
    Ok(())
}
