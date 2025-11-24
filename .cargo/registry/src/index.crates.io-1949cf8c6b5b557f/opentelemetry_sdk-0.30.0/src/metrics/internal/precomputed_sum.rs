use opentelemetry::KeyValue;

use crate::metrics::data::{self, AggregatedMetrics, MetricData, SumDataPoint};
use crate::metrics::Temporality;

use super::aggregate::{AggregateTimeInitiator, AttributeSetFilter};
use super::{last_value::Assign, AtomicTracker, Number, ValueMap};
use super::{ComputeAggregation, Measure};
use std::{collections::HashMap, sync::Mutex};

/// Summarizes a set of pre-computed sums as their arithmetic sum.
pub(crate) struct PrecomputedSum<T: Number> {
    value_map: ValueMap<Assign<T>>,
    init_time: AggregateTimeInitiator,
    temporality: Temporality,
    filter: AttributeSetFilter,
    monotonic: bool,
    reported: Mutex<HashMap<Vec<KeyValue>, T>>,
}

impl<T: Number> PrecomputedSum<T> {
    pub(crate) fn new(
        temporality: Temporality,
        filter: AttributeSetFilter,
        monotonic: bool,
        cardinality_limit: usize,
    ) -> Self {
        PrecomputedSum {
            value_map: ValueMap::new((), cardinality_limit),
            init_time: AggregateTimeInitiator::default(),
            temporality,
            filter,
            monotonic,
            reported: Mutex::new(Default::default()),
        }
    }

    pub(crate) fn delta(&self, dest: Option<&mut MetricData<T>>) -> (usize, Option<MetricData<T>>) {
        let time = self.init_time.delta();

        let s_data = dest.and_then(|d| {
            if let MetricData::Sum(sum) = d {
                Some(sum)
            } else {
                None
            }
        });
        let mut new_agg = if s_data.is_none() {
            Some(data::Sum {
                data_points: vec![],
                start_time: time.start,
                time: time.current,
                temporality: Temporality::Delta,
                is_monotonic: self.monotonic,
            })
        } else {
            None
        };
        let s_data = s_data.unwrap_or_else(|| new_agg.as_mut().expect("present if s_data is none"));
        s_data.start_time = time.start;
        s_data.time = time.current;
        s_data.temporality = Temporality::Delta;
        s_data.is_monotonic = self.monotonic;

        let mut reported = match self.reported.lock() {
            Ok(r) => r,
            Err(_) => return (0, None),
        };
        let mut new_reported = HashMap::with_capacity(reported.len());

        self.value_map
            .collect_and_reset(&mut s_data.data_points, |attributes, aggr| {
                let value = aggr.value.get_value();
                new_reported.insert(attributes.clone(), value);
                let delta = value - *reported.get(&attributes).unwrap_or(&T::default());
                SumDataPoint {
                    attributes,
                    value: delta,
                    exemplars: vec![],
                }
            });

        *reported = new_reported;
        drop(reported); // drop before values guard is dropped

        (s_data.data_points.len(), new_agg.map(Into::into))
    }

    pub(crate) fn cumulative(
        &self,
        dest: Option<&mut MetricData<T>>,
    ) -> (usize, Option<MetricData<T>>) {
        let time = self.init_time.cumulative();

        let s_data = dest.and_then(|d| {
            if let MetricData::Sum(sum) = d {
                Some(sum)
            } else {
                None
            }
        });
        let mut new_agg = if s_data.is_none() {
            Some(data::Sum {
                data_points: vec![],
                start_time: time.start,
                time: time.current,
                temporality: Temporality::Cumulative,
                is_monotonic: self.monotonic,
            })
        } else {
            None
        };
        let s_data = s_data.unwrap_or_else(|| new_agg.as_mut().expect("present if s_data is none"));
        s_data.start_time = time.start;
        s_data.time = time.current;
        s_data.temporality = Temporality::Cumulative;
        s_data.is_monotonic = self.monotonic;

        self.value_map
            .collect_readonly(&mut s_data.data_points, |attributes, aggr| SumDataPoint {
                attributes,
                value: aggr.value.get_value(),
                exemplars: vec![],
            });

        (s_data.data_points.len(), new_agg.map(Into::into))
    }
}

impl<T> Measure<T> for PrecomputedSum<T>
where
    T: Number,
{
    fn call(&self, measurement: T, attrs: &[KeyValue]) {
        self.filter.apply(attrs, |filtered| {
            self.value_map.measure(measurement, filtered);
        })
    }
}

impl<T> ComputeAggregation for PrecomputedSum<T>
where
    T: Number,
{
    fn call(&self, dest: Option<&mut AggregatedMetrics>) -> (usize, Option<AggregatedMetrics>) {
        let data = dest.and_then(|d| T::extract_metrics_data_mut(d));
        let (len, new) = match self.temporality {
            Temporality::Delta => self.delta(data),
            _ => self.cumulative(data),
        };
        (len, new.map(T::make_aggregated_metrics))
    }
}
