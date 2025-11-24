use opentelemetry::otel_debug;
use std::time::Duration;
use std::{
    fmt,
    sync::{Mutex, Weak},
};

use crate::{
    error::{OTelSdkError, OTelSdkResult},
    metrics::Temporality,
};

use super::{
    data::ResourceMetrics,
    pipeline::Pipeline,
    reader::{MetricReader, SdkProducer},
};

/// A simple [MetricReader] that allows an application to read metrics on demand.
///
/// See [ManualReaderBuilder] for configuration options.
///
/// # Example
///
/// ```
/// use opentelemetry_sdk::metrics::ManualReader;
///
/// // can specify additional reader configuration
/// let reader = ManualReader::builder().build();
/// # drop(reader)
/// ```
pub struct ManualReader {
    inner: Mutex<ManualReaderInner>,
    temporality: Temporality,
}

impl Default for ManualReader {
    fn default() -> Self {
        ManualReader::builder().build()
    }
}

impl fmt::Debug for ManualReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ManualReader")
    }
}

#[derive(Debug)]
struct ManualReaderInner {
    sdk_producer: Option<Weak<dyn SdkProducer>>,
    is_shutdown: bool,
}

impl ManualReader {
    /// Configuration for this reader
    pub fn builder() -> ManualReaderBuilder {
        ManualReaderBuilder::default()
    }

    /// A [MetricReader] which is directly called to collect metrics.
    pub(crate) fn new(temporality: Temporality) -> Self {
        ManualReader {
            inner: Mutex::new(ManualReaderInner {
                sdk_producer: None,
                is_shutdown: false,
            }),
            temporality,
        }
    }
}

impl MetricReader for ManualReader {
    ///  Register a pipeline which enables the caller to read metrics from the SDK
    ///  on demand.
    fn register_pipeline(&self, pipeline: Weak<Pipeline>) {
        let _ = self.inner.lock().map(|mut inner| {
            // Only register once. If producer is already set, do nothing.
            if inner.sdk_producer.is_none() {
                inner.sdk_producer = Some(pipeline);
            } else {
                otel_debug!(
                    name: "ManualReader.DuplicateRegistration",
                    message = "The pipeline is already registered to the Reader. Registering pipeline multiple times is not allowed.");
            }
        });
    }

    /// Gathers all metrics from the SDK, calling any
    /// callbacks necessary and returning the results.
    ///
    /// Returns an error if called after shutdown.
    fn collect(&self, rm: &mut ResourceMetrics) -> OTelSdkResult {
        let inner = self
            .inner
            .lock()
            .map_err(|_| OTelSdkError::InternalFailure("Failed to lock pipeline".into()))?;

        match &inner.sdk_producer.as_ref().and_then(|w| w.upgrade()) {
            Some(producer) => producer.produce(rm)?,
            None => {
                return Err(OTelSdkError::InternalFailure(
                    "reader is shut down or not registered".into(),
                ))
            }
        };

        Ok(())
    }

    /// ForceFlush is a no-op, it always returns nil.
    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }

    /// Closes any connections and frees any resources used by the reader.
    fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
        let mut inner = self
            .inner
            .lock()
            .map_err(|e| OTelSdkError::InternalFailure(format!("Failed to acquire lock: {}", e)))?;

        // Any future call to collect will now return an error.
        inner.sdk_producer = None;
        inner.is_shutdown = true;

        Ok(())
    }

    fn temporality(&self, kind: super::InstrumentKind) -> Temporality {
        kind.temporality_preference(self.temporality)
    }
}

/// Configuration for a [ManualReader]
#[derive(Default)]
pub struct ManualReaderBuilder {
    temporality: Temporality,
}

impl fmt::Debug for ManualReaderBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ManualReaderBuilder")
    }
}

impl ManualReaderBuilder {
    /// New manual builder configuration
    pub fn new() -> Self {
        Default::default()
    }

    /// Set the [Temporality] of the exporter.
    pub fn with_temporality(mut self, temporality: Temporality) -> Self {
        self.temporality = temporality;
        self
    }

    /// Create a new [ManualReader] from this configuration.
    pub fn build(self) -> ManualReader {
        ManualReader::new(self.temporality)
    }
}
