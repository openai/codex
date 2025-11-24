// The prost currently will generate a non optional deprecated field for labels.
// We cannot assign value to it otherwise clippy will complain.
// We cannot ignore it as it's not an optional field.
// We can remove this after we removed the labels field from proto.
#[allow(deprecated)]
#[cfg(feature = "gen-tonic-messages")]
pub mod tonic {
    use std::fmt::Debug;

    use opentelemetry::{otel_debug, Key, Value};
    use opentelemetry_sdk::metrics::data::{
        AggregatedMetrics, Exemplar as SdkExemplar,
        ExponentialHistogram as SdkExponentialHistogram, Gauge as SdkGauge,
        Histogram as SdkHistogram, Metric as SdkMetric, MetricData, ResourceMetrics,
        ScopeMetrics as SdkScopeMetrics, Sum as SdkSum,
    };
    use opentelemetry_sdk::metrics::Temporality;
    use opentelemetry_sdk::Resource as SdkResource;

    use crate::proto::tonic::{
        collector::metrics::v1::ExportMetricsServiceRequest,
        common::v1::KeyValue,
        metrics::v1::{
            exemplar, exemplar::Value as TonicExemplarValue,
            exponential_histogram_data_point::Buckets as TonicBuckets,
            metric::Data as TonicMetricData, number_data_point,
            number_data_point::Value as TonicDataPointValue,
            AggregationTemporality as TonicTemporality, AggregationTemporality,
            DataPointFlags as TonicDataPointFlags, Exemplar as TonicExemplar,
            ExponentialHistogram as TonicExponentialHistogram,
            ExponentialHistogramDataPoint as TonicExponentialHistogramDataPoint,
            Gauge as TonicGauge, Histogram as TonicHistogram,
            HistogramDataPoint as TonicHistogramDataPoint, Metric as TonicMetric,
            NumberDataPoint as TonicNumberDataPoint, ResourceMetrics as TonicResourceMetrics,
            ScopeMetrics as TonicScopeMetrics, Sum as TonicSum,
        },
        resource::v1::Resource as TonicResource,
    };
    use crate::transform::common::to_nanos;

    impl From<u64> for exemplar::Value {
        fn from(value: u64) -> Self {
            exemplar::Value::AsInt(i64::try_from(value).unwrap_or_default())
        }
    }

    impl From<i64> for exemplar::Value {
        fn from(value: i64) -> Self {
            exemplar::Value::AsInt(value)
        }
    }

    impl From<f64> for exemplar::Value {
        fn from(value: f64) -> Self {
            exemplar::Value::AsDouble(value)
        }
    }

    impl From<u64> for number_data_point::Value {
        fn from(value: u64) -> Self {
            number_data_point::Value::AsInt(i64::try_from(value).unwrap_or_default())
        }
    }

    impl From<i64> for number_data_point::Value {
        fn from(value: i64) -> Self {
            number_data_point::Value::AsInt(value)
        }
    }

    impl From<f64> for number_data_point::Value {
        fn from(value: f64) -> Self {
            number_data_point::Value::AsDouble(value)
        }
    }

    impl From<(&Key, &Value)> for KeyValue {
        fn from(kv: (&Key, &Value)) -> Self {
            KeyValue {
                key: kv.0.to_string(),
                value: Some(kv.1.clone().into()),
            }
        }
    }

    impl From<&opentelemetry::KeyValue> for KeyValue {
        fn from(kv: &opentelemetry::KeyValue) -> Self {
            KeyValue {
                key: kv.key.to_string(),
                value: Some(kv.value.clone().into()),
            }
        }
    }

    impl From<Temporality> for AggregationTemporality {
        fn from(temporality: Temporality) -> Self {
            match temporality {
                Temporality::Cumulative => AggregationTemporality::Cumulative,
                Temporality::Delta => AggregationTemporality::Delta,
                other => {
                    otel_debug!(
                        name: "AggregationTemporality::Unknown",
                        message = "Unknown temporality,using default instead.",
                        unknown_temporality = format!("{:?}", other),
                        default_temporality = format!("{:?}", Temporality::Cumulative)
                    );
                    AggregationTemporality::Cumulative
                }
            }
        }
    }

