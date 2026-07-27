/// Host-provided metrics capability for extension-owned behavior.
///
/// Implementations are expected to attach the host's session attribution before
/// forwarding samples to the configured metrics backend.
pub trait ExtensionMetrics: Send + Sync {
    /// Records one histogram sample with optional extension-provided tags.
    fn histogram(&self, name: &str, value: i64, tags: &[(&str, &str)]);
}
