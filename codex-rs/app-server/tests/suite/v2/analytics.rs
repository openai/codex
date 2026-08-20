use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::DEFAULT_CLIENT_NAME;
use app_test_support::TestAppServer;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use app_test_support::write_mock_responses_config_toml_with_chatgpt_base_url;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxPolicy;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use codex_config::types::AuthCredentialsStoreMode;
use codex_config::types::OtelExporterKind;
use codex_config::types::OtelHttpProtocol;
use codex_core::config::ConfigBuilder;
use codex_core_plugins::loader::curated_plugin_cache_version;
use codex_core_plugins::store::PluginStore;
use codex_plugin::PluginId;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_remote;
use core_test_support::skip_if_wine_exec;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
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

pub(crate) async fn wait_for_analytics_payload(
    server: &MockServer,
    read_timeout: Duration,
) -> Result<Value> {
    let body = timeout(read_timeout, async {
        loop {
            let Some(requests) = server.received_requests().await else {
                tokio::time::sleep(Duration::from_millis(25)).await;
                continue;
            };
            if let Some(request) = requests.iter().find(|request| {
                request.method == "POST" && request.url.path() == "/codex/analytics-events/events"
            }) {
                break request.body.clone();
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await?;
    serde_json::from_slice(&body).map_err(|err| anyhow::anyhow!("invalid analytics payload: {err}"))
}

pub(crate) async fn wait_for_analytics_event(
    server: &MockServer,
    read_timeout: Duration,
    event_type: &str,
) -> Result<Value> {
    wait_for_matching_analytics_event(server, read_timeout, |event| {
        event["event_type"] == event_type
    })
    .await
}

pub(crate) async fn wait_for_goal_event(
    server: &MockServer,
    read_timeout: Duration,
    event_kind: &str,
    goal_status: &str,
) -> Result<Value> {
    wait_for_matching_analytics_event(server, read_timeout, |event| {
        event["event_type"] == "codex_goal_event"
            && event["event_params"]["event_kind"] == event_kind
            && event["event_params"]["goal_status"] == goal_status
    })
    .await
}

pub(crate) async fn wait_for_matching_analytics_event(
    server: &MockServer,
    read_timeout: Duration,
    matches: impl Fn(&Value) -> bool,
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
                if let Some(event) = events.iter().find(|event| matches(event)) {
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
    session_id: &str,
    expected_product_client_id: &str,
    expected_model: &str,
    initialization_mode: &str,
    expected_thread_source: &str,
) {
    assert_eq!(event["event_params"]["thread_id"], thread_id);
    assert_eq!(event["event_params"]["session_id"], session_id);
    assert_eq!(
        event["event_params"]["app_server_client"]["product_client_id"],
        expected_product_client_id
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
    assert_eq!(
        event["event_params"]["thread_source"],
        expected_thread_source
    );
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

const METRICS_PLUGIN_ID: &str = "sample@openai-curated";
const TEST_CURATED_PLUGIN_SHA: &str = "0123456789abcdef0123456789abcdef01234567";

fn write_curated_metrics_plugin(codex_home: &Path) -> Result<PathBuf> {
    let plugin_id = PluginId::parse(METRICS_PLUGIN_ID)?;
    let plugin_root = PluginStore::new(codex_home.to_path_buf()).plugin_root(
        &plugin_id,
        &curated_plugin_cache_version(TEST_CURATED_PLUGIN_SHA),
    );
    let script_path = plugin_root.join("scripts/run.sh");
    std::fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
    std::fs::create_dir_all(script_path.parent().expect("script path has parent"))?;
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample","version":"0.1.0"}"#,
    )?;
    std::fs::write(
        plugin_root.join("analytics.yaml"),
        r#"version: 1
operations:
  scan:
    path: ./scripts/run.sh
    measurements:
      findings:
        dimensions:
          severity: [high, low]
      files_scanned: {}
"#,
    )?;
    std::fs::write(
        &script_path,
        r#"test -n "$CODEX_PLUGIN_METRICS_OUTPUT"
sleep "${1:-0.3}"
printf '%s' '{"version":1,"measurements":[{"name":"findings","value":3,"dimensions":{"severity":"high"}},{"name":"files_scanned","value":17}]}' > "$CODEX_PLUGIN_METRICS_OUTPUT"
"#,
    )?;

    let curated_repo = codex_home.join(".tmp/plugins");
    std::fs::create_dir_all(curated_repo.join(".agents/plugins"))?;
    std::fs::write(
        curated_repo.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "openai-curated",
  "plugins": [
    {"name": "sample", "source": {"source": "local", "path": "./plugins/sample"}}
  ]
}"#,
    )?;
    std::fs::write(
        codex_home.join(".tmp/plugins.sha"),
        format!("{TEST_CURATED_PLUGIN_SHA}\n"),
    )?;
    Ok(script_path.into_path_buf())
}

async fn assert_plugin_measurement_analytics(remote: bool, background: bool) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_remote!(
        Ok(()),
        "trusted plugin metrics fixture uses a local Codex home cache"
    );
    skip_if_wine_exec!(Ok(()), "plugin metrics fixture is Unix-only");

    let codex_home = TempDir::new()?;
    let script_path = write_curated_metrics_plugin(codex_home.path())?.canonicalize()?;
    let mut command = vec![
        "/bin/sh".to_string(),
        script_path.to_string_lossy().into_owned(),
    ];
    if background {
        command.push("1.0".to_string());
    }
    let call_id = "curated-plugin-metrics";
    let arguments = serde_json::to_string(&json!({
        "cmd": shlex::try_join(command.iter().map(String::as_str))?,
        "yield_time_ms": if background { 10 } else { 1_000 },
    }))?;
    let command_response = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_function_call(call_id, "exec_command", &arguments),
        responses::ev_completed("resp-1"),
    ]);
    let final_response = create_final_assistant_message_sse_response("done")?;
    let server =
        create_mock_responses_server_sequence(vec![command_response, final_response]).await;

    let analytics_server = responses::start_mock_server().await;
    write_mock_responses_config_toml_with_chatgpt_base_url(
        codex_home.path(),
        &server.uri(),
        &analytics_server.uri(),
    )?;
    let config_path = codex_home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        config_path,
        format!(
            r#"{config}
[features]
plugins = true
remote_plugin = false
unified_exec = true
shell_zsh_fork = false
unified_exec_zsh_fork = false

[plugins."{METRICS_PLUGIN_ID}"]
enabled = true
"#,
        ),
    )?;
    mount_analytics_capture(&analytics_server, codex_home.path()).await?;

    let mut builder = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config();
    if remote {
        builder = builder.with_exec_server_delay(Duration::ZERO);
    }
    let mut mcp = builder.build().await?;
    if remote {
        assert_eq!(
            mcp.auto_env_params()?.environment_id,
            codex_exec_server::REMOTE_ENVIRONMENT_ID
        );
    }
    timeout(Duration::from_secs(10), mcp.initialize()).await??;
    let thread_request = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_response: JSONRPCResponse = timeout(
        Duration::from_secs(10),
        mcp.read_stream_until_response_message(RequestId::Integer(thread_request)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(thread_response)?;
    let thread_id = thread.id.clone();

    let turn_request = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![UserInput::Text {
                text: "run the curated plugin metrics script".to_string(),
                text_elements: Vec::new(),
            }],
            sandbox_policy: Some(SandboxPolicy::ReadOnly {
                network_access: false,
            }),
            ..Default::default()
        })
        .await?;
    timeout(
        Duration::from_secs(10),
        mcp.read_stream_until_response_message(RequestId::Integer(turn_request)),
    )
    .await??;
    let completed_turn = timeout(
        Duration::from_secs(10),
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    let turn_id = completed_turn
        .params
        .as_ref()
        .and_then(|params| params["turn"]["id"].as_str())
        .expect("completed turn id");

    if background {
        let model_request_bodies = server
            .received_requests()
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|request| request.url.path().ends_with("/responses"))
            .map(|request| serde_json::from_slice::<Value>(&request.body))
            .collect::<Result<Vec<_>, _>>()?;
        let request_text = serde_json::to_string(&model_request_bodies)?;
        assert!(request_text.contains("Process running with session ID "));
        assert!(!request_text.contains("Process exited with code 0"));
    }

    for measurement_name in ["findings", "files_scanned"] {
        wait_for_matching_analytics_event(&analytics_server, Duration::from_secs(10), |event| {
            event["event_type"] == "codex_plugin_measurement_event"
                && event["event_params"]["item_id"] == call_id
                && event["event_params"]["measurement_name"] == measurement_name
        })
        .await?;
    }
    let command_event =
        wait_for_matching_analytics_event(&analytics_server, Duration::from_secs(10), |event| {
            event["event_type"] == "codex_command_execution_event"
                && event["event_params"]["item_id"] == call_id
        })
        .await?;
    assert_eq!(
        json!({
            "plugin_id": command_event["event_params"]["plugin_id"],
            "script_path": command_event["event_params"]["script_path"],
            "item_id": command_event["event_params"]["item_id"],
            "exit_code": command_event["event_params"]["exit_code"],
        }),
        json!({
            "plugin_id": METRICS_PLUGIN_ID,
            "script_path": "scripts/run.sh",
            "item_id": call_id,
            "exit_code": 0,
        })
    );
    let mut measurement_events = Vec::new();
    for request in analytics_server
        .received_requests()
        .await
        .unwrap_or_default()
    {
        if request.method != "POST" || request.url.path() != "/codex/analytics-events/events" {
            continue;
        }
        let payload: Value = serde_json::from_slice(&request.body)?;
        let Some(events) = payload["events"].as_array() else {
            continue;
        };
        measurement_events.extend(
            events
                .iter()
                .filter(|event| {
                    event["event_type"] == "codex_plugin_measurement_event"
                        && event["event_params"]["item_id"] == call_id
                })
                .cloned(),
        );
    }
    measurement_events.sort_by(|left, right| {
        left["event_params"]["measurement_name"]
            .as_str()
            .cmp(&right["event_params"]["measurement_name"].as_str())
    });
    assert_eq!(measurement_events.len(), 2);
    let execution_id = measurement_events[0]["event_params"]["execution_id"]
        .as_str()
        .expect("measurement execution id");
    assert!(!execution_id.is_empty());
    assert_eq!(
        measurement_events[1]["event_params"]["execution_id"].as_str(),
        Some(execution_id)
    );
    assert_eq!(
        measurement_events
            .iter()
            .map(|event| json!({
                "plugin_id": event["event_params"]["plugin_id"],
                "operation": event["event_params"]["operation"],
                "measurement_name": event["event_params"]["measurement_name"],
                "number_value": event["event_params"]["number_value"],
                "dimensions": event["event_params"]["dimensions"],
                "item_id": event["event_params"]["item_id"],
            }))
            .collect::<Vec<_>>(),
        vec![
            json!({
                "plugin_id": METRICS_PLUGIN_ID,
                "operation": "scan",
                "measurement_name": "files_scanned",
                "number_value": 17.0,
                "dimensions": null,
                "item_id": call_id,
            }),
            json!({
                "plugin_id": METRICS_PLUGIN_ID,
                "operation": "scan",
                "measurement_name": "findings",
                "number_value": 3.0,
                "dimensions": {"severity": "high"},
                "item_id": call_id,
            }),
        ]
    );
    for event in &measurement_events {
        let event_params = event["event_params"]
            .as_object()
            .expect("measurement event params");
        let mut field_names = event_params.keys().map(String::as_str).collect::<Vec<_>>();
        field_names.sort_unstable();
        assert_eq!(
            field_names,
            vec![
                "dimensions",
                "execution_id",
                "item_id",
                "measurement_name",
                "number_value",
                "operation",
                "plugin_id",
                "thread_id",
                "turn_id",
            ]
        );
        assert_eq!(event_params["thread_id"], thread_id);
        assert_eq!(event_params["turn_id"], turn_id);
    }

    Ok(())
}

#[cfg_attr(windows, ignore = "plugin metrics fixture is Unix-only")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_plugin_script_emits_measurement_analytics() -> Result<()> {
    assert_plugin_measurement_analytics(/*remote*/ false, /*background*/ false).await
}

#[cfg_attr(windows, ignore = "plugin metrics fixture is Unix-only")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_unified_plugin_script_emits_measurement_analytics() -> Result<()> {
    assert_plugin_measurement_analytics(/*remote*/ true, /*background*/ false).await
}

#[cfg_attr(windows, ignore = "plugin metrics fixture is Unix-only")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_background_plugin_script_emits_measurements_after_turn_completion() -> Result<()> {
    assert_plugin_measurement_analytics(/*remote*/ false, /*background*/ true).await
}

#[cfg_attr(windows, ignore = "plugin metrics fixture is Unix-only")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_unified_background_plugin_script_emits_measurements_after_turn_completion()
-> Result<()> {
    assert_plugin_measurement_analytics(/*remote*/ true, /*background*/ true).await
}
