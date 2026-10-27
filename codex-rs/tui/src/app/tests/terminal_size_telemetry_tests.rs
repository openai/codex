//! Terminal-size observation eligibility follows the config reloaded for resume.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn terminal_size_telemetry_respects_reloaded_exporter_and_consent() -> Result<()> {
    let (mut app, _app_event_rx, _op_rx) = Box::pin(make_test_app_with_channels()).await;
    let cwd = app.config.cwd.to_path_buf();
    let mut enabled = Vec::new();
    for config in [
        "[otel]\nmetrics_exporter = \"statsig\"\n",
        "[otel]\nmetrics_exporter = \"none\"\n",
        "[otel]\nmetrics_exporter = \"statsig\"\n[analytics]\nenabled = false\n",
        "analytics.enabled = false\n[otel.metrics_exporter.otlp-http]\nendpoint = \"http://127.0.0.1:4318/v1/metrics\"\nprotocol = \"json\"\n",
    ] {
        std::fs::write(app.config.codex_home.join("config.toml"), config)?;
        let (config, local_settings) = Box::pin(app.rebuild_config_for_resume_or_fallback(
            &cwd,
            cwd.clone(),
            (&app.config, &app.local_settings),
        ))
        .await?;
        app.config = config;
        app.local_settings = local_settings;
        enabled.push(crate::terminal_size_telemetry::enabled(&app.config));
    }
    assert_eq!(enabled, vec![true, false, false, true]);
    Ok(())
}
