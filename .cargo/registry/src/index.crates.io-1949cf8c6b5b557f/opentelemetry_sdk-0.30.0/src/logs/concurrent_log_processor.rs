use opentelemetry::{otel_info, InstrumentationScope};
use std::time::Duration;

use crate::{error::OTelSdkResult, Resource};

use super::{LogBatch, LogExporter, LogProcessor, SdkLogRecord};

/// A concurrent log processor calls exporter's export method on each emit. This
/// processor does not buffer logs. Note: This invokes exporter's export method
/// on the current thread without synchronization. i.e multiple export() calls
/// can happen simultaneously from different threads. This is not a problem if
/// the exporter is designed to handle that. As of now, exporters in the
/// opentelemetry-rust project (stdout/otlp) are not thread-safe.
/// This is intended to be used when exporting to operating system
/// tracing facilities like Windows ETW, Linux TracePoints etc.
#[derive(Debug)]
pub struct SimpleConcurrentLogProcessor<T: LogExporter> {
    exporter: T,
}

impl<T: LogExporter> SimpleConcurrentLogProcessor<T> {
    /// Creates a new `ConcurrentExportProcessor` with the given exporter.
    pub fn new(exporter: T) -> Self {
        Self { exporter }
    }
}

impl<T: LogExporter> LogProcessor for SimpleConcurrentLogProcessor<T> {
    fn emit(&self, record: &mut SdkLogRecord, instrumentation: &InstrumentationScope) {
        let log_tuple = &[(record as &SdkLogRecord, instrumentation)];
        let result = futures_executor::block_on(self.exporter.export(LogBatch::new(log_tuple)));
        if let Err(err) = result {
            otel_info!(
                name: "SimpleConcurrentLogProcessor.Emit.ExportError",
                error = format!("{}",err)
            );
        }
    }

    fn force_flush(&self) -> OTelSdkResult {
        // TODO: invoke flush on exporter
        // once https://github.com/open-telemetry/opentelemetry-rust/issues/2261
        // is resolved
        Ok(())
    }

    fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult {
        self.exporter.shutdown_with_timeout(timeout)
    }

    #[cfg(feature = "spec_unstable_logs_enabled")]
    #[inline]
    fn event_enabled(
        &self,
        level: opentelemetry::logs::Severity,
        target: &str,
        name: Option<&str>,
    ) -> bool {
        self.exporter.event_enabled(level, target, name)
    }

    fn set_resource(&mut self, resource: &Resource) {
        self.exporter.set_resource(resource);
    }
}
