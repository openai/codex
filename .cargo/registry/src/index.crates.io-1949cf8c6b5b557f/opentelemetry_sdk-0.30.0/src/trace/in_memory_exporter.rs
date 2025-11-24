use crate::error::{OTelSdkError, OTelSdkResult};
use crate::resource::Resource;
use crate::trace::{SpanData, SpanExporter};
use crate::InMemoryExporterError;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// An in-memory span exporter that stores span data in memory.
///
/// This exporter is useful for testing and debugging purposes. It stores
/// metric data in a `Vec<SpanData>`. Metrics can be retrieved
/// using the `get_finished_spans` method.
/// # Example
/// ```
///# use opentelemetry::trace::{SpanKind, TraceContextExt};
///# use opentelemetry::{global, trace::Tracer, Context};
///# use opentelemetry_sdk::propagation::TraceContextPropagator;
///# use opentelemetry_sdk::runtime;
///# use opentelemetry_sdk::trace::InMemorySpanExporterBuilder;
///# use opentelemetry_sdk::trace::{BatchSpanProcessor, SdkTracerProvider};
///
///# #[tokio::main]
///# async fn main() {
///     let exporter = InMemorySpanExporterBuilder::new().build();
///     let provider = SdkTracerProvider::builder()
///         .with_span_processor(BatchSpanProcessor::builder(exporter.clone()).build())
///         .build();
///
///     global::set_tracer_provider(provider.clone());
///
///     let tracer = global::tracer("example/in_memory_exporter");
///     let span = tracer
///         .span_builder("say hello")
///         .with_kind(SpanKind::Server)
///         .start(&tracer);
///
///     let cx = Context::current_with_span(span);
///     cx.span().add_event("handling this...", Vec::new());
///     cx.span().end();
///
///     if let Err(e) = provider.force_flush() {
///         println!("{:?}", e)
///     }
///     let spans = exporter.get_finished_spans().unwrap();
///     for span in spans {
///         println!("{:?}", span)
///     }
///# }
/// ```
#[derive(Clone, Debug)]
pub struct InMemorySpanExporter {
    spans: Arc<Mutex<Vec<SpanData>>>,
    resource: Arc<Mutex<Resource>>,
}

impl Default for InMemorySpanExporter {
    fn default() -> Self {
        InMemorySpanExporterBuilder::new().build()
    }
}

/// Builder for [`InMemorySpanExporter`].
/// # Example
/// ```
///# use opentelemetry_sdk::trace::InMemorySpanExporterBuilder;
///
/// let exporter = InMemorySpanExporterBuilder::new().build();
/// ```
#[derive(Clone, Debug)]
pub struct InMemorySpanExporterBuilder {}

impl Default for InMemorySpanExporterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemorySpanExporterBuilder {
    /// Creates a new instance of the `InMemorySpanExporterBuilder`.
    pub fn new() -> Self {
        Self {}
    }

    /// Creates a new instance of the `InMemorySpanExporter`.
    pub fn build(&self) -> InMemorySpanExporter {
        InMemorySpanExporter {
            spans: Arc::new(Mutex::new(Vec::new())),
            resource: Arc::new(Mutex::new(Resource::builder().build())),
        }
    }
}

impl InMemorySpanExporter {
    /// Returns the finished span as a vector of `SpanData`.
    ///
    /// # Errors
    ///
    /// Returns a `TraceError` if the internal lock cannot be acquired.
    ///
    /// # Example
    ///
    /// ```
    /// # use opentelemetry_sdk::trace::InMemorySpanExporter;
    ///
    /// let exporter = InMemorySpanExporter::default();
    /// let finished_spans = exporter.get_finished_spans().unwrap();
    /// ```
    pub fn get_finished_spans(&self) -> Result<Vec<SpanData>, InMemoryExporterError> {
        let spans = self
            .spans
            .lock()
            .map(|spans_guard| spans_guard.iter().cloned().collect())
            .map_err(InMemoryExporterError::from)?;
        Ok(spans)
    }

    /// Clears the internal storage of finished spans.
    ///
    /// # Example
    ///
    /// ```
    /// # use opentelemetry_sdk::trace::InMemorySpanExporter;
    ///
    /// let exporter = InMemorySpanExporter::default();
    /// exporter.reset();
    /// ```
    pub fn reset(&self) {
        let _ = self.spans.lock().map(|mut spans_guard| spans_guard.clear());
    }
}

impl SpanExporter for InMemorySpanExporter {
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        let result = self
            .spans
            .lock()
            .map(|mut spans_guard| spans_guard.append(&mut batch.clone()))
            .map_err(|err| {
                OTelSdkError::InternalFailure(format!("Failed to lock spans: {:?}", err))
            });
        result
    }

    fn shutdown_with_timeout(&mut self, _timeout: Duration) -> OTelSdkResult {
        self.reset();
        Ok(())
    }

    fn set_resource(&mut self, resource: &Resource) {
        self.resource
            .lock()
            .map(|mut res_guard| *res_guard = resource.clone())
            .expect("Resource lock poisoned");
    }
}
