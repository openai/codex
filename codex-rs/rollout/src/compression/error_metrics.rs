//! Failure labels for rollout compression. Only static stages and bounded I/O kinds
//! are exported; error messages, paths, and rollout contents never become metric tags.

use std::io;

use super::metrics;

pub(super) enum FailureMetric {
    File,
    Materialize,
    Run,
    Scan,
    TempCleanup,
}

impl FailureMetric {
    pub(super) fn record(self, stage: &'static str, error: &io::Error) {
        let (name, outcome_key) = match self {
            Self::File => (metrics::FILE_COUNTER, "outcome"),
            Self::Materialize => (metrics::MATERIALIZE_COUNTER, "outcome"),
            Self::Run => (metrics::RUN_COUNTER, "status"),
            Self::Scan => ("codex.rollout_compression.scan", "outcome"),
            Self::TempCleanup => (metrics::TEMP_CLEANUP_COUNTER, "outcome"),
        };
        let error_kind = match error.kind() {
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
        };
        let Some(metrics) = codex_otel::global() else {
            return;
        };
        let _ = metrics.counter(
            name,
            /*inc*/ 1,
            &[
                (outcome_key, "failed"),
                ("stage", stage),
                ("error_kind", error_kind),
            ],
        );
    }
}
