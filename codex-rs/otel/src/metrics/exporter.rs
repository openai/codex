use opentelemetry::KeyValue;
use opentelemetry::metrics::Histogram;
use opentelemetry::metrics::Meter;
use opentelemetry::metrics::UpDownCounter;
use std::collections::BTreeMap;
use std::collections::HashMap;

pub(crate) const METER_NAME: &str = "codex-otel-metrics";

#[derive(Clone, Debug)]
pub(crate) enum MetricEvent {
    Counter {
        name: String,
        value: i64,
        tags: BTreeMap<String, String>,
    },
    Histogram {
        name: String,
        value: i64,
        tags: BTreeMap<String, String>,
    },
}

#[derive(Debug)]
pub(crate) struct MetricRecorder {
    meter: Meter,
    counters: HashMap<String, UpDownCounter<i64>>,
    histograms: HashMap<String, Histogram<f64>>,
}

impl MetricRecorder {
    pub(crate) fn new(meter: Meter) -> Self {
        Self {
            meter,
            counters: HashMap::new(),
            histograms: HashMap::new(),
        }
    }

    pub(crate) fn record_event(&mut self, event: MetricEvent) {
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
