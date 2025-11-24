//! Interfaces for exporting metrics

use crate::error::OTelSdkResult;
use std::time::Duration;

use crate::metrics::data::ResourceMetrics;

use super::Temporality;

/// Exporter handles the delivery of metric data to external receivers.
///
/// This is the final component in the metric push pipeline.
pub trait PushMetricExporter: Send + Sync + 'static {
    /// Export serializes and transmits metric data to a receiver.
    ///
    /// All retry logic must be contained in this function. The SDK does not
    /// implement any retry logic. All errors returned by this function are
    /// considered unrecoverable and will be logged.
    fn export(
        &self,
        metrics: &ResourceMetrics,
    ) -> impl std::future::Future<Output = OTelSdkResult> + Send;

    /// Flushes any metric data held by an exporter.
    fn force_flush(&self) -> OTelSdkResult;

    /// Releases any held computational resources.
    ///
    /// After Shutdown is called, calls to Export will perform no operation and
    /// instead will return an error indicating the shutdown state.
    fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult;

    /// Shutdown with the default timeout of 5 seconds.
    fn shutdown(&self) -> OTelSdkResult {
        self.shutdown_with_timeout(Duration::from_secs(5))
    }

    /// Access the [Temporality] of the MetricExporter.
    fn temporality(&self) -> Temporality;
}
