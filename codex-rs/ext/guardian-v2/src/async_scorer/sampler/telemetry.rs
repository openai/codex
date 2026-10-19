//! Records sampling retries and attempts through score delivery or cancellation.
//! Retained scores carry telemetry until accepted or discarded by their consumer.

use super::LunaSamplerError;
use super::PendingScore;
use codex_extension_api::ExtensionMetrics;
use std::sync::Arc;

pub(in crate::async_scorer) struct SampledScore {
    output: String,
    telemetry: SamplingTelemetry,
}

impl SampledScore {
    pub(in crate::async_scorer) fn accept(mut self) -> String {
        self.telemetry.outcome = Some(("success", "none"));
        self.output
    }
}

pub(super) struct SamplingTelemetry {
    metrics: Option<Arc<dyn ExtensionMetrics>>,
    pending_score: Option<PendingScore>,
    pub(super) attempts: usize,
    outcome: Option<(&'static str, &'static str)>,
}
impl Drop for SamplingTelemetry {
    fn drop(&mut self) {
        if let Some(metrics) = self.metrics.as_deref() {
            let (outcome, failure_reason) =
                self.outcome.unwrap_or_else(|| self.undelivered_outcome());
            let attempt_count = self.attempts.to_string();
            let tags = [
                ("outcome", outcome),
                ("attempt_count", attempt_count.as_str()),
                ("retried", if self.attempts > 1 { "true" } else { "false" }),
                ("failure_reason", failure_reason),
            ];
            // One initial attempt, two ordinary retries and at most two auth recoveries.
            metrics.histogram_with_boundaries(
                "codex.guardian_v2.sampling.attempt_count",
                i64::try_from(self.attempts).unwrap_or(i64::MAX),
                &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
                &tags,
            );
            metrics.counter(
                "codex.guardian_v2.sampling.retry_outcome",
                /*inc*/ 1,
                &tags,
            );
        }
    }
}

impl SamplingTelemetry {
    pub(super) fn new(
        metrics: Option<Arc<dyn ExtensionMetrics>>,
        pending_score: Option<PendingScore>,
    ) -> Self {
        Self {
            metrics,
            pending_score,
            attempts: 0,
            outcome: None,
        }
    }

    fn undelivered_outcome(&self) -> (&'static str, &'static str) {
        // An expiry-first select can drop sampling before it returns Superseded.
        if self
            .pending_score
            .as_ref()
            .is_some_and(PendingScore::is_expired)
        {
            ("superseded", "superseded")
        } else {
            ("cancelled", "cancelled")
        }
    }

    pub(super) fn record_result(&mut self, result: &Result<String, LunaSamplerError>) {
        self.outcome.get_or_insert_with(|| match result {
            Ok(_) => ("success", "none"),
            Err(LunaSamplerError::Superseded) => ("superseded", "superseded"),
            Err(error) => (
                "failure",
                super::super::metrics::sampler_failure_reason(error),
            ),
        });
    }

    pub(super) fn deliver(
        &mut self,
        ready: tokio::sync::oneshot::Sender<Result<SampledScore, LunaSamplerError>>,
        result: Result<String, LunaSamplerError>,
    ) {
        if result.is_err() {
            self.record_result(&result);
        }
        let result = result.map(|output| SampledScore {
            output,
            telemetry: std::mem::replace(self, Self::new(/*metrics*/ None, /*pending_score*/ None)),
        });
        if ready.send(result).is_err() {
            self.outcome = Some(self.undelivered_outcome());
        }
    }
}

pub(super) fn record_retry(
    metrics: Option<&dyn ExtensionMetrics>,
    failure_reason: &'static str,
    kind: &'static str,
) {
    if let Some(metrics) = metrics {
        metrics.counter(
            "codex.guardian_v2.sampling.retry",
            /*inc*/ 1,
            &[("failure_reason", failure_reason), ("kind", kind)],
        );
    }
}
