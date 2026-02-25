use anyhow::Result;
use helios_core::config::ConfigBuilder;
use helios_core::config::types::OtelExporterKind;
use helios_core::config::types::OtelHttpProtocol;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use tempfile::TempDir;

const SERVICE_VERSION: &str = "0.0.0-test";

fn set_metrics_exporter(config: &mut helios_core::config::Config) {
    config.otel.metrics_exporter = OtelExporterKind::OtlpHttp {
        endpoint: "http://localhost:4318".to_string(),
        headers: HashMap::new(),
        protocol: OtelHttpProtocol::Json,
        tls: None,
    };
}

#[tokio::test]
async fn app_server_default_analytics_disabled_without_flag() -> Result<()> {
    let helios_home = TempDir::new()?;
    let mut config = ConfigBuilder::default()
        .helios_home(helios_home.path().to_path_buf())
        .build()
        .await?;
    set_metrics_exporter(&mut config);
    config.analytics_enabled = None;

    let provider = helios_core::otel_init::build_provider(
        &config,
        SERVICE_VERSION,
        Some("helios_app_server"),
        false,
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
    let helios_home = TempDir::new()?;
    let mut config = ConfigBuilder::default()
        .helios_home(helios_home.path().to_path_buf())
        .build()
        .await?;
    set_metrics_exporter(&mut config);
    config.analytics_enabled = None;

    let provider = helios_core::otel_init::build_provider(
        &config,
        SERVICE_VERSION,
        Some("helios_app_server"),
        true,
    )
    .map_err(|err| anyhow::anyhow!(err.to_string()))?;

    // With analytics unset in the config and the default flag is true, metrics are enabled.
    let has_metrics = provider.as_ref().and_then(|otel| otel.metrics()).is_some();
    assert_eq!(has_metrics, true);
    Ok(())
}
