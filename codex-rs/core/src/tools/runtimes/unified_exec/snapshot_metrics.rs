//! Command-level v1 snapshot observations. Count replay selection, not successful
//! restoration or command completion; background captures are not commands.

use std::time::Duration;
use std::time::Instant;

use codex_features::Feature;
use codex_otel::SessionTelemetry;
use codex_tools::UnifiedExecShellMode;

use super::UnifiedExecRequest;
use crate::config::Config;

pub(super) struct SnapshotMetrics {
    pub(super) started_at: Instant,
    state: &'static str,
}

impl SnapshotMetrics {
    pub(super) fn start(
        req: &UnifiedExecRequest,
        config: &Config,
        shell_mode: &UnifiedExecShellMode,
    ) -> Option<Self> {
        let brokered = config
            .permissions
            .network
            .as_ref()
            .is_some_and(|network| network.enabled() && network.credential_broker_enabled());
        // Executor snapshots emit at their own selection boundary. Disabled,
        // remote legacy, and non-login invocations are outside this denominator.
        if !cfg!(unix)
            || req.shell_snapshot.is_some()
            || req.turn_environment.environment.is_remote()
            || !config.features.enabled(Feature::ShellSnapshot)
            || !matches!(shell_mode, UnifiedExecShellMode::Direct)
            || !req.shell.is_posix_login()
            || (req.cwd != req.turn_environment.selection.cwd && !brokered)
        {
            return None;
        }
        let state = if brokered {
            // Protected snapshots use a separate lazy cache, not the prewarm task.
            "protected"
        } else {
            match req.turn_environment.shell_snapshot.peek() {
                None => "prewarm_pending",
                Some(Some(snapshot))
                    if &snapshot.shell_environment_policy
                        == req.turn_environment.shell_environment_policy() =>
                {
                    "prewarm_ready"
                }
                Some(_) => "unavailable",
            }
        };
        Some(Self {
            started_at: Instant::now(),
            state,
        })
    }

    pub(super) fn record(
        self,
        telemetry: &SessionTelemetry,
        wait: Duration,
        outcome: &'static str,
    ) {
        let tags = [
            ("version", "v1"),
            ("state", self.state),
            ("outcome", outcome),
        ];
        telemetry.counter("codex.shell_snapshot.command", /*inc*/ 1, &tags);
        telemetry.record_duration("codex.shell_snapshot.wait_ms", wait, &tags);
    }
}
