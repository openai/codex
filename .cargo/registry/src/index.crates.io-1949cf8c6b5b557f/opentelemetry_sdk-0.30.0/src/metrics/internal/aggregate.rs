use std::{
    marker,
    mem::replace,
    ops::DerefMut,
    sync::{Arc, Mutex},
    time::SystemTime,
};

use crate::metrics::{data::AggregatedMetrics, Temporality};
use opentelemetry::time::now;
use opentelemetry::KeyValue;

use super::{
    exponential_histogram::ExpoHistogram, histogram::Histogram, last_value::LastValue,
    precomputed_sum::PrecomputedSum, sum::Sum, Number,
};

/// Receives measurements to be aggregated.
pub(crate) trait Measure<T>: Send + Sync + 'static {
    fn call(&self, measurement: T, attrs: &[KeyValue]);
}

/// Stores the aggregate of measurements into the aggregation and returns the number
/// of aggregate data-points output.
pub(crate) trait ComputeAggregation: Send + Sync + 'static {
    /// Compute the new aggregation and store in `dest`.
    ///
    /// If no initial aggregation exists, `dest` will be `None`, in which case the
    /// returned option is expected to contain a new aggregation with the data from
    /// the current collection cycle.
    fn call(&self, dest: Option<&mut AggregatedMetrics>) -> (usize, Option<AggregatedMetrics>);
}

/// Separate `measure` and `collect` functions for an aggregate.
pub(crate) struct AggregateFns<T> {
    pub(crate) measure: Arc<dyn Measure<T>>,
    pub(crate) collect: Arc<dyn ComputeAggregation>,
}

/// Creates aggregate functions out of aggregate instance
impl<A, T> From<A> for AggregateFns<T>
where
    A: Measure<T> + ComputeAggregation,
{
    fn from(value: A) -> Self {
        let inst = Arc::new(value);
        Self {
            measure: inst.clone(),
            collect: inst,
        }
    }
}

pub(crate) struct AggregateTime {
    pub start: SystemTime,
    pub current: SystemTime,
}

/// Initialized [`AggregateTime`] for specific [`Temporality`]
pub(crate) struct AggregateTimeInitiator(Mutex<SystemTime>);

impl AggregateTimeInitiator {
    pub(crate) fn delta(&self) -> AggregateTime {
        let current_time = now();
        let start_time = self
            .0
            .lock()
            .map(|mut start| replace(start.deref_mut(), current_time))
            .unwrap_or(current_time);
        AggregateTime {
            start: start_time,
            current: current_time,
        }
    }

    pub(crate) fn cumulative(&self) -> AggregateTime {
        let current_time = now();
        let start_time = self.0.lock().map(|start| *start).unwrap_or(current_time);
        AggregateTime {
            start: start_time,
            current: current_time,
        }
    }
}

impl Default for AggregateTimeInitiator {
    fn default() -> Self {
        Self(Mutex::new(now()))
    }
}

type Filter = Arc<dyn Fn(&KeyValue) -> bool + Send + Sync>;

/// Applies filter on provided attribute set
/// No-op, if filter is not set
#[derive(Clone)]
pub(crate) struct AttributeSetFilter {
    filter: Option<Filter>,
}

impl AttributeSetFilter {
    pub(crate) fn new(filter: Option<Filter>) -> Self {
        Self { filter }
    }

    pub(crate) fn apply(&self, attrs: &[KeyValue], run: impl FnOnce(&[KeyValue])) {
        if let Some(filter) = &self.filter {
            let filtered_attrs: Vec<KeyValue> =
                attrs.iter().filter(|kv| filter(kv)).cloned().collect();
            run(&filtered_attrs);
        } else {
            run(attrs);
        };
    }
}

/// Builds aggregate functions
pub(crate) struct AggregateBuilder<T> {
    /// The temporality used for the returned aggregate functions.
    temporality: Temporality,

    /// The attribute filter the aggregate function will use on the input of
    /// measurements.
    filter: AttributeSetFilter,

    /// Cardinality limit for the metric stream
    cardinality_limit: usize,

    _marker: marker::PhantomData<T>,
}

impl<T: Number> AggregateBuilder<T> {
    pub(crate) fn new(
        temporality: Temporality,
        filter: Option<Filter>,
        cardinality_limit: usize,
    ) -> Self {
        AggregateBuilder {
            temporality,
            filter: AttributeSetFilter::new(filter),
            cardinality_limit,
            _marker: marker::PhantomData,
        }
    }

    /// Builds a last-value aggregate function input and output.
    pub(crate) fn last_value(&self, overwrite_temporality: Option<Temporality>) -> AggregateFns<T> {
        LastValue::new(
            overwrite_temporality.unwrap_or(self.temporality),
            self.filter.clone(),
            self.cardinality_limit,
        )
        .into()
    }

    /// Builds a precomputed sum aggregate function input and output.
    pub(crate) fn precomputed_sum(&self, monotonic: bool) -> AggregateFns<T> {
        PrecomputedSum::new(
            self.temporality,
            self.filter.clone(),
            monotonic,
            self.cardinality_limit,
        )
        .into()
    }

