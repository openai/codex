use crate::error::{OTelSdkError, OTelSdkResult};
use crate::metrics::data::{
    ExponentialHistogram, Gauge, Histogram, MetricData, ResourceMetrics, Sum,
};
use crate::metrics::exporter::PushMetricExporter;
use crate::metrics::Temporality;
use crate::InMemoryExporterError;
use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::data::{AggregatedMetrics, Metric, ScopeMetrics};

/// An in-memory metrics exporter that stores metrics data in memory.
///
/// This exporter is useful for testing and debugging purposes. It stores
/// metric data in a `VecDeque<ResourceMetrics>`. Metrics can be retrieved
/// using the `get_finished_metrics` method.
///
/// # Panics
///
/// This exporter may panic
/// - if there's an issue with locking the `metrics` Mutex, such as if the Mutex is poisoned.
/// - the data point recorded is not one of [i64, u64, f64]. This shouldn't happen if used with OpenTelemetry API.
///
/// # Example
///
/// ```
///# use opentelemetry_sdk::metrics;
///# use opentelemetry::{KeyValue};
///# use opentelemetry::metrics::MeterProvider;
///# use opentelemetry_sdk::metrics::InMemoryMetricExporter;
///# use opentelemetry_sdk::metrics::PeriodicReader;
///
///# #[tokio::main]
///# async fn main() {
/// // Create an InMemoryMetricExporter
///  let exporter = InMemoryMetricExporter::default();
///
///  // Create a MeterProvider and register the exporter
///  let meter_provider = metrics::SdkMeterProvider::builder()
///      .with_reader(PeriodicReader::builder(exporter.clone()).build())
///      .build();
///
///  // Create and record metrics using the MeterProvider
///  let meter = meter_provider.meter("example");
///  let counter = meter.u64_counter("my_counter").build();
///  counter.add(1, &[KeyValue::new("key", "value")]);
///
///  meter_provider.force_flush().unwrap();
///
///  // Retrieve the finished metrics from the exporter
///  let finished_metrics = exporter.get_finished_metrics().unwrap();
///
///  // Print the finished metrics
/// for resource_metrics in finished_metrics {
///      println!("{:?}", resource_metrics);
///  }
///# }
/// ```
pub struct InMemoryMetricExporter {
    metrics: Arc<Mutex<VecDeque<ResourceMetrics>>>,
    temporality: Temporality,
}

impl Clone for InMemoryMetricExporter {
    fn clone(&self) -> Self {
        InMemoryMetricExporter {
            metrics: self.metrics.clone(),
            temporality: self.temporality,
        }
    }
}

impl fmt::Debug for InMemoryMetricExporter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InMemoryMetricExporter").finish()
    }
}

impl Default for InMemoryMetricExporter {
    fn default() -> Self {
        InMemoryMetricExporterBuilder::new().build()
    }
}

/// Builder for [`InMemoryMetricExporter`].
/// # Example
///
/// ```
/// # use opentelemetry_sdk::metrics::{InMemoryMetricExporter, InMemoryMetricExporterBuilder};
///
/// let exporter = InMemoryMetricExporterBuilder::new().build();
/// ```
pub struct InMemoryMetricExporterBuilder {
    temporality: Option<Temporality>,
}

impl fmt::Debug for InMemoryMetricExporterBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InMemoryMetricExporterBuilder").finish()
    }
}

impl Default for InMemoryMetricExporterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryMetricExporterBuilder {
    /// Creates a new instance of the `InMemoryMetricExporterBuilder`.
    pub fn new() -> Self {
        Self { temporality: None }
    }

    /// Set the [Temporality] of the exporter.
    pub fn with_temporality(mut self, temporality: Temporality) -> Self {
        self.temporality = Some(temporality);
        self
    }

    /// Creates a new instance of the `InMemoryMetricExporter`.
    ///
    pub fn build(self) -> InMemoryMetricExporter {
        InMemoryMetricExporter {
            metrics: Arc::new(Mutex::new(VecDeque::new())),
            temporality: self.temporality.unwrap_or_default(),
        }
    }
}

