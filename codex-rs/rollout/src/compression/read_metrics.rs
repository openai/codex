//! One observation per rollout reader, including failed opens and partial reads.
//! Duration sums completed open/retry and read calls, excluding caller processing
//! and any canceled call. Partial readers must not be treated as successful EOFs.
//! Aggregate locally rather than exporting a metric for every JSONL record.
//! Failure reasons and progress are bounded labels captured at the first failure;
//! error messages, file paths, and rollout contents are never exported.

use std::io;
use std::time::Duration;

use super::error_metrics::error_kind;

pub(super) struct ReadMetrics {
    pub(super) format: &'static str,
    pub(super) reached_eof: bool,
    pub(super) duration: Duration,
    pub(super) read_any_line: bool,
    failure: Option<ReadFailure>,
}

pub(super) enum ReadFailureSource {
    Stream,
    ReaderBusy,
    TaskJoin,
}

struct ReadFailure {
    stage: &'static str,
    error_kind: &'static str,
    reason: &'static str,
    read_progress: &'static str,
}

impl Default for ReadMetrics {
    fn default() -> Self {
        Self {
            format: "unknown",
            reached_eof: false,
            duration: Duration::ZERO,
            read_any_line: false,
            failure: None,
        }
    }
}

impl ReadMetrics {
    pub(super) fn failed(
        &mut self,
        stage: &'static str,
        source: ReadFailureSource,
        error: &io::Error,
    ) {
        self.failure.get_or_insert_with(|| ReadFailure {
            stage,
            error_kind: error_kind(error),
            reason: match source {
                ReadFailureSource::ReaderBusy => "reader_busy",
                ReadFailureSource::TaskJoin => "task_join",
                ReadFailureSource::Stream if error.raw_os_error().is_some() => "os_error",
                ReadFailureSource::Stream
                    if self.format == "zstd" && error.kind() == io::ErrorKind::Other =>
                {
                    // zstd converts its numeric error code to an io::Error containing
                    // only a message. Match exact known messages, never export that text.
                    // An unfamiliar message must remain a generic stream error.
                    match error.to_string().as_str() {
                        "Unknown frame descriptor" => "zstd_invalid_frame",
                        "Data corruption detected"
                        | "Header of Literals' block doesn't respect format specification" => {
                            "zstd_corrupt_block"
                        }
                        "Restored data doesn't match checksum" => "zstd_checksum",
                        "Frame requires too much memory for decoding"
                        | "Allocation error : not enough memory" => "zstd_resource_limit",
                        "Version not supported" | "Unsupported frame parameter" => {
                            "zstd_unsupported_frame"
                        }
                        "Dictionary is corrupted" | "Dictionary mismatch" => "zstd_dictionary",
                        _ => "stream_error",
                    }
                }
                ReadFailureSource::Stream => "stream_error",
            },
            read_progress: if self.read_any_line {
                "after_first_line"
            } else {
                "before_first_line"
            },
        });
    }
}

impl Drop for ReadMetrics {
    fn drop(&mut self) {
        let Some(metrics) = codex_otel::global() else {
            return;
        };
        let (outcome, stage, error_kind, reason, read_progress) = match &self.failure {
            Some(failure) => (
                "failed",
                failure.stage,
                failure.error_kind,
                failure.reason,
                failure.read_progress,
            ),
            None if self.reached_eof => ("eof", "none", "none", "none", "none"),
            None => ("partial", "none", "none", "none", "none"),
        };
        let tags = [
            ("format", self.format),
            ("outcome", outcome),
            ("stage", stage),
            ("error_kind", error_kind),
            ("reason", reason),
            ("read_progress", read_progress),
        ];
        let _ = metrics.counter("codex.rollout_compression.read", /*inc*/ 1, &tags);
        let _ = metrics.record_duration(
            "codex.rollout_compression.read.io_duration_ms",
            self.duration,
            &tags,
        );
    }
}