    /// Builds a sum aggregate function input and output.
    pub(crate) fn sum(&self, monotonic: bool) -> AggregateFns<T> {
        Sum::new(
            self.temporality,
            self.filter.clone(),
            monotonic,
            self.cardinality_limit,
        )
        .into()
    }

    /// Builds a histogram aggregate function input and output.
    pub(crate) fn explicit_bucket_histogram(
        &self,
        boundaries: Vec<f64>,
        record_min_max: bool,
        record_sum: bool,
    ) -> AggregateFns<T> {
        Histogram::new(
            self.temporality,
            self.filter.clone(),
            boundaries,
            record_min_max,
            record_sum,
            self.cardinality_limit,
        )
        .into()
    }

    /// Builds an exponential histogram aggregate function input and output.
    pub(crate) fn exponential_bucket_histogram(
        &self,
        max_size: u32,
        max_scale: i8,
        record_min_max: bool,
        record_sum: bool,
    ) -> AggregateFns<T> {
        ExpoHistogram::new(
            self.temporality,
            self.filter.clone(),
            max_size,
            max_scale,
            record_min_max,
            record_sum,
            self.cardinality_limit,
        )
        .into()
    }
}

#[cfg(test)]
mod tests {
    use crate::metrics::data::{
        ExponentialBucket, ExponentialHistogram, ExponentialHistogramDataPoint, Gauge,
        GaugeDataPoint, Histogram, HistogramDataPoint, MetricData, Sum, SumDataPoint,
    };
    use std::vec;

    use super::*;

    const CARDINALITY_LIMIT_DEFAULT: usize = 2000;

    #[test]
    fn last_value_aggregation() {
        let AggregateFns { measure, collect } =
            AggregateBuilder::<u64>::new(Temporality::Cumulative, None, CARDINALITY_LIMIT_DEFAULT)
                .last_value(None);
        let mut a = MetricData::Gauge(Gauge {
            data_points: vec![GaugeDataPoint {
                attributes: vec![KeyValue::new("a", 1)],
                value: 1u64,
                exemplars: vec![],
            }],
            start_time: Some(now()),
            time: now(),
        })
        .into();
        let new_attributes = [KeyValue::new("b", 2)];
        measure.call(2, &new_attributes[..]);

        let (count, new_agg) = collect.call(Some(&mut a));
        let AggregatedMetrics::U64(MetricData::Gauge(a)) = a else {
            unreachable!()
        };

        assert_eq!(count, 1);
        assert!(new_agg.is_none());
        assert_eq!(a.data_points.len(), 1);
        assert_eq!(a.data_points[0].attributes, new_attributes.to_vec());
        assert_eq!(a.data_points[0].value, 2);
    }

    #[test]
    fn precomputed_sum_aggregation() {
        for temporality in [Temporality::Delta, Temporality::Cumulative] {
            let AggregateFns { measure, collect } =
                AggregateBuilder::<u64>::new(temporality, None, CARDINALITY_LIMIT_DEFAULT)
                    .precomputed_sum(true);
            let mut a = MetricData::Sum(Sum {
                data_points: vec![
                    SumDataPoint {
                        attributes: vec![KeyValue::new("a1", 1)],
                        value: 1u64,
                        exemplars: vec![],
                    },
                    SumDataPoint {
                        attributes: vec![KeyValue::new("a2", 1)],
                        value: 2u64,
                        exemplars: vec![],
                    },
                ],
                start_time: now(),
                time: now(),
                temporality: if temporality == Temporality::Delta {
                    Temporality::Cumulative
                } else {
                    Temporality::Delta
                },
                is_monotonic: false,
            })
            .into();
            let new_attributes = [KeyValue::new("b", 2)];
            measure.call(3, &new_attributes[..]);

            let (count, new_agg) = collect.call(Some(&mut a));
            let AggregatedMetrics::U64(MetricData::Sum(a)) = a else {
                unreachable!()
            };

            assert_eq!(count, 1);
            assert!(new_agg.is_none());
            assert_eq!(a.temporality, temporality);
            assert!(a.is_monotonic);
            assert_eq!(a.data_points.len(), 1);
            assert_eq!(a.data_points[0].attributes, new_attributes.to_vec());
            assert_eq!(a.data_points[0].value, 3);
        }
    }

    #[test]
    fn sum_aggregation() {
        for temporality in [Temporality::Delta, Temporality::Cumulative] {
            let AggregateFns { measure, collect } =
                AggregateBuilder::<u64>::new(temporality, None, CARDINALITY_LIMIT_DEFAULT)
                    .sum(true);
            let mut a = MetricData::Sum(Sum {
                data_points: vec![
                    SumDataPoint {
                        attributes: vec![KeyValue::new("a1", 1)],
                        value: 1u64,
                        exemplars: vec![],
                    },
                    SumDataPoint {
                        attributes: vec![KeyValue::new("a2", 1)],
                        value: 2u64,
                        exemplars: vec![],
                    },
                ],
                start_time: now(),
                time: now(),
                temporality: if temporality == Temporality::Delta {
                    Temporality::Cumulative
                } else {
                    Temporality::Delta
                },
                is_monotonic: false,
            })
            .into();
            let new_attributes = [KeyValue::new("b", 2)];
            measure.call(3, &new_attributes[..]);

            let (count, new_agg) = collect.call(Some(&mut a));
            let AggregatedMetrics::U64(MetricData::Sum(a)) = a else {
                unreachable!()
            };

            assert_eq!(count, 1);
            assert!(new_agg.is_none());
            assert_eq!(a.temporality, temporality);
            assert!(a.is_monotonic);
            assert_eq!(a.data_points.len(), 1);
            assert_eq!(a.data_points[0].attributes, new_attributes.to_vec());
            assert_eq!(a.data_points[0].value, 3);
        }
    }

