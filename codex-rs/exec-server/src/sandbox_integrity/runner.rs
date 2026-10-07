//! Prepare the policy and dependency facts, then run checks per dependency.
//! Results never affect execution. See AGENTS.md for scope and accepted approximations.

use super::backend;
use super::dependencies::DependencyTarget;
use super::telemetry::IntegrityMetrics;
use codex_otel::MetricsClient;
use codex_protocol::sandbox::SandboxOverride;
use codex_sandboxing::FileContentsChecker;
use codex_sandboxing::SandboxExecRequest;
use std::io;
use std::time::Duration;
use std::time::Instant;

const CHECK_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 5);

/// Observe executor-local containment dependencies without affecting the launch.
/// Used by both exec-server and the legacy local execution path.
pub async fn run_integrity_checks(request: &SandboxExecRequest) {
    run_checks_with_metrics(request, codex_otel::global().as_ref()).await;
}

async fn run_checks_with_metrics(request: &SandboxExecRequest, metrics: Option<&MetricsClient>) {
    if request.sandbox != backend::SANDBOX_TYPE
        || request.sandbox_override == SandboxOverride::EscalatedSandboxWithRestrictions
    {
        return;
    }
    let policy = request.permission_profile.file_system_sandbox_policy();
    if policy.has_full_disk_write_access() {
        return;
    }
    let telemetry = IntegrityMetrics::new(metrics, backend::NAME, &policy);
    let _timer = telemetry.start_timer();
    let request = request.clone();
    let started_at = Instant::now();
    let work = tokio::task::spawn_blocking(move || {
        let mut dependencies = Vec::new();
        let prepared_at = Instant::now();
        let prepared = (|| {
            let cwd = request.sandbox_policy_cwd.to_abs_path()?;
            dependencies = backend::critical_dependencies(&request, &policy, &cwd)?;
            let policy = backend::prepare_policy(&request, policy, &cwd)?;
            // An empty glob expansion can remove the last write restriction.
            if policy.has_full_disk_write_access() {
                return Ok(None);
            }
            FileContentsChecker::new(&policy, &cwd).map(Some)
        })();
        let preparation_duration = prepared_at.elapsed();
        let mut checks = Vec::new();
        if matches!(&prepared, Ok(None)) {
            return (preparation_duration, true, checks);
        }
        for (dependency, path) in dependencies {
            let target = DependencyTarget::inspect(path);
            let result = match (&prepared, &target) {
                (Ok(Some(checker)), Ok(target)) => Some(checker.check(&target.path)),
                _ => None,
            };
            checks.push((dependency, target, result));
        }
        (preparation_duration, prepared.is_ok(), checks)
    });
    // Timing out stops waiting; running blocking work is intentionally not cancelled.
    let result = match tokio::time::timeout(CHECK_TIMEOUT, work).await {
        Ok(result) => result.map_err(io::Error::other),
        Err(error) => Err(io::Error::new(io::ErrorKind::TimedOut, error)),
    };
    match result {
        Ok((duration, succeeded, checks)) => {
            telemetry.record_preparation(duration, succeeded);
            for (dependency, target, result) in checks {
                telemetry.record_check("contents", dependency, &target, result);
            }
        }
        Err(error) => {
            telemetry.record_preparation(started_at.elapsed(), /*succeeded*/ false);
            telemetry.record_check("contents", "preparation", &Err(error), /*result*/ None);
        }
    }
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;
