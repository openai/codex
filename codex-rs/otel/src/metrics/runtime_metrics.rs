use crate::metrics::names::API_CALL_COUNT_METRIC;
use crate::metrics::names::API_CALL_DURATION_METRIC;
use crate::metrics::names::SSE_EVENT_COUNT_METRIC;
use crate::metrics::names::SSE_EVENT_DURATION_METRIC;
use crate::metrics::names::TOOL_CALL_COUNT_METRIC;
use crate::metrics::names::TOOL_CALL_DURATION_METRIC;
use crate::metrics::names::WEBSOCKET_EVENT_COUNT_METRIC;
use crate::metrics::names::WEBSOCKET_EVENT_DURATION_METRIC;
use crate::metrics::names::WEBSOCKET_REQUEST_COUNT_METRIC;
use crate::metrics::names::WEBSOCKET_REQUEST_DURATION_METRIC;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::Metric;
use opentelemetry_sdk::metrics::data::MetricData;
use opentelemetry_sdk::metrics::data::ResourceMetrics;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeMetricTotals {
    pub count: u64,
    pub duration_ms: u64,
}

impl RuntimeMetricTotals {
    pub fn is_empty(self) -> bool {
        self.count == 0 && self.duration_ms == 0
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeMetricsSummary {
    pub tool_calls: RuntimeMetricTotals,
    pub api_calls: RuntimeMetricTotals,
    pub streaming_events: RuntimeMetricTotals,
    pub websocket_calls: RuntimeMetricTotals,
    pub websocket_events: RuntimeMetricTotals,
}

impl RuntimeMetricsSummary {
    pub fn is_empty(self) -> bool {
        self.tool_calls.is_empty()
            && self.api_calls.is_empty()
            && self.streaming_events.is_empty()
            && self.websocket_calls.is_empty()
            && self.websocket_events.is_empty()
    }

    pub(crate) fn from_snapshot(snapshot: &ResourceMetrics) -> Self {
        let tool_calls = RuntimeMetricTotals {
            count: sum_counter(snapshot, TOOL_CALL_COUNT_METRIC),
            duration_ms: sum_histogram_ms(snapshot, TOOL_CALL_DURATION_METRIC),
        };
        let api_calls = RuntimeMetricTotals {
            count: sum_counter(snapshot, API_CALL_COUNT_METRIC),
            duration_ms: sum_histogram_ms(snapshot, API_CALL_DURATION_METRIC),
        };
        let streaming_events = RuntimeMetricTotals {
            count: sum_counter(snapshot, SSE_EVENT_COUNT_METRIC),
            duration_ms: sum_histogram_ms(snapshot, SSE_EVENT_DURATION_METRIC),
        };
        let websocket_calls = RuntimeMetricTotals {
            count: sum_counter(snapshot, WEBSOCKET_REQUEST_COUNT_METRIC),
            duration_ms: sum_histogram_ms(snapshot, WEBSOCKET_REQUEST_DURATION_METRIC),
        };
        let websocket_events = RuntimeMetricTotals {
            count: sum_counter(snapshot, WEBSOCKET_EVENT_COUNT_METRIC),
            duration_ms: sum_histogram_ms(snapshot, WEBSOCKET_EVENT_DURATION_METRIC),
        };
        Self {
            tool_calls,
            api_calls,
            streaming_events,
            websocket_calls,
            websocket_events,
        }
    }
}

fn sum_counter(snapshot: &ResourceMetrics, name: &str) -> u64 {
    snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
        .filter(|metric| metric.name() == name)
        .map(sum_counter_metric)
        .sum()
}

fn sum_counter_metric(metric: &Metric) -> u64 {
    match metric.data() {
        AggregatedMetrics::U64(MetricData::Sum(sum)) => sum
            .data_points()
            .map(opentelemetry_sdk::metrics::data::SumDataPoint::value)
            .sum(),
        _ => 0,
    }
}

fn sum_histogram_ms(snapshot: &ResourceMetrics, name: &str) -> u64 {
    snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
        .filter(|metric| metric.name() == name)
        .map(sum_histogram_metric_ms)
        .sum()
}

fn sum_histogram_metric_ms(metric: &Metric) -> u64 {
    match metric.data() {
        AggregatedMetrics::F64(MetricData::Histogram(histogram)) => histogram
            .data_points()
            .map(|point| f64_to_u64(point.sum()))
            .sum(),
        _ => 0,
    }
}

fn f64_to_u64(value: f64) -> u64 {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    let clamped = value.min(u64::MAX as f64);
    clamped.round() as u64
}
