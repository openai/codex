//! Records low-cardinality migration metrics shared by manual and startup runs.
//!
//! Migration owns the outcome stream, so it is the one place that can count successful,
//! skipped, and failed rollouts consistently. Callers only tell it why the run started.

use std::time::Instant;

use super::RolloutMigrationMode;
use super::RolloutMigrationOptions;
use super::RolloutMigrationReport;
use super::RolloutMigrationStatus;
use crate::ThreadStoreResult;

const RUN_METRIC: &str = "codex.rollout_migration.run";
const RUN_DURATION_METRIC: &str = "codex.rollout_migration.run.duration_ms";
const RUN_IO_BYTES_METRIC: &str = "codex.rollout_migration.run.io_bytes";
const THREAD_METRIC: &str = "codex.rollout_migration.thread";

#[derive(Clone, Copy)]
pub(super) enum RolloutMigrationTrigger {
    Manual,
    Startup,
}

#[derive(Clone, Copy)]
enum RolloutMigrationScope {
    All,
    Selected,
}

pub(super) struct RolloutMigrationTelemetry {
    trigger: RolloutMigrationTrigger,
    mode: RolloutMigrationMode,
    scope: RolloutMigrationScope,
    started_at: Instant,
}

impl RolloutMigrationTelemetry {
    pub(super) fn new(trigger: RolloutMigrationTrigger, options: &RolloutMigrationOptions) -> Self {
        Self {
            trigger,
            mode: options.mode,
            scope: if options.thread_ids.is_empty() {
                RolloutMigrationScope::All
            } else {
                RolloutMigrationScope::Selected
            },
            started_at: Instant::now(),
        }
    }

    pub(super) fn finish(&self, result: &ThreadStoreResult<RolloutMigrationReport>) {
        let Some(metrics) = codex_otel::global() else {
            return;
        };
        let mut io_bytes = 0_u64;
        if let Ok(report) = result {
            for outcome in &report.outcomes {
                let tags = [
                    ("trigger", self.trigger.tag()),
                    ("mode", self.mode.tag()),
                    ("scope", self.scope.tag()),
                    ("status", outcome.status.tag()),
                ];
                let _ = metrics.counter(THREAD_METRIC, /*inc*/ 1, &tags);
                io_bytes = io_bytes.saturating_add(outcome.bytes_processed);
            }
        }
        let result = match result {
            Err(_) => "error",
            Ok(report)
                if report
                    .outcomes
                    .iter()
                    .any(|outcome| outcome.status == RolloutMigrationStatus::Failed) =>
            {
                "partial_failure"
            }
            Ok(_) => "success",
        };
        let tags = [
            ("trigger", self.trigger.tag()),
            ("mode", self.mode.tag()),
            ("scope", self.scope.tag()),
            ("result", result),
        ];
        let duration_ms = i64::try_from(self.started_at.elapsed().as_millis()).unwrap_or(i64::MAX);
        let io_bytes = i64::try_from(io_bytes).unwrap_or(i64::MAX);
        let _ = metrics.counter(RUN_METRIC, /*inc*/ 1, &tags);
        let _ = metrics.histogram(RUN_DURATION_METRIC, duration_ms, &tags);
        let _ = metrics.histogram(RUN_IO_BYTES_METRIC, io_bytes, &tags);
    }
}

impl RolloutMigrationTrigger {
    fn tag(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Startup => "startup",
        }
    }
}

impl RolloutMigrationMode {
    fn tag(self) -> &'static str {
        match self {
            Self::DryRun => "dry_run",
            Self::Apply => "apply",
        }
    }
}

impl RolloutMigrationScope {
    fn tag(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Selected => "selected",
        }
    }
}

impl RolloutMigrationStatus {
    fn tag(self) -> &'static str {
        match self {
            Self::Eligible => "eligible",
            Self::Migrated => "migrated",
            Self::AlreadyPaginated => "already_paginated",
            Self::SkippedEmpty => "skipped_empty",
            Self::SkippedBusy => "skipped_busy",
            Self::Failed => "failed",
        }
    }
}
