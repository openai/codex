//! Foreground daemon observations use the CLI's existing consent and identity handling.
//! Detached updater entry points never call this module.

use codex_utils_cli::CliConfigOverrides;
use std::time::Duration;
use tokio::time::timeout;

pub(crate) async fn record_command(
    overrides: &CliConfigOverrides,
    analytics_default_enabled: bool,
    target: &'static str,
    result: &anyhow::Result<Option<codex_app_server_daemon::UpdateOutput>>,
) {
    if std::env::var_os(codex_app_server_daemon::telemetry::HANDOFF_ENV).is_some() {
        return;
    }
    // Skip telemetry if consent-aware configuration cannot be loaded promptly.
    let Ok(Ok(config)) = timeout(
        Duration::from_secs(/*secs*/ 2),
        crate::cloud_config::load_config(overrides, codex_config::LoaderOverrides::default()),
    )
    .await
    else {
        return;
    };
    use codex_app_server_daemon::UpdateStatus;
    let outcome = match result {
        Ok(Some(output)) => match output.status {
            UpdateStatus::Updated => "updated",
            UpdateStatus::NoUpdate => "no_update",
            UpdateStatus::Unsupported => "unsupported",
        },
        Ok(None) => "cancelled",
        Err(_) => "unconfirmed",
    };
    let Ok(Ok(Some(otel))) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        codex_core::otel_init::build_provider(
            &config,
            env!("CARGO_PKG_VERSION"),
            /*service_name_override*/ None,
            analytics_default_enabled,
        )
    })) else {
        return;
    };
    if let Some(metrics) = otel.metrics() {
        let mut tags = codex_app_server_daemon::telemetry::settings_tags(&config.codex_home)
            .await
            .to_vec();
        tags.extend([
            ("initiation_source", "cli"),
            ("update_target", target),
            ("outcome", outcome),
        ]);
        let _ = metrics.counter("codex.daemon.update", /*inc*/ 1, &tags);
    }
    let _ = otel
        .shutdown_with_timeout(std::time::Duration::from_secs(/*secs*/ 2))
        .await;
}
