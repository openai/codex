/// Host-provided metrics capability for extension-owned behavior.
///
/// Implementations are expected to attach the host's session attribution before
/// forwarding samples to the configured metrics backend.
pub trait ExtensionMetrics: Send + Sync {
    /// Increments a counter with optional extension-provided tags.
    fn counter(&self, name: &str, inc: i64, tags: &[(&str, &str)]);

    /// Records one histogram sample with optional extension-provided tags.
    fn histogram(&self, name: &str, value: i64, tags: &[(&str, &str)]);
}