    #[test]
    fn explicit_bucket_histogram_aggregation() {
        for temporality in [Temporality::Delta, Temporality::Cumulative] {
            let AggregateFns { measure, collect } =
                AggregateBuilder::<u64>::new(temporality, None, CARDINALITY_LIMIT_DEFAULT)
                    .explicit_bucket_histogram(vec![1.0], true, true);
            let mut a = MetricData::Histogram(Histogram {
                data_points: vec![HistogramDataPoint {
                    attributes: vec![KeyValue::new("a1", 1)],
                    count: 2,
                    bounds: vec![1.0, 2.0],
                    bucket_counts: vec![0, 1, 1],
                    min: None,
                    max: None,
                    sum: 3u64,
                    exemplars: vec![],
                }],
                start_time: now(),
                time: now(),
                temporality: if temporality == Temporality::Delta {
                    Temporality::Cumulative
                } else {
                    Temporality::Delta
                },
            })
            .into();
            let new_attributes = [KeyValue::new("b", 2)];
            measure.call(3, &new_attributes[..]);

            let (count, new_agg) = collect.call(Some(&mut a));
            let AggregatedMetrics::U64(MetricData::Histogram(a)) = a else {
                unreachable!()
            };

            assert_eq!(count, 1);
            assert!(new_agg.is_none());
            assert_eq!(a.temporality, temporality);
            assert_eq!(a.data_points.len(), 1);
            assert_eq!(a.data_points[0].attributes, new_attributes.to_vec());
            assert_eq!(a.data_points[0].count, 1);
            assert_eq!(a.data_points[0].bounds, vec![1.0]);
            assert_eq!(a.data_points[0].bucket_counts, vec![0, 1]);
            assert_eq!(a.data_points[0].min, Some(3));
            assert_eq!(a.data_points[0].max, Some(3));
            assert_eq!(a.data_points[0].sum, 3);
        }
    }

    #[test]
    fn exponential_histogram_aggregation() {
        for temporality in [Temporality::Delta, Temporality::Cumulative] {
            let AggregateFns { measure, collect } =
                AggregateBuilder::<u64>::new(temporality, None, CARDINALITY_LIMIT_DEFAULT)
                    .exponential_bucket_histogram(4, 20, true, true);
            let mut a = MetricData::ExponentialHistogram(ExponentialHistogram {
                data_points: vec![ExponentialHistogramDataPoint {
                    attributes: vec![KeyValue::new("a1", 1)],
                    count: 2,
                    min: None,
                    max: None,
                    sum: 3u64,
                    scale: 10,
                    zero_count: 1,
                    positive_bucket: ExponentialBucket {
                        offset: 1,
                        counts: vec![1],
                    },
                    negative_bucket: ExponentialBucket {
                        offset: 1,
                        counts: vec![1],
                    },
                    zero_threshold: 1.0,
                    exemplars: vec![],
                }],
                start_time: now(),
                time: now(),
                temporality: if temporality == Temporality::Delta {
                    Temporality::Cumulative
                } else {
                    Temporality::Delta
                },
            })
            .into();
            let new_attributes = [KeyValue::new("b", 2)];
            measure.call(3, &new_attributes[..]);

            let (count, new_agg) = collect.call(Some(&mut a));
            let AggregatedMetrics::U64(MetricData::ExponentialHistogram(a)) = a else {
                unreachable!()
            };

            assert_eq!(count, 1);
            assert!(new_agg.is_none());
            assert_eq!(a.temporality, temporality);
            assert_eq!(a.data_points.len(), 1);
            assert_eq!(a.data_points[0].attributes, new_attributes.to_vec());
            assert_eq!(a.data_points[0].count, 1);
            assert_eq!(a.data_points[0].min, Some(3));
            assert_eq!(a.data_points[0].max, Some(3));
            assert_eq!(a.data_points[0].sum, 3);
            assert_eq!(a.data_points[0].zero_count, 0);
            assert_eq!(a.data_points[0].zero_threshold, 0.0);
            assert_eq!(a.data_points[0].positive_bucket.offset, 1661953);
            assert_eq!(a.data_points[0].positive_bucket.counts, vec![1]);
            assert_eq!(a.data_points[0].negative_bucket.offset, 0);
            assert!(a.data_points[0].negative_bucket.counts.is_empty());
        }
    }
}
