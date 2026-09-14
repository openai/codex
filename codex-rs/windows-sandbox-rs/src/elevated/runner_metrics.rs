//! Compare registered and legacy runner outcomes at the actual local execution boundary.
//! Startup is counted once after credential retry; commands are counted on exit or pipe failure.

pub(crate) fn record(phase: &'static str, outcome: &'static str) {
    let runtime = if crate::app_package::registered_core_requested() {
        "registered"
    } else {
        "legacy"
    };
    if let Some(metrics) = codex_otel::global() {
        // Fixed labels only: never send command text, paths, or arbitrary error messages.
        let _ = metrics.counter(
            "codex.windows_sandbox.runner_result",
            /*inc*/ 1,
            &[("runtime", runtime), ("phase", phase), ("outcome", outcome)],
        );
    }
}

pub(crate) fn record_command(exit: Option<(i32, bool)>) {
    record("command", command_outcome(exit));
}

fn command_outcome(exit: Option<(i32, bool)>) -> &'static str {
    match exit {
        Some((_, true)) => "timeout",
        Some((0, false)) => "success",
        Some((_, false)) => "nonzero_exit",
        None => "transport_error",
    }
}

#[cfg(test)]
#[path = "runner_metrics_tests.rs"]
mod tests;
