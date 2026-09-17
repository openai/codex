//! Records automatic daemon startup and ensures launch observations survive setup failures.

use super::*;

/// Emit once after connection, or as unconfirmed if startup returns early.
pub(super) struct Launch<F: FnOnce(&AppServerTarget, bool)>(pub(super) Option<F>);

impl<F: FnOnce(&AppServerTarget, bool)> Launch<F> {
    pub(super) fn record(mut self, target: &AppServerTarget, connected: bool) {
        if let Some(record) = self.0.take() {
            record(target, connected);
        }
    }
}

impl<F: FnOnce(&AppServerTarget, bool)> Drop for Launch<F> {
    fn drop(&mut self) {
        if let Some(record) = self.0.take() {
            record(&AppServerTarget::Embedded, /*connected*/ false);
        }
    }
}

pub(super) async fn record_start(
    config: &Config,
    result: &anyhow::Result<codex_app_server_daemon::LifecycleOutput>,
) {
    use codex_app_server_daemon::LifecycleStatus;
    let outcome = match result {
        Ok(output) => match output.status {
            LifecycleStatus::Started => "started",
            LifecycleStatus::AlreadyRunning => "already_running",
            LifecycleStatus::Restarted => "restarted",
            LifecycleStatus::Stopped | LifecycleStatus::NotRunning | LifecycleStatus::Running => {
                "unconfirmed"
            }
        },
        Err(_) => "failed",
    };
    // Failure can happen before normal TUI telemetry is initialized. This short-lived
    // provider uses exactly the same consent and identity construction as TUI startup.
    let Ok(Ok(Some(otel))) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        codex_app_server_client::build_otel_provider(
            config,
            env!("CARGO_PKG_VERSION"),
            /*service_name_override*/ None,
            /*default_analytics_enabled*/ true,
        )
    })) else {
        return;
    };
    if let Some(metrics) = otel.metrics() {
        let mut tags = codex_app_server_daemon::telemetry::settings_tags(&config.codex_home)
            .await
            .to_vec();
        tags.extend([
            ("initiation_source", "tui_auto_start"),
            ("outcome", outcome),
        ]);
        let _ = metrics.counter("codex.daemon.start", /*inc*/ 1, &tags);
    }
    let _ = otel
        .shutdown_with_timeout(INTERACTIVE_OTEL_SHUTDOWN_TIMEOUT)
        .await;
}
