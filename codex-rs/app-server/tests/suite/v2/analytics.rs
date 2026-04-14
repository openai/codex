use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::DEFAULT_CLIENT_NAME;
use app_test_support::write_chatgpt_auth;
use codex_config::types::AuthCredentialsStoreMode;
use codex_config::types::OtelExporterKind;
use codex_config::types::OtelHttpProtocol;
use codex_core::config::ConfigBuilder;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const SERVICE_VERSION: &str = "0.0.0-test";

fn set_metrics_exporter(config: &mut codex_core::config::Config) {
    config.otel.metrics_exporter = OtelExporterKind::OtlpHttp {
        endpoint: "http://localhost:4318".to_string(),
        headers: HashMap::new(),
        protocol: OtelHttpProtocol::Json,
        tls: None,
    };
}

#[tokio::test]
async fn app_server_default_analytics_disabled_without_flag() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await?;
    set_metrics_exporter(&mut config);
    config.analytics_enabled = None;

    let provider = codex_core::otel_init::build_provider(
        &config,
        SERVICE_VERSION,
        Some("codex-app-server"),
        /*default_analytics_enabled*/ false,
    )
    .map_err(|err| anyhow::anyhow!(err.to_string()))?;

    // With analytics unset in the config and the default flag is false, metrics are disabled.
    // A provider may still exist for non-metrics telemetry, so check metrics specifically.
    let has_metrics = provider.as_ref().and_then(|otel| otel.metrics()).is_some();
    assert_eq!(has_metrics, false);
    Ok(())
}

#[tokio::test]
async fn app_server_default_analytics_enabled_with_flag() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await?;
    set_metrics_exporter(&mut config);
    config.analytics_enabled = None;

    let provider = codex_core::otel_init::build_provider(
        &config,
        SERVICE_VERSION,
        Some("codex-app-server"),
        /*default_analytics_enabled*/ true,
    )
    .map_err(|err| anyhow::anyhow!(err.to_string()))?;

    // With analytics unset in the config and the default flag is true, metrics are enabled.
    let has_metrics = provider.as_ref().and_then(|otel| otel.metrics()).is_some();
    assert_eq!(has_metrics, true);
    Ok(())
}

pub(crate) async fn enable_analytics_capture(server: &MockServer, codex_home: &Path) -> Result<()> {
    let config_path = codex_home.join("config.toml");
    let config_toml = std::fs::read_to_string(&config_path)?;
    if !config_toml.contains("[features]") {
        std::fs::write(
            &config_path,
            format!("{config_toml}\n[features]\ngeneral_analytics = true\n"),
        )?;
    } else if !config_toml.contains("general_analytics") {
        std::fs::write(
            &config_path,
            config_toml.replace("[features]\n", "[features]\ngeneral_analytics = true\n"),
        )?;
    }

    mount_analytics_capture(server, codex_home).await
}

pub(crate) async fn mount_analytics_capture(server: &MockServer, codex_home: &Path) -> Result<()> {
    Mock::given(method("POST"))
        .and(path("/codex/analytics-events/events"))
        .respond_with(ResponseTemplate::new(200))
        .mount(server)
        .await;

    write_chatgpt_auth(
        codex_home,
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    Ok(())
}

pub(crate) async fn wait_for_thread_initialized_payload(
    server: &MockServer,
    read_timeout: Duration,
) -> Result<Value> {
    timeout(read_timeout, async {
        loop {
            let Some(requests) = server.received_requests().await else {
                tokio::time::sleep(Duration::from_millis(25)).await;
                continue;
            };
            for request in requests.iter().filter(|request| {
                request.method == "POST" && request.url.path() == "/codex/analytics-events/events"
            }) {
                let payload: Value = serde_json::from_slice(&request.body)
                    .map_err(|err| anyhow::anyhow!("invalid analytics payload: {err}"))?;
                if thread_initialized_event(&payload).is_ok() {
                    return Ok(payload);
                }
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await?
}

pub(crate) async fn wait_for_analytics_event(
    server: &MockServer,
    read_timeout: Duration,
    event_type: &str,
) -> Result<Value> {
    timeout(read_timeout, async {
        loop {
            let Some(requests) = server.received_requests().await else {
                tokio::time::sleep(Duration::from_millis(25)).await;
                continue;
            };
            for request in &requests {
                if request.method != "POST"
                    || request.url.path() != "/codex/analytics-events/events"
                {
                    continue;
                }
                let payload: Value = serde_json::from_slice(&request.body)
                    .map_err(|err| anyhow::anyhow!("invalid analytics payload: {err}"))?;
                let Some(events) = payload["events"].as_array() else {
                    continue;
                };
                if let Some(event) = events
                    .iter()
                    .find(|event| event["event_type"] == event_type)
                {
                    return Ok::<Value, anyhow::Error>(event.clone());
                }
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await?
}

pub(crate) fn thread_initialized_event(payload: &Value) -> Result<&Value> {
    let events = payload["events"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("analytics payload missing events array"))?;
    events
        .iter()
        .find(|event| event["event_type"] == "codex_thread_initialized")
        .ok_or_else(|| anyhow::anyhow!("codex_thread_initialized event should be present"))
}

pub(crate) fn assert_basic_thread_initialized_event(
    event: &Value,
    thread_id: &str,
    expected_model: &str,
    initialization_mode: &str,
) {
    assert_eq!(event["event_params"]["thread_id"], thread_id);
    assert_eq!(
        event["event_params"]["app_server_client"]["product_client_id"],
        DEFAULT_CLIENT_NAME
    );
    assert_eq!(
        event["event_params"]["app_server_client"]["client_name"],
        DEFAULT_CLIENT_NAME
    );
    assert_eq!(
        event["event_params"]["app_server_client"]["rpc_transport"],
        "stdio"
    );
    assert_eq!(event["event_params"]["model"], expected_model);
    assert_eq!(event["event_params"]["ephemeral"], false);
    assert_eq!(event["event_params"]["thread_source"], "user");
    assert_eq!(
        event["event_params"]["subagent_source"],
        serde_json::Value::Null
    );
    assert_eq!(
        event["event_params"]["parent_thread_id"],
        serde_json::Value::Null
    );
    assert_eq!(
        event["event_params"]["initialization_mode"],
        initialization_mode
    );
    assert!(event["event_params"]["created_at"].as_u64().is_some());
}
