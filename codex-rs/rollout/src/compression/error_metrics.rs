//! Failure labels for rollout compression. Only static stages and bounded I/O kinds
//! are exported; error messages, paths, and rollout contents never become metric tags.

use std::io;

use super::RolloutCompressionTrigger;
use super::metrics;

pub(super) enum FailureMetric {
    File(RolloutCompressionTrigger),
    Materialize,
    Run(RolloutCompressionTrigger),
    Scan(RolloutCompressionTrigger),
    TempCleanup(RolloutCompressionTrigger),
}

impl FailureMetric {
    pub(super) fn record(self, stage: &'static str, error: &io::Error) {
        let (name, outcome_key, trigger) = match self {
            Self::File(trigger) => (metrics::FILE_COUNTER, "outcome", Some(trigger)),
            Self::Materialize => (metrics::MATERIALIZE_COUNTER, "outcome", None),
            Self::Run(trigger) => (metrics::RUN_COUNTER, "status", Some(trigger)),
            Self::Scan(trigger) => ("codex.rollout_compression.scan", "outcome", Some(trigger)),
            Self::TempCleanup(trigger) => (metrics::TEMP_CLEANUP_COUNTER, "outcome", Some(trigger)),
        };
        let Some(metrics) = codex_otel::global() else {
            return;
        };
        let tags = [
            (outcome_key, "failed"),
            ("stage", stage),
            ("error_kind", error_kind(error)),
            (
                "trigger",
                trigger.map_or("", RolloutCompressionTrigger::tag),
            ),
        ];
        let tags = if trigger.is_some() {
            &tags[..]
        } else {
            &tags[..3]
        };
        let _ = metrics.counter(name, /*inc*/ 1, tags);
    }
}

pub(super) fn error_kind(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::NotFound => "not_found",
        io::ErrorKind::PermissionDenied => "permission_denied",
        io::ErrorKind::AlreadyExists => "already_exists",
        io::ErrorKind::InvalidInput => "invalid_input",
        io::ErrorKind::InvalidData => "invalid_data",
        io::ErrorKind::TimedOut => "timed_out",
        io::ErrorKind::WriteZero => "write_zero",
        io::ErrorKind::Interrupted => "interrupted",
        io::ErrorKind::Unsupported => "unsupported",
        io::ErrorKind::UnexpectedEof => "unexpected_eof",
        io::ErrorKind::StorageFull => "storage_full",
        io::ErrorKind::ReadOnlyFilesystem => "read_only_filesystem",
        io::ErrorKind::NotADirectory => "not_a_directory",
        io::ErrorKind::IsADirectory => "is_a_directory",
        _ => "other",
    }
}
