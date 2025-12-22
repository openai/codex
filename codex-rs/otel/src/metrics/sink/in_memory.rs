use crate::metrics::exporter::METER_NAME;
use crate::metrics::exporter::MetricEvent;
use crate::metrics::exporter::MetricRecorder;
use crate::metrics::sink::MetricSink;
use crate::metrics::util::error_or_panic;
use opentelemetry::metrics::MeterProvider;
use opentelemetry_sdk::metrics::PeriodicReader;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use std::pin::Pin;

pub(crate) struct InMemoryExporter {
    recorder: MetricRecorder,
    meter_provider: SdkMeterProvider,
}

impl InMemoryExporter {
    pub(crate) fn from(exporter: opentelemetry_sdk::metrics::InMemoryMetricExporter) -> Self {
        let reader = PeriodicReader::builder(exporter).build();
        let meter_provider = SdkMeterProvider::builder().with_reader(reader).build();
        let meter = meter_provider.meter(METER_NAME);
        let recorder = MetricRecorder::new(meter);
        Self {
            recorder,
            meter_provider,
        }
    }
}

impl MetricSink for InMemoryExporter {
    fn export_batch<'a>(
        &'a mut self,
        events: Vec<MetricEvent>,
    ) -> Pin<Box<dyn Future<Output = crate::metrics::Result<()>> + Send + 'a>> {
        Box::pin(async move {
            for event in events {
                self.recorder.record_event(event);
            }
            if let Err(err) = self.meter_provider.force_flush() {
                error_or_panic(format!("metrics flush failed: {err}"));
            }
            Ok(())
        })
    }

    fn shutdown<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = crate::metrics::Result<()>> + Send + 'a>> {
        Box::pin(async move {
            if let Err(err) = self.meter_provider.force_flush() {
                error_or_panic(format!("metrics flush failed during shutdown: {err}"));
            }
            if let Err(err) = self.meter_provider.shutdown() {
                error_or_panic(format!("metrics shutdown failed: {err}"));
            }
            Ok(())
        })
    }
}