    impl From<&ResourceMetrics> for ExportMetricsServiceRequest {
        fn from(rm: &ResourceMetrics) -> Self {
            ExportMetricsServiceRequest {
                resource_metrics: vec![TonicResourceMetrics {
                    resource: Some((rm.resource()).into()),
                    scope_metrics: rm.scope_metrics().map(Into::into).collect(),
                    schema_url: rm
                        .resource()
                        .schema_url()
                        .map(Into::into)
                        .unwrap_or_default(),
                }],
            }
        }
    }

    impl From<&SdkResource> for TonicResource {
        fn from(resource: &SdkResource) -> Self {
            TonicResource {
                attributes: resource.iter().map(Into::into).collect(),
                dropped_attributes_count: 0,
                entity_refs: vec![], // internal and currently unused
            }
        }
    }

    impl From<&SdkScopeMetrics> for TonicScopeMetrics {
        fn from(sm: &SdkScopeMetrics) -> Self {
            TonicScopeMetrics {
                scope: Some((sm.scope(), None).into()),
                metrics: sm.metrics().map(Into::into).collect(),
                schema_url: sm
                    .scope()
                    .schema_url()
                    .map(ToOwned::to_owned)
                    .unwrap_or_default(),
            }
        }
    }

    impl From<&SdkMetric> for TonicMetric {
        fn from(metric: &SdkMetric) -> Self {
            TonicMetric {
                name: metric.name().to_string(),
                description: metric.description().to_string(),
                unit: metric.unit().to_string(),
                metadata: vec![], // internal and currently unused
                data: Some(match metric.data() {
                    AggregatedMetrics::F64(data) => data.into(),
                    AggregatedMetrics::U64(data) => data.into(),
                    AggregatedMetrics::I64(data) => data.into(),
                }),
            }
        }
    }

    impl<T> From<&MetricData<T>> for TonicMetricData
    where
        T: Numeric + Debug,
    {
        fn from(data: &MetricData<T>) -> Self {
            match data {
                MetricData::Gauge(gauge) => TonicMetricData::Gauge(gauge.into()),
                MetricData::Sum(sum) => TonicMetricData::Sum(sum.into()),
                MetricData::Histogram(hist) => TonicMetricData::Histogram(hist.into()),
                MetricData::ExponentialHistogram(hist) => {
                    TonicMetricData::ExponentialHistogram(hist.into())
                }
            }
        }
    }

    trait Numeric: Into<TonicExemplarValue> + Into<TonicDataPointValue> + Copy {
        // lossy at large values for u64 and i64 but otlp histograms only handle float values
        fn into_f64(self) -> f64;
    }

    impl Numeric for u64 {
        fn into_f64(self) -> f64 {
            self as f64
        }
    }

    impl Numeric for i64 {
        fn into_f64(self) -> f64 {
            self as f64
        }
    }

    impl Numeric for f64 {
        fn into_f64(self) -> f64 {
            self
        }
    }

    impl<T> From<&SdkHistogram<T>> for TonicHistogram
    where
        T: Numeric,
    {
        fn from(hist: &SdkHistogram<T>) -> Self {
            TonicHistogram {
                data_points: hist
                    .data_points()
                    .map(|dp| TonicHistogramDataPoint {
                        attributes: dp.attributes().map(Into::into).collect(),
                        start_time_unix_nano: to_nanos(hist.start_time()),
                        time_unix_nano: to_nanos(hist.time()),
                        count: dp.count(),
                        sum: Some(dp.sum().into_f64()),
                        bucket_counts: dp.bucket_counts().collect(),
                        explicit_bounds: dp.bounds().collect(),
                        exemplars: dp.exemplars().map(Into::into).collect(),
                        flags: TonicDataPointFlags::default() as u32,
                        min: dp.min().map(Numeric::into_f64),
                        max: dp.max().map(Numeric::into_f64),
                    })
                    .collect(),
                aggregation_temporality: TonicTemporality::from(hist.temporality()).into(),
            }
        }
    }

