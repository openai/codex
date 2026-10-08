//! Foreground updates honor explicit metrics exporters and never claim unconfirmed installation.

use anyhow::Context as _;
use pretty_assertions::assert_eq;
use serde_json::Value;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;

#[tokio::test]
async fn foreground_update_honors_metrics_exporter_and_reports_unconfirmed() -> anyhow::Result<()> {
    for (analytics_config, analytics_default_enabled, metrics_enabled) in [
        ("", false, true),
        ("", true, true),
        ("analytics.enabled = true\n", false, true),
        ("analytics.enabled = false\n", true, true),
        ("analytics.enabled = true\n", true, false),
    ] {
        let codex = codex_utils_cargo_bin::cargo_bin("codex")?;
        let server = MockServer::start().await;
        Mock::given(wiremock::matchers::path("/metrics"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let home = tempfile::tempdir()?;
        std::fs::write(
            home.path().join("config.toml"),
            format!(
                "cli_auth_credentials_store = \"file\"\n{analytics_config}[otel]\nmetrics_exporter = {{ otlp-http = {{ endpoint = \"{}/metrics\", protocol = \"json\" }} }}\n",
                server.uri(),
            ),
        )?;
        // No managed installation exists, so the command cannot confirm an applied update.
        let mut command = tokio::process::Command::new(&codex);
        command
            .current_dir(home.path())
            .env("CODEX_HOME", home.path())
            .env_remove(codex_app_server_daemon::telemetry::HANDOFF_ENV)
            .arg("app-server");
        if analytics_default_enabled {
            command.arg("--analytics-default-enabled");
        }
        if !metrics_enabled {
            command.args(["-c", "otel.metrics_exporter=\"none\""]);
        }
        let output = command
            .args(["daemon", "update", "--from-cli", "--yes"])
            .output()
            .await?;
        assert!(!output.status.success());
        let requests = server
            .received_requests()
            .await
            .context("metric requests")?;
        assert_eq!(requests.len(), usize::from(metrics_enabled));
        if !metrics_enabled {
            continue;
        }
        let body: Value = serde_json::from_slice(&requests[0].body)?;
        let metric = body["resourceMetrics"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|resource| resource["scopeMetrics"].as_array().into_iter().flatten())
            .flat_map(|scope| scope["metrics"].as_array().into_iter().flatten())
            .find(|metric| metric["name"] == "codex.daemon.update")
            .context("daemon update metric")?;
        assert!(
            metric["sum"]["dataPoints"][0]["attributes"]
                .as_array()
                .context("metric attributes")?
                .iter()
                .any(|tag| tag["key"] == "outcome" && tag["value"]["stringValue"] == "unconfirmed")
        );
    }
    Ok(())
}
