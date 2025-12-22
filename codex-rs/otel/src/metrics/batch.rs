use crate::metrics::error::MetricsError;
use crate::metrics::error::Result;
use crate::metrics::tags::collect_tags;
use crate::metrics::validation::validate_metric_name;
use crate::metrics::validation::validate_tag_key;
use crate::metrics::validation::validate_tag_value;

#[cfg_attr(test, derive(PartialEq, Eq))]
#[derive(Clone, Debug)]
pub struct HistogramBuckets {
    bounds: Vec<i64>,
}

impl HistogramBuckets {
    /// Build histogram buckets from unsorted bounds (upper limits).
    pub fn new(mut bounds: Vec<i64>) -> Result<Self> {
        if bounds.is_empty() {
            return Err(MetricsError::EmptyBuckets);
        }
        bounds.sort_unstable();
        bounds.dedup();
        Ok(Self { bounds })
    }

    /// Build histogram buckets from a slice of upper bounds.
    pub fn from_values(bounds: &[i64]) -> Result<Self> {
        Self::new(bounds.to_vec())
    }

    /// Build linear histogram buckets from an inclusive range and step size.
    pub fn from_range(from: i64, to: i64, n_step: i64) -> Result<Self> {
        if n_step <= 0 {
            return Err(MetricsError::BucketStepNonPositive { step: n_step });
        }
        if from > to {
            return Err(MetricsError::BucketRangeDescending { from, to });
        }

        let mut bounds = Vec::new();
        let mut current = from;
        bounds.push(current);

        while current < to {
            let next = match current.checked_add(n_step) {
                Some(next) => next,
                None => {
                    return Err(MetricsError::BucketRangeOverflow {
                        from,
                        to,
                        step: n_step,
                    });
                }
            };
            if next >= to {
                bounds.push(to);
                break;
            }
            bounds.push(next);
            current = next;
        }

        Self::new(bounds)
    }

    /// Build exponential histogram buckets from an inclusive range and factor.
    pub fn from_exponential(from: i64, to: i64, factor: f64) -> Result<Self> {
        if from <= 0 {
            return Err(MetricsError::BucketStartNonPositive { start: from });
        }
        if from > to {
            return Err(MetricsError::BucketRangeDescending { from, to });
        }
        if !factor.is_finite() || factor <= 1.0 {
            return Err(MetricsError::BucketFactorInvalid { factor });
        }

        let mut bounds = Vec::new();
        let mut current = from;
        bounds.push(current);

        while current < to {
            let next_value = (current as f64) * factor;
            if !next_value.is_finite() || next_value >= to as f64 {
                bounds.push(to);
                break;
            }
            let mut next = next_value.ceil() as i64;
            if next <= current {
                next = current + 1;
            }
            if next >= to {
                bounds.push(to);
                break;
            }
            bounds.push(next);
            current = next;
        }

        Self::new(bounds)
    }

    pub(crate) fn bounds(&self) -> &[i64] {
        &self.bounds
    }
}

#[derive(Clone, Debug)]
pub(crate) enum MetricEvent {
    Counter {
        name: String,
        value: i64,
        tags: Vec<(String, String)>,
    },
    Histogram {
        name: String,
        value: i64,
        tags: Vec<(String, String)>,
    },
}

pub struct MetricsBatch {
    events: Vec<MetricEvent>,
    default_tags: Vec<(String, String)>,
}