    impl<T> From<&SdkExponentialHistogram<T>> for TonicExponentialHistogram
    where
        T: Numeric,
    {
        fn from(hist: &SdkExponentialHistogram<T>) -> Self {
            TonicExponentialHistogram {
                data_points: hist
                    .data_points()
                    .map(|dp| TonicExponentialHistogramDataPoint {
                        attributes: dp.attributes().map(Into::into).collect(),
                        start_time_unix_nano: to_nanos(hist.start_time()),
                        time_unix_nano: to_nanos(hist.time()),
                        count: dp.count() as u64,
                        sum: Some(dp.sum().into_f64()),
                        scale: dp.scale().into(),
                        zero_count: dp.zero_count(),
                        positive: Some(TonicBuckets {
                            offset: dp.positive_bucket().offset(),
                            bucket_counts: dp.positive_bucket().counts().collect(),
                        }),
                        negative: Some(TonicBuckets {
                            offset: dp.negative_bucket().offset(),
                            bucket_counts: dp.negative_bucket().counts().collect(),
                        }),
                        flags: TonicDataPointFlags::default() as u32,
                        exemplars: dp.exemplars().map(Into::into).collect(),
                        min: dp.min().map(Numeric::into_f64),
                        max: dp.max().map(Numeric::into_f64),
                        zero_threshold: dp.zero_threshold(),
                    })
                    .collect(),
                aggregation_temporality: TonicTemporality::from(hist.temporality()).into(),
            }
        }
    }

    impl<T> From<&SdkSum<T>> for TonicSum
    where
        T: Debug + Into<TonicExemplarValue> + Into<TonicDataPointValue> + Copy,
    {
        fn from(sum: &SdkSum<T>) -> Self {
            TonicSum {
                data_points: sum
                    .data_points()
                    .map(|dp| TonicNumberDataPoint {
                        attributes: dp.attributes().map(Into::into).collect(),
                        start_time_unix_nano: to_nanos(sum.start_time()),
                        time_unix_nano: to_nanos(sum.time()),
                        exemplars: dp.exemplars().map(Into::into).collect(),
                        flags: TonicDataPointFlags::default() as u32,
                        value: Some(dp.value().into()),
                    })
                    .collect(),
                aggregation_temporality: TonicTemporality::from(sum.temporality()).into(),
                is_monotonic: sum.is_monotonic(),
            }
        }
    }

    impl<T> From<&SdkGauge<T>> for TonicGauge
    where
        T: Debug + Into<TonicExemplarValue> + Into<TonicDataPointValue> + Copy,
    {
        fn from(gauge: &SdkGauge<T>) -> Self {
            TonicGauge {
                data_points: gauge
                    .data_points()
                    .map(|dp| TonicNumberDataPoint {
                        attributes: dp.attributes().map(Into::into).collect(),
                        start_time_unix_nano: gauge.start_time().map(to_nanos).unwrap_or_default(),
                        time_unix_nano: to_nanos(gauge.time()),
                        exemplars: dp.exemplars().map(Into::into).collect(),
                        flags: TonicDataPointFlags::default() as u32,
                        value: Some(dp.value().into()),
                    })
                    .collect(),
            }
        }
    }

    impl<T> From<&SdkExemplar<T>> for TonicExemplar
    where
        T: Into<TonicExemplarValue> + Copy,
    {
        fn from(ex: &SdkExemplar<T>) -> Self {
            TonicExemplar {
                filtered_attributes: ex
                    .filtered_attributes()
                    .map(|kv| (&kv.key, &kv.value).into())
                    .collect(),
                time_unix_nano: to_nanos(ex.time()),
                span_id: ex.span_id().into(),
                trace_id: ex.trace_id().into(),
                value: Some(ex.value.into()),
            }
        }
    }
}
