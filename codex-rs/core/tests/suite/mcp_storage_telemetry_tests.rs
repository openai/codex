//! Exercises thread-owned storage attribution through turn-time MCP authentication recovery.

use anyhow::Result;
use codex_config::McpServerConfig;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_history::InitialHistory;
use codex_history::RolloutItem;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn storage_loads(metrics: &MetricsClient) -> Result<BTreeMap<String, u64>> {
    let mut counts = BTreeMap::new();
    for metric in metrics
        .snapshot()?
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
    {
        if metric.name() == "codex.auth_storage.operation"
            && let AggregatedMetrics::U64(MetricData::Sum(sum)) = metric.data()
        {
            for point in sum.data_points() {
                let tags: BTreeMap<_, _> = point
                    .attributes()
                    .map(|tag| (tag.key.to_string(), tag.value.to_string()))
                    .collect();
                if tags.get("credential_kind").map(String::as_str) == Some("mcp")
                    && tags.get("operation").map(String::as_str) == Some("load")
                {
                    *counts.entry(tags["originator"].clone()).or_default() += point.value();
                }
            }
        }
    }
    Ok(counts)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recovery_without_a_snapshot_keeps_each_threads_originator() -> Result<()> {
    skip_if_no_network!(Ok(()));
    // Isolate global metrics and CODEX_HOME from other integration tests.
    if std::env::var_os("CODEX_MCP_TELEMETRY_TEST_CHILD").is_none() {
        let home = tempfile::tempdir()?;
        let output = tokio::process::Command::new(std::env::current_exe()?)
            .args(["--exact", "suite::rmcp_client::storage_telemetry_tests::recovery_without_a_snapshot_keeps_each_threads_originator", "--nocapture"])
            .env("CODEX_MCP_TELEMETRY_TEST_CHILD", "1")
            .env("CODEX_HOME", home.path())
            .output().await?;
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(());
    }
    let server = responses::start_mock_server().await;
    let mcp = responses::start_mock_server().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(/*s*/ 401).append_header("www-authenticate", "Bearer"))
        .mount(&mcp)
        .await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "authorization_endpoint": format!("{}/authorize", mcp.uri()),
            "token_endpoint": format!("{}/token", mcp.uri()),
        })))
        .mount(&mcp)
        .await;
    let home = Arc::new(tempfile::tempdir()?);
    // The OAuth endpoint remains on the host when the test uses a remote executor.
    let fixture = test_codex()
        .with_home(home)
        .build_with_remote_and_local_env(&server)
        .await?;
    let server_config: McpServerConfig =
        serde_json::from_value(json!({"url": format!("{}/mcp", mcp.uri())}))?;
    let mut config = fixture.config.clone();
    config.mcp_oauth_credentials_store_mode = OAuthCredentialsStoreMode::File;
    config
        .mcp_servers
        .set([("reauth".into(), server_config)].into())?;
    let metrics = codex_otel::install_global_metrics(MetricsClient::new(
        MetricsConfig::in_memory("test", "test", "1", Default::default()).with_runtime_reader(),
    )?);
    let originators = ["codex_desktop", "codex_vscode"];
    let mut threads = Vec::new();
    for originator in originators {
        let thread = fixture
            .thread_manager
            .start_thread(StartThreadOptions {
                initial_history: InitialHistory::Forked(vec![RolloutItem::SessionMeta(
                    SessionMetaLine {
                        meta: SessionMeta {
                            originator: originator.into(),
                            ..Default::default()
                        },
                        git: None,
                    },
                )]),
                environments: Some(
                    fixture
                        .codex
                        .environment_selections()
                        .await
                        .into_iter()
                        .map(TurnEnvironmentSelection::into_request)
                        .collect(),
                ),
                ..StartThreadOptions::new(config.clone())
            })
            .await?
            .thread;
        let EventMsg::McpStartupComplete(startup) = wait_for_event(&thread, |event| {
            matches!(event, EventMsg::McpStartupComplete(_))
        })
        .await
        else {
            unreachable!();
        };
        assert_eq!(
            startup
                .failed
                .iter()
                .map(|failure| failure.server.as_str())
                .collect::<Vec<_>>(),
            vec!["reauth"]
        );
        threads.push(thread);
        responses::mount_sse_once(
            &server,
            responses::sse(vec![
                responses::ev_assistant_message("msg", "Sign in to continue."),
                responses::ev_completed("resp"),
            ]),
        )
        .await;
    }
    // The runtime reader exports deltas; drain startup observations before recovery.
    let _ = storage_loads(&metrics)?;
    futures::future::try_join_all(threads.iter().map(|thread| async {
        thread
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Continue".into(),
                text_elements: Vec::new(),
            }]))
            .await?;
        wait_for_event(thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
        Ok::<_, anyhow::Error>(())
    }))
    .await?;
    let mut after = storage_loads(&metrics)?;
    after.retain(|_, count| *count > 0);
    assert_eq!(
        after.keys().map(String::as_str).collect::<Vec<_>>(),
        originators
    );
    for thread in threads {
        thread.shutdown_and_wait().await?;
    }
    fixture.codex.shutdown_and_wait().await?;
    metrics.shutdown()?;
    Ok(())
}