impl Default for MetricsBatch {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsBatch {
    /// Create an empty metrics batch.
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            default_tags: Vec::new(),
        }
    }

    pub fn with_default_tags(default_tags: Vec<(String, String)>) -> Result<Self> {
        for (key, value) in &default_tags {
            validate_tag_key(key)?;
            validate_tag_value(value)?;
        }
        Ok(Self {
            events: Vec::new(),
            default_tags,
        })
    }

    /// Append a counter increment to the batch.
    pub fn counter(&mut self, name: &str, inc: i64, tags: &[(&str, &str)]) -> Result<()> {
        validate_metric_name(name)?;
        let mut merged_tags = self.default_tags.clone();
        merged_tags.extend(collect_tags(tags)?);
        self.events.push(MetricEvent::Counter {
            name: name.to_string(),
            value: inc,
            tags: merged_tags,
        });
        Ok(())
    }

    /// Append a histogram sample to the batch.
    pub fn histogram(
        &mut self,
        name: &str,
        value: i64,
        buckets: &HistogramBuckets,
        tags: &[(&str, &str)],
    ) -> Result<()> {
        // Buckets remain part of the API, but OTEL histogram aggregation owns bucket selection.
        let _ = buckets.bounds();
        validate_metric_name(name)?;
        let mut merged_tags = self.default_tags.clone();
        merged_tags.extend(collect_tags(tags)?);
        self.events.push(MetricEvent::Histogram {
            name: name.to_string(),
            value,
            tags: merged_tags,
        });
        Ok(())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub(crate) fn into_events(self) -> Vec<MetricEvent> {
        self.events
    }
}

#[cfg(test)]
mod tests {
    use super::HistogramBuckets;
    use crate::metrics::error::MetricsError;
    use crate::metrics::error::Result;
    use pretty_assertions::assert_eq;

    #[test]
    // Verifies linear bucket construction over a clean step range.
    fn from_range_builds_linear_buckets() -> Result<()> {
        let buckets = HistogramBuckets::from_range(25, 100, 25)?;
        let expected = HistogramBuckets::from_values(&[25, 50, 75, 100])?;
        assert_eq!(buckets, expected);
        Ok(())
    }

    #[test]
    // Ensures uneven steps still include the final upper bound.
    fn from_range_includes_upper_bound_when_step_is_uneven() -> Result<()> {
        let buckets = HistogramBuckets::from_range(10, 95, 30)?;
        let expected = HistogramBuckets::from_values(&[10, 40, 70, 95])?;
        assert_eq!(buckets, expected);
        Ok(())
    }

    #[test]
    // Confirms a single-value range produces one bucket.
    fn from_range_accepts_single_value_range() -> Result<()> {
        let buckets = HistogramBuckets::from_range(42, 42, 5)?;
        let expected = HistogramBuckets::from_values(&[42])?;
        assert_eq!(buckets, expected);
        Ok(())
    }

    #[test]
    // Rejects a non-positive step to avoid invalid ranges.
    fn from_range_rejects_non_positive_step() {
        let err = HistogramBuckets::from_range(0, 10, 0).unwrap_err();
        assert_eq!(err.to_string(), "histogram bucket step must be positive: 0");
    }

    #[test]
    // Rejects descending ranges to prevent inverted buckets.
    fn from_range_rejects_descending_range() {
        let err = HistogramBuckets::from_range(10, 0, 1).unwrap_err();
        assert_eq!(
            err.to_string(),
            "histogram bucket range must be ascending: 10..=0"
        );
    }

    #[test]
    // Verifies exponential buckets grow and include the upper bound.
    fn from_exponential_builds_buckets() -> Result<()> {
        let buckets = HistogramBuckets::from_exponential(10, 100, 2.0)?;
        let expected = HistogramBuckets::from_values(&[10, 20, 40, 80, 100])?;
        assert_eq!(buckets, expected);
        Ok(())
    }

    #[test]
    // Ensures exponential buckets always include the final bound.
    fn from_exponential_includes_upper_bound() -> Result<()> {
        let buckets = HistogramBuckets::from_exponential(30, 100, 3.0)?;
        let expected = HistogramBuckets::from_values(&[30, 90, 100])?;
        assert_eq!(buckets, expected);
        Ok(())
    }

    #[test]
    // Rejects non-positive starts because exponential growth requires > 0.
    fn from_exponential_rejects_non_positive_start() {
        let err = HistogramBuckets::from_exponential(0, 10, 2.0).unwrap_err();
        assert!(matches!(
            err,
            MetricsError::BucketStartNonPositive { start: 0 }
        ));
    }

    #[test]
    // Rejects invalid exponential factors (non-finite or <= 1).
    fn from_exponential_rejects_invalid_factor() {
        let err = HistogramBuckets::from_exponential(1, 10, 1.0).unwrap_err();
        assert!(matches!(
            err,
            MetricsError::BucketFactorInvalid { factor: 1.0 }
        ));
    }
}
