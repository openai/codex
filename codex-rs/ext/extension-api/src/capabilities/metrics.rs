/// Host-provided metrics capability for extension-owned behavior.
///
/// Implementations are expected to attach the host's session attribution before
/// forwarding samples to the configured metrics backend.
pub trait ExtensionMetrics: Send + Sync {
    /// Increments a counter with optional extension-provided tags.
    fn counter(&self, name: &str, inc: i64, tags: &[(&str, &str)]);

    /// Records one histogram sample with optional extension-provided tags.
    fn histogram(&self, name: &str, value: i64, tags: &[(&str, &str)]);

    /// Records a histogram with explicit buckets, preserving host attribution.
    /// All callers of the same metric name must use the same boundaries.
    fn histogram_with_boundaries(
        &self,
        name: &str,
        value: i64,
        boundaries: &[f64],
        tags: &[(&str, &str)],
    );
}

/// Records a published catalog size, with partial results taking precedence over warnings.
pub fn record_catalog_entries(
    metrics: Option<&dyn ExtensionMetrics>,
    name: &str,
    stage: &str,
    entry_count: usize,
    is_partial: bool,
    warning_count: usize,
) {
    let Some(metrics) = metrics else {
        return;
    };
    let status = if is_partial {
        "partial"
    } else if warning_count > 0 {
        "warning"
    } else {
        "complete"
    };
    metrics.histogram_with_boundaries(
        name,
        i64::try_from(entry_count).unwrap_or(i64::MAX),
        &[0., 1., 2., 5., 10., 20., 50., 100., 200., 500., 1000.],
        &[("stage", stage), ("status", status)],
    );
}
