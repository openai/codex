use crate::error::{OTelSdkError, OTelSdkResult};
use crate::{
    trace::{SpanData, SpanExporter},
    trace::{SpanEvents, SpanLinks},
    ExportError,
};
pub use opentelemetry::testing::trace::TestSpan;
use opentelemetry::{
    trace::{SpanContext, SpanId, SpanKind, Status, TraceFlags, TraceId, TraceState},
    InstrumentationScope,
};
use std::fmt::{Display, Formatter};

pub fn new_test_export_span_data() -> SpanData {
    SpanData {
        span_context: SpanContext::new(
            TraceId::from_u128(1),
            SpanId::from_u64(1),
            TraceFlags::SAMPLED,
            false,
            TraceState::default(),
        ),
        parent_span_id: SpanId::INVALID,
        span_kind: SpanKind::Internal,
        name: "opentelemetry".into(),
        start_time: opentelemetry::time::now(),
        end_time: opentelemetry::time::now(),
        attributes: Vec::new(),
        dropped_attributes_count: 0,
        events: SpanEvents::default(),
        links: SpanLinks::default(),
        status: Status::Unset,
        instrumentation_scope: InstrumentationScope::default(),
    }
}

#[derive(Debug)]
pub struct TokioSpanExporter {
    tx_export: tokio::sync::mpsc::UnboundedSender<SpanData>,
    tx_shutdown: tokio::sync::mpsc::UnboundedSender<()>,
}

impl SpanExporter for TokioSpanExporter {
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        batch.into_iter().try_for_each(|span_data| {
            self.tx_export
                .send(span_data)
                .map_err(|err| OTelSdkError::InternalFailure(format!("Export failed: {:?}", err)))
        })
    }

    fn shutdown(&mut self) -> OTelSdkResult {
        self.tx_shutdown.send(()).map_err(|_| {
            OTelSdkError::InternalFailure("Failed to send shutdown signal".to_string())
        })
    }
}

pub fn new_tokio_test_exporter() -> (
    TokioSpanExporter,
    tokio::sync::mpsc::UnboundedReceiver<SpanData>,
    tokio::sync::mpsc::UnboundedReceiver<()>,
) {
    let (tx_export, rx_export) = tokio::sync::mpsc::unbounded_channel();
    let (tx_shutdown, rx_shutdown) = tokio::sync::mpsc::unbounded_channel();
    let exporter = TokioSpanExporter {
        tx_export,
        tx_shutdown,
    };
    (exporter, rx_export, rx_shutdown)
}

#[derive(Debug)]
pub struct TestExportError(String);

impl std::error::Error for TestExportError {}

impl ExportError for TestExportError {
    fn exporter_name(&self) -> &'static str {
        "test"
    }
}

impl Display for TestExportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(any(feature = "rt-tokio", feature = "rt-tokio-current-thread"))]
impl<T> From<tokio::sync::mpsc::error::SendError<T>> for TestExportError {
    fn from(err: tokio::sync::mpsc::error::SendError<T>) -> Self {
        TestExportError(err.to_string())
    }
}

/// A no-op instance of an [`SpanExporter`].
///
/// [`SpanExporter`]: crate::trace::SpanExporter
#[derive(Debug, Default)]
pub struct NoopSpanExporter {
    _private: (),
}

impl NoopSpanExporter {
    /// Create a new noop span exporter
    pub fn new() -> Self {
        NoopSpanExporter { _private: () }
    }
}

impl SpanExporter for NoopSpanExporter {
    async fn export(&self, _: Vec<SpanData>) -> OTelSdkResult {
        Ok(())
    }
}