impl InMemoryMetricExporter {
    /// Returns the finished metrics as a vector of `ResourceMetrics`.
    ///
    /// # Errors
    ///
    /// Returns a `MetricError` if the internal lock cannot be acquired.
    ///
    /// # Example
    ///
    /// ```
    /// # use opentelemetry_sdk::metrics::InMemoryMetricExporter;
    ///
    /// let exporter = InMemoryMetricExporter::default();
    /// let finished_metrics = exporter.get_finished_metrics().unwrap();
    /// ```
    pub fn get_finished_metrics(&self) -> Result<Vec<ResourceMetrics>, InMemoryExporterError> {
        let metrics = self
            .metrics
            .lock()
            .map(|metrics_guard| metrics_guard.iter().map(Self::clone_metrics).collect())
            .map_err(InMemoryExporterError::from)?;
        Ok(metrics)
    }

    /// Clears the internal storage of finished metrics.
    ///
    /// # Example
    ///
    /// ```
    /// # use opentelemetry_sdk::metrics::InMemoryMetricExporter;
    ///
    /// let exporter = InMemoryMetricExporter::default();
    /// exporter.reset();
    /// ```
    pub fn reset(&self) {
        let _ = self
            .metrics
            .lock()
            .map(|mut metrics_guard| metrics_guard.clear());
    }

    fn clone_metrics(metric: &ResourceMetrics) -> ResourceMetrics {
        ResourceMetrics {
            resource: metric.resource.clone(),
            scope_metrics: metric
                .scope_metrics
                .iter()
                .map(|scope_metric| ScopeMetrics {
                    scope: scope_metric.scope.clone(),
                    metrics: scope_metric
                        .metrics
                        .iter()
                        .map(|metric| Metric {
                            name: metric.name.clone(),
                            description: metric.description.clone(),
                            unit: metric.unit.clone(),
                            data: Self::clone_data(&metric.data),
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    fn clone_data(data: &AggregatedMetrics) -> AggregatedMetrics {
        fn clone_inner<T: Clone>(data: &MetricData<T>) -> MetricData<T> {
            match data {
                MetricData::Gauge(gauge) => Gauge {
                    data_points: gauge.data_points.clone(),
                    start_time: gauge.start_time,
                    time: gauge.time,
                }
                .into(),
                MetricData::Sum(sum) => Sum {
                    data_points: sum.data_points.clone(),
                    start_time: sum.start_time,
                    time: sum.time,
                    temporality: sum.temporality,
                    is_monotonic: sum.is_monotonic,
                }
                .into(),
                MetricData::Histogram(histogram) => Histogram {
                    data_points: histogram.data_points.clone(),
                    start_time: histogram.start_time,
                    time: histogram.time,
                    temporality: histogram.temporality,
                }
                .into(),
                MetricData::ExponentialHistogram(exponential_histogram) => ExponentialHistogram {
                    data_points: exponential_histogram.data_points.clone(),
                    start_time: exponential_histogram.start_time,
                    time: exponential_histogram.time,
                    temporality: exponential_histogram.temporality,
                }
                .into(),
            }
        }
        match data {
            AggregatedMetrics::F64(metric_data) => AggregatedMetrics::F64(clone_inner(metric_data)),
            AggregatedMetrics::U64(metric_data) => AggregatedMetrics::U64(clone_inner(metric_data)),
            AggregatedMetrics::I64(metric_data) => AggregatedMetrics::I64(clone_inner(metric_data)),
        }
    }
}

impl PushMetricExporter for InMemoryMetricExporter {
    async fn export(&self, metrics: &ResourceMetrics) -> OTelSdkResult {
        self.metrics
            .lock()
            .map(|mut metrics_guard| {
                metrics_guard.push_back(InMemoryMetricExporter::clone_metrics(metrics))
            })
            .map_err(|_| OTelSdkError::InternalFailure("Failed to lock metrics".to_string()))
    }

    fn force_flush(&self) -> OTelSdkResult {
        Ok(()) // In this implementation, flush does nothing
    }

    fn shutdown(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
        Ok(())
    }

    fn temporality(&self) -> Temporality {
        self.temporality
    }
}
