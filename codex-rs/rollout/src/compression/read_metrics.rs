//! One observation per rollout reader, including failed opens and partial reads.
//! Duration sums completed open/retry and read calls, excluding caller processing
//! and any canceled call. Partial readers must not be treated as successful EOFs.
//! Aggregate locally rather than exporting a metric for every JSONL record.

use std::io;
use std::time::Duration;

use super::error_metrics::error_kind;

pub(super) struct ReadMetrics {
    pub(super) format: &'static str,
    pub(super) reached_eof: bool,
    pub(super) duration: Duration,
    failure: Option<(&'static str, &'static str)>,
}

impl Default for ReadMetrics {
    fn default() -> Self {
        Self {
            format: "unknown",
            reached_eof: false,
            duration: Duration::ZERO,
            failure: None,
        }
    }
}

impl ReadMetrics {
    pub(super) fn failed(&mut self, stage: &'static str, error: &io::Error) {
        self.failure.get_or_insert((stage, error_kind(error)));
    }
}

impl Drop for ReadMetrics {
    fn drop(&mut self) {
        let Some(metrics) = codex_otel::global() else {
            return;
        };
        let (outcome, stage, error_kind) = match self.failure {
            Some((stage, error_kind)) => ("failed", stage, error_kind),
            None if self.reached_eof => ("eof", "none", "none"),
            None => ("partial", "none", "none"),
        };
        let tags = [
            ("format", self.format),
            ("outcome", outcome),
            ("stage", stage),
            ("error_kind", error_kind),
        ];
        let _ = metrics.counter("codex.rollout_compression.read", /*inc*/ 1, &tags);
        let _ = metrics.record_duration(
            "codex.rollout_compression.read.io_duration_ms",
            self.duration,
            &tags,
        );
    }
}
