use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use uuid::Uuid;

use super::*;

#[tokio::test]
async fn sqlite_sink_filters_noisy_targets_without_dropping_useful_diagnostics() {
    let codex_home =
        std::env::temp_dir().join(format!("codex-state-log-db-filter-{}", Uuid::new_v4()));
    let _cleanup = scopeguard::guard(codex_home.clone(), |codex_home| {
        let _ = std::fs::remove_dir_all(codex_home);
    });
    let runtime = StateRuntime::init(
        crate::SqliteConfig::new_for_testing(codex_home.as_path().abs()),
        "test-provider".to_string(),
    )
    .await
    .expect("initialize runtime");
    let layer = start(runtime.clone());

    let guard = tracing_subscriber::registry()
        .with(layer.clone().with_filter(default_filter()))
        .set_default();

    tracing::trace!(target: "opentelemetry_sdk", "dropped-trace");
    tracing::debug!(target: "opentelemetry_sdk", "dropped-debug");
    tracing::info!(target: "opentelemetry_sdk", "retained-info");
    tracing::debug!(target: "rmcp::transport", "dropped-rmcp-debug");
    tracing::info!(target: "rmcp::transport", "retained-rmcp-info");
    tracing::debug!(
        target: "codex_rmcp_client::oauth",
        "dropped-codex-rmcp-client-debug"
    );
    tracing::info!(
        target: "codex_rmcp_client::oauth",
        "retained-codex-rmcp-client-info"
    );
    tracing::trace!(target: "codex_http_client::transport", "dropped-request-body");
    tracing::debug!(target: "codex_http_client::transport", "retained-request-diagnostic");
    tracing::trace!(target: "codex_api::sse", "dropped-sse-parent");
    tracing::trace!(target: "codex_api::sse::responses", "dropped-sse-payload");
    tracing::debug!(target: "codex_api::sse::responses", "retained-sse-diagnostic");
    tracing::trace!(target: "codex_state", "retained-trace");
    tracing::trace!(
        target: "codex_tui::streaming::controller",
        "dropped-controller-trace"
    );
    tracing::debug!(
        target: "codex_tui::streaming::controller",
        "retained-controller-debug"
    );
    tracing::trace!(
        target: "codex_tui::streaming::table_holdback",
        "dropped-table-holdback-trace"
    );
    tracing::debug!(
        target: "codex_tui::streaming::table_holdback",
        "retained-table-holdback-debug"
    );
    tracing::trace!(
        target: "codex_tui::streaming::commit_tick",
        "retained-commit-tick-trace"
    );
    tracing::trace!(
        target: "codex_api::responses_websocket_timing",
        payload = "complete timing payload",
        "dropped-websocket-timing"
    );

    layer.flush().await;
    drop(guard);

    let logs = runtime
        .query_logs(&crate::LogQuery::default())
        .await
        .expect("query logs after flush");
    assert_eq!(
        logs.iter()
            .map(|row| (
                row.level.as_str(),
                row.target.as_str(),
                row.message.as_deref()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("INFO", "opentelemetry_sdk", Some("retained-info")),
            ("INFO", "rmcp::transport", Some("retained-rmcp-info")),
            (
                "INFO",
                "codex_rmcp_client::oauth",
                Some("retained-codex-rmcp-client-info")
            ),
            (
                "DEBUG",
                "codex_http_client::transport",
                Some("retained-request-diagnostic")
            ),
            (
                "DEBUG",
                "codex_api::sse::responses",
                Some("retained-sse-diagnostic")
            ),
            ("TRACE", "codex_state", Some("retained-trace")),
            (
                "DEBUG",
                "codex_tui::streaming::controller",
                Some("retained-controller-debug"),
            ),
            (
                "DEBUG",
                "codex_tui::streaming::table_holdback",
                Some("retained-table-holdback-debug"),
            ),
            (
                "TRACE",
                "codex_tui::streaming::commit_tick",
                Some("retained-commit-tick-trace"),
            ),
        ]
    );
}
