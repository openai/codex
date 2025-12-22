use crate::metrics::MetricEvent;
use crate::metrics::sink::MetricSink;
use crate::metrics::util::error_or_panic;
use opentelemetry::KeyValue;
use opentelemetry::metrics::Histogram;
use opentelemetry::metrics::Meter;
use opentelemetry::metrics::MeterProvider;
use opentelemetry::metrics::UpDownCounter;
use opentelemetry_sdk::metrics::PeriodicReader;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::pin::Pin;

const METER_NAME: &str = "codex-otel-metrics";

#[derive(Debug)]
struct MetricRecorder {
    meter: Meter,
    counters: HashMap<String, UpDownCounter<i64>>,
    histograms: HashMap<String, Histogram<f64>>,
}

impl MetricRecorder {
    fn new(meter: Meter) -> Self {
        Self {
            meter,
            counters: HashMap::new(),
            histograms: HashMap::new(),
        }
    }

    fn record_event(&mut self, event: MetricEvent) {
        match event {
            MetricEvent::Counter { name, value, tags } => {
                self.record_counter(&name, value, &tags);
            }
            MetricEvent::Histogram { name, value, tags } => {
                self.record_histogram(&name, value, &tags);
            }
        }
    }

    fn record_counter(&mut self, name: &str, value: i64, tags: &BTreeMap<String, String>) {
        let attributes = self.attributes_for(tags);
        let name = name.to_string();
        let counter = self
            .counters
            .entry(name.clone())
            .or_insert_with(|| self.meter.i64_up_down_counter(name.clone()).build());
        counter.add(value, &attributes);
    }

    fn record_histogram(&mut self, name: &str, value: i64, tags: &BTreeMap<String, String>) {
        let attributes = self.attributes_for(tags);
        let name = name.to_string();
        let histogram = self
            .histograms
            .entry(name.clone())
            .or_insert_with(|| self.meter.f64_histogram(name.clone()).build());
        histogram.record(value as f64, &attributes);
    }

    fn attributes_for(&self, tags: &BTreeMap<String, String>) -> Vec<KeyValue> {
        tags.iter()
            .map(|(key, value)| KeyValue::new(key.clone(), value.clone()))
            .collect()
    }
}

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
