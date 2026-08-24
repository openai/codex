use std::collections::BTreeMap;

use codex_extension_api::ExtensionMetrics;

pub(crate) const CLASSIFICATION_TRUNCATION_METRIC: &str =
    "codex.guardian_v2.classification.truncation";
pub(crate) const CLASSIFICATION_TRUNCATION_BYTES_METRIC: &str =
    "codex.guardian_v2.classification.truncation.bytes";

pub(crate) struct TruncationObservation {
    pub(crate) component: &'static str,
    pub(crate) original_bytes: usize,
    pub(crate) retained_bytes: usize,
}

/// Aggregates actual context reductions once per component and classifier request.
#[derive(Default)]
pub(crate) struct ClassificationTruncations {
    totals: BTreeMap<(&'static str, &'static str), (usize, usize)>,
}

impl ClassificationTruncations {
    pub(crate) fn record(
        &mut self,
        component: &'static str,
        original_bytes: usize,
        retained_bytes: usize,
    ) {
        if original_bytes <= retained_bytes {
            return;
        }

        let disposition = if retained_bytes == 0 {
            "omitted"
        } else {
            "truncated"
        };
        let totals = self.totals.entry((component, disposition)).or_default();
        totals.0 = totals.0.saturating_add(original_bytes);
        totals.1 = totals.1.saturating_add(retained_bytes);
    }

    pub(crate) fn emit(&self, metrics: Option<&dyn ExtensionMetrics>) {
        let Some(metrics) = metrics else {
            return;
        };

        for (&(component, disposition), &(original_bytes, retained_bytes)) in &self.totals {
            let tags = [("component", component), ("disposition", disposition)];
            metrics.counter(CLASSIFICATION_TRUNCATION_METRIC, /*inc*/ 1, &tags);

            for (measurement, bytes) in [
                ("original", original_bytes),
                ("retained", retained_bytes),
                ("omitted", original_bytes.saturating_sub(retained_bytes)),
            ] {
                let tags = [
                    ("component", component),
                    ("disposition", disposition),
                    ("measurement", measurement),
                ];
                metrics.histogram(
                    CLASSIFICATION_TRUNCATION_BYTES_METRIC,
                    i64::try_from(bytes).unwrap_or(i64::MAX),
                    &tags,
                );
            }
        }
    }
}

impl Extend<TruncationObservation> for ClassificationTruncations {
    fn extend<T: IntoIterator<Item = TruncationObservation>>(&mut self, observations: T) {
        for observation in observations {
            self.record(
                observation.component,
                observation.original_bytes,
                observation.retained_bytes,
            );
        }
    }
}
