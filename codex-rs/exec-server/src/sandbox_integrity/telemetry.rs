//! Construct bounded tags from policies, dependency facts, and checker results, and emit metrics.
//! This module does not discover dependencies, inspect files, prepare policies, or run checks.
//! Without a metrics client, emission is a no-op; callers still run the checks.

use super::dependencies::DependencyTarget;
use codex_otel::MetricsClient;
use codex_otel::Timer;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_sandboxing::IntegrityFinding;
use std::fs;
use std::io;
use std::time::Duration;

pub(super) struct IntegrityMetrics<'a> {
    client: Option<&'a MetricsClient>,
    tags: [(&'static str, &'static str); 2],
}

impl<'a> IntegrityMetrics<'a> {
    pub(super) fn new(
        client: Option<&'a MetricsClient>,
        backend: &'static str,
        policy: &FileSystemSandboxPolicy,
    ) -> Self {
        let has_deny_globs = policy
            .entries
            .iter()
            .any(|entry| matches!(entry.path, FileSystemPath::GlobPattern { .. }));
        Self {
            client,
            tags: [
                ("backend", backend),
                ("has_deny_globs", bool_tag(Some(has_deny_globs))),
            ],
        }
    }

    pub(super) fn start_timer(&self) -> Option<Timer> {
        self.client?
            .start_timer("codex.sandbox_integrity.duration_ms", &self.tags)
            .ok()
    }

    pub(super) fn record_preparation(&self, duration: Duration, succeeded: bool) {
        let Some(client) = self.client else {
            return;
        };
        let _ = client.record_duration(
            "codex.sandbox_integrity.preparation_ms",
            duration,
            &[
                self.tags[0],
                self.tags[1],
                ("outcome", if succeeded { "ok" } else { "error" }),
            ],
        );
    }

    /// A missing result means dependency or checker preparation failed.
    pub(super) fn record_check(
        &self,
        checker: &'static str,
        dependency: &'static str,
        target: &io::Result<DependencyTarget>,
        result: Option<io::Result<Option<IntegrityFinding>>>,
    ) {
        let Some(client) = self.client else {
            return;
        };
        let error_kind = match (target, &result) {
            (Err(error), _) | (_, Some(Err(error))) => {
                if error.kind() == io::ErrorKind::TimedOut {
                    "timeout"
                } else {
                    "other"
                }
            }
            (_, None) => "preparation",
            (_, Some(Ok(_))) => "none",
        };
        let (outcome, multiple_roots) = match (target, result) {
            (Err(error), _) if error.kind() == io::ErrorKind::NotFound => ("missing", None),
            (_, Some(Ok(Some(finding)))) => ("finding", Some(finding.write_roots.len() > 1)),
            (_, Some(Ok(None))) => ("clean", Some(false)),
            _ => ("error", None),
        };
        let target = target.as_ref().ok();
        let resolution_changed = target.and_then(|target| {
            target
                .resolved_path
                .as_ref()
                .map(|resolved| resolved != &target.path)
        });
        let hard_links = target.and_then(|target| multiple_hard_links(&target.metadata));
        let _ = client.counter(
            "codex.sandbox_integrity.check",
            /*inc*/ 1,
            &[
                self.tags[0],
                ("checker", checker),
                ("dependency", dependency),
                ("outcome", outcome),
                ("error_kind", error_kind),
                self.tags[1],
                ("target_resolution_changed", bool_tag(resolution_changed)),
                ("multiple_hard_links", bool_tag(hard_links)),
                ("multiple_write_roots", bool_tag(multiple_roots)),
            ],
        );
    }
}

#[cfg(unix)]
fn multiple_hard_links(metadata: &fs::Metadata) -> Option<bool> {
    use std::os::unix::fs::MetadataExt;
    Some(metadata.nlink() > 1)
}

#[cfg(not(unix))]
fn multiple_hard_links(_: &fs::Metadata) -> Option<bool> {
    None
}

fn bool_tag(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "true",
        Some(false) => "false",
        None => "unknown",
    }
}
