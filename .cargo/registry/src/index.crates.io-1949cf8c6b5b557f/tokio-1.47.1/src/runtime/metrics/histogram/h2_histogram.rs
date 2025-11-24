use crate::runtime::metrics::batch::duration_as_u64;
use std::cmp;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::time::Duration;

const DEFAULT_MIN_VALUE: Duration = Duration::from_nanos(100);
const DEFAULT_MAX_VALUE: Duration = Duration::from_secs(60);

/// Default precision is 2^-2 = 25% max error
const DEFAULT_PRECISION: u32 = 2;
const MAX_PRECISION: u32 = 10;

/// Log Histogram
///
/// This implements an [H2 Histogram](https://iop.systems/blog/h2-histogram/), a histogram similar
/// to HdrHistogram, but with better performance. It guarantees an error bound of `2^-p`.
///
/// Unlike a traditional H2 histogram this has two small changes:
/// 1. The 0th bucket runs for `0..min_value`. This allows truncating a large number of buckets that
///    would cover extremely short timescales that customers usually don't care about.
/// 2. The final bucket runs all the way to `u64::MAX` — traditional H2 histograms would truncate
///    or reject these values.
///
/// For information about the default configuration, see [`LogHistogramBuilder`].
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct LogHistogram {
    /// Number of buckets in the histogram
    pub(crate) num_buckets: usize,

    /// Precision of histogram. Error is bounded to 2^-p.
    pub(crate) p: u32,

    /// All buckets `idx < bucket_offset` are grouped into bucket 0.
    ///
    /// This increases the smallest measurable value of the histogram.
    pub(crate) bucket_offset: usize,
}

impl Default for LogHistogram {
    fn default() -> Self {
        LogHistogramBuilder::default().build()
    }
}

impl LogHistogram {
    /// Create a Histogram configuration directly from values for `n` and `p`.
    ///
    /// # Panics
    /// - If `bucket_offset` is greater than the specified number of buckets, `(n - p + 1) * 2^p`
    fn from_n_p(n: u32, p: u32, bucket_offset: usize) -> Self {
        assert!(n >= p, "{n} (n) must be at least as large as {p} (p)");
        let num_buckets = ((n - p + 1) * 1 << p) as usize - bucket_offset;
        Self {
            num_buckets,
            p,
            bucket_offset,
        }
    }

    fn truncate_to_max_value(&self, max_value: u64) -> LogHistogram {
        let mut hist = self.clone();
        while hist.max_value() >= max_value {
            hist.num_buckets -= 1;
        }
        hist.num_buckets += 1;
        hist
    }

    /// Creates a builder for [`LogHistogram`]
    pub fn builder() -> LogHistogramBuilder {
        LogHistogramBuilder::default()
    }

    /// The maximum value that can be stored before truncation in this histogram
    pub fn max_value(&self) -> u64 {
        self.bucket_range(self.num_buckets - 2).end
    }

    pub(crate) fn value_to_bucket(&self, value: u64) -> usize {
        let index = bucket_index(value, self.p);
        let offset_bucket = if index < self.bucket_offset as u64 {
            0
        } else {
            index - self.bucket_offset as u64
        };
        let max = self.num_buckets - 1;
        offset_bucket.min(max as u64) as usize
    }

    pub(crate) fn bucket_range(&self, bucket: usize) -> std::ops::Range<u64> {
        let LogHistogram {
            p,
            bucket_offset,
            num_buckets,
        } = self;
        let input_bucket = bucket;
        let bucket = bucket + bucket_offset;
        let range_start_0th_bucket = match input_bucket {
            0 => Some(0_u64),
            _ => None,
        };
        let range_end_last_bucket = match input_bucket {
            n if n == num_buckets - 1 => Some(u64::MAX),
            _ => None,
        };
        if bucket < 1 << p {
            // The first set of buckets are all size 1
            let bucket = bucket as u64;
            range_start_0th_bucket.unwrap_or(bucket)..range_end_last_bucket.unwrap_or(bucket + 1)
        } else {
            // Determine which range of buckets we're in, then determine which bucket in the range it is
            let bucket = bucket as u64;
            let p = *p as u64;
            let w = (bucket >> p) - 1;
            let base_bucket = (w + 1) * (1_u64 << p);
            let offset = bucket - base_bucket;
            let s = 1_u64 << (w + p);
            let start = s + (offset << w);
            let end = s + ((offset + 1) << w);

            range_start_0th_bucket.unwrap_or(start)..range_end_last_bucket.unwrap_or(end)
        }
    }
}

/// Configuration for a [`LogHistogram`]
///
/// The log-scaled histogram implements an H2 histogram where the first bucket covers
/// the range from 0 to [`LogHistogramBuilder::min_value`] and the final bucket covers
/// [`LogHistogramBuilder::max_value`] to infinity. The precision is bounded to the specified
/// [`LogHistogramBuilder::max_error`]. Specifically, the precision is the next smallest value
/// of `2^-p` such that it is smaller than the requested max error. You can also select `p` directly
/// with [`LogHistogramBuilder::precision_exact`].
///
/// Depending on the selected parameters, the number of buckets required is variable. To ensure
/// that the histogram size is acceptable, callers may call [`LogHistogramBuilder::max_buckets`].
/// If the resulting histogram would require more buckets, then the method will return an error.
///
/// ## Default values
/// The default configuration provides the following settings:
/// 1. `min_value`: 100ns
/// 2. `max_value`: 68 seconds. The final bucket covers all values >68 seconds
/// 3. `precision`: max error of 25%
///
/// This uses 237 64-bit buckets.
#[derive(Default, Debug, Copy, Clone)]
pub struct LogHistogramBuilder {
    max_value: Option<Duration>,
    min_value: Option<Duration>,
    precision: Option<u32>,
}

impl From<LogHistogramBuilder> for LogHistogram {
    fn from(value: LogHistogramBuilder) -> Self {
        value.build()
    }
}

impl LogHistogramBuilder {
    /// Set the precision for this histogram
    ///
    /// This function determines the smallest value of `p` that would satisfy the requested precision
    /// such that `2^-p` is less than `precision`. To set `p` directly, use
    /// [`LogHistogramBuilder::precision_exact`].
    ///
    /// Precision controls the size of the "bucket groups" (consecutive buckets with identical
    /// ranges). When `p` is 0, each bucket will be twice the size of the previous bucket. To match
    /// the behavior of the legacy log histogram implementation, use `builder.precision_exact(0)`.
    ///
    /// The default value is 25% (2^-2)
    ///
    /// The highest supported precision is `0.0977%` `(2^-10)`. Provided values
    /// less than this will be truncated.
    ///
    /// # Panics
    /// - `max_error` <= 0
    /// - `max_error` >= 1
    pub fn max_error(mut self, max_error: f64) -> Self {
        assert!(max_error > 0.0, "max_error must be greater than 0");
        assert!(max_error < 1.0, "max_error must be less than 1");
        let mut p = 2;
        while 2_f64.powf(-1.0 * p as f64) > max_error && p <= MAX_PRECISION {
            p += 1;
        }
        self.precision = Some(p);
        self
    }

    /// Sets the precision of this histogram directly.
    ///
    /// The precision (meaning: the ratio `n/bucket_range(n)` for some given `n`) will be `2^-p`.
    ///
    /// Precision controls the number consecutive buckets with identically sized ranges.
    /// When `p` is 0, each bucket will be twice the size of the previous bucket (bucket groups are
    /// only a single bucket wide).
    ///
    /// To match the behavior of the legacy implementation ([`HistogramScale::Log`]), use `builder.precision_exact(0)`.
    ///
    /// # Panics
    /// - `p` > 10
    ///
    /// [`HistogramScale::Log`]: [crate::runtime::HistogramScale]
    pub fn precision_exact(mut self, p: u32) -> Self {
        assert!(p <= MAX_PRECISION, "precision must be <= {MAX_PRECISION}");
        self.precision = Some(p);
        self
    }

    /// Sets the minimum duration that can be accurately stored by this histogram.
    ///
    /// This sets the resolution. The first bucket will be no larger than
    /// the provided duration. Setting this value will reduce the number of required buckets,
    /// sometimes quite significantly.
    pub fn min_value(mut self, duration: Duration) -> Self {
        self.min_value = Some(duration);
        self
    }

    /// Sets the maximum value that can by this histogram without truncation
    ///
    /// Values greater than this fall in the final bucket that stretches to `u64::MAX`.
    ///
    /// # Panics
    /// The provided value is 0
    pub fn max_value(mut self, duration: Duration) -> Self {
        if duration.is_zero() {
            panic!("max value must be greater than 0");
        }
        self.max_value = Some(duration);
        self
    }

    /// Builds the log histogram, enforcing the max buckets requirement
    pub fn max_buckets(
        &mut self,
        max_buckets: usize,
    ) -> Result<LogHistogram, InvalidHistogramConfiguration> {
        let histogram = self.build();
        if histogram.num_buckets > max_buckets {
            return Err(InvalidHistogramConfiguration::TooManyBuckets {
                required_bucket_count: histogram.num_buckets,
            });
        }
        Ok(histogram)
    }

    /// Builds the log histogram
    pub fn build(&self) -> LogHistogram {
        let requested_max_value = duration_as_u64(self.max_value.unwrap_or(DEFAULT_MAX_VALUE));
        let max_value = requested_max_value.next_power_of_two();
        let min_value = duration_as_u64(self.min_value.unwrap_or(DEFAULT_MIN_VALUE));
        let p = self.precision.unwrap_or(DEFAULT_PRECISION);
        // determine the bucket offset by finding the bucket for the minimum value. We need to lower
        // this by one to ensure we are at least as granular as requested.
        let bucket_offset = cmp::max(bucket_index(min_value, p), 1) - 1;
        // n must be at least as large as p
        let n = max_value.ilog2().max(p) + 1;
        LogHistogram::from_n_p(n, p, bucket_offset as usize)
            .truncate_to_max_value(requested_max_value)
    }
}

/// Error constructing a histogram
#[derive(Debug)]
pub enum InvalidHistogramConfiguration {
    /// This histogram required more than the specified number of buckets
    TooManyBuckets {
        /// The number of buckets that would have been required
        required_bucket_count: usize,
    },
}

impl Display for InvalidHistogramConfiguration {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            InvalidHistogramConfiguration::TooManyBuckets { required_bucket_count } =>
                write!(f, "The configuration for this histogram would have required {required_bucket_count} buckets")
        }
    }
}

impl Error for InvalidHistogramConfiguration {}

/// Compute the index for a given value + p combination
///
/// This function does NOT enforce that the value is within the number of expected buckets.
fn bucket_index(value: u64, p: u32) -> u64 {
    // Algorithm described here: https://iop.systems/blog/h2-histogram/
    // find the highest non-zero digit
    if value == 0 {
        return 0;
    }
    let h = 63 - value.leading_zeros();
    if h <= p {
        value
    } else {
        let w = h - p;
        ((w + 1) * (1_u32 << p)) as u64 + ((value - (1_u64 << h)) >> w)
    }
}

#[cfg(test)]
mod test {
    use super::InvalidHistogramConfiguration;
    use crate::runtime::metrics::histogram::h2_histogram::LogHistogram;
    use crate::runtime::metrics::histogram::HistogramType;

    #[cfg(not(target_family = "wasm"))]
    mod proptests {
        use super::*;
        use crate::runtime::metrics::batch::duration_as_u64;
        use crate::runtime::metrics::histogram::h2_histogram::MAX_PRECISION;
        use proptest::prelude::*;
        use std::time::Duration;

        fn valid_log_histogram_strategy() -> impl Strategy<Value = LogHistogram> {
            (1..=50u32, 0..=MAX_PRECISION, 0..100usize).prop_map(|(n, p, bucket_offset)| {
                let p = p.min(n);
                let base = LogHistogram::from_n_p(n, p, 0);
                LogHistogram::from_n_p(n, p, bucket_offset.min(base.num_buckets - 1))
            })
        }

        fn log_histogram_settings() -> impl Strategy<Value = (u64, u64, u32)> {
            (
                duration_as_u64(Duration::from_nanos(1))..duration_as_u64(Duration::from_secs(20)),
                duration_as_u64(Duration::from_secs(1))..duration_as_u64(Duration::from_secs(1000)),
                0..MAX_PRECISION,
            )
        }

        // test against a wide assortment of different histogram configurations to ensure invariants hold
        proptest! {
            #[test]
            fn log_histogram_settings_maintain_invariants((min_value, max_value, p) in log_histogram_settings()) {
                if max_value < min_value {
                    return Ok(())
                }
                let (min_value, max_value) = (Duration::from_nanos(min_value), Duration::from_nanos(max_value));
                let histogram = LogHistogram::builder().min_value(min_value).max_value(max_value).precision_exact(p).build();
                let first_bucket_end = Duration::from_nanos(histogram.bucket_range(0).end);
                let last_bucket_start = Duration::from_nanos(histogram.bucket_range(histogram.num_buckets - 1).start);
                let second_last_bucket_start = Duration::from_nanos(histogram.bucket_range(histogram.num_buckets - 2).start);
                prop_assert!(
                    first_bucket_end  <= min_value,
                    "first bucket {first_bucket_end:?} must be less than {min_value:?}"
                );
                prop_assert!(
                    last_bucket_start > max_value,
                    "last bucket start ({last_bucket_start:?} must be at least as big as `max_value` ({max_value:?})"
                );

                // We should have the exact right number of buckets. The second to last bucket should be strictly less than max value.
                prop_assert!(
                    second_last_bucket_start < max_value,
                    "second last bucket end ({second_last_bucket_start:?} must be at least as big as `max_value` ({max_value:?})"
                );
            }

            #[test]
            fn proptest_log_histogram_invariants(histogram in valid_log_histogram_strategy()) {
                // 1. Assert that the first bucket always starts at 0
                let first_range = histogram.bucket_range(0);
                prop_assert_eq!(first_range.start, 0, "First bucket doesn't start at 0");

                // Check that bucket ranges are disjoint and contiguous
                let mut prev_end = 0;
                let mut prev_size = 0;
                for bucket in 0..histogram.num_buckets {
                    let range = histogram.bucket_range(bucket);
                    prop_assert_eq!(range.start, prev_end, "Bucket ranges are not contiguous");
                    prop_assert!(range.start < range.end, "Bucket range is empty or reversed");

                    let size = range.end - range.start;

                    // 2. Assert that the sizes of the buckets are always powers of 2
                    if bucket > 0 && bucket < histogram.num_buckets - 1 {
                        prop_assert!(size.is_power_of_two(), "Bucket size is not a power of 2");
                    }

                    if bucket > 1 {
                        // Assert that the sizes of the buckets are monotonically increasing
                        // (after the first bucket, which may be smaller than the 0 bucket)
                        prop_assert!(size >= prev_size, "Bucket sizes are not monotonically increasing: This size {size} (previous: {prev_size}). Bucket: {bucket}");
                    }


                    // 4. Assert that the size of the buckets is always within the error bound of 2^-p
                    if bucket > 0 && bucket < histogram.num_buckets - 1 {
                        let p = histogram.p as f64;
                        let error_bound = 2.0_f64.powf(-p);
                        // the most it could be wrong is by the length of the range / 2
                        let relative_error = ((size as f64 - 1.0) / 2.0) / range.start as f64;
                        prop_assert!(
                            relative_error <= error_bound,
                            "Bucket size error exceeds bound: {:?} > {:?} ({range:?})",
                            relative_error,
                            error_bound
                        );
                    }

                    prev_end = range.end;
                    prev_size = size;
                }
                prop_assert_eq!(prev_end, u64::MAX, "Last bucket should end at u64::MAX");

                // Check bijection between value_to_bucket and bucket_range
                for bucket in 0..histogram.num_buckets {
                    let range = histogram.bucket_range(bucket);
                    for value in [range.start, range.end - 1] {
                        prop_assert_eq!(
                            histogram.value_to_bucket(value),
                            bucket,
                            "value_to_bucket is not consistent with bucket_range"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn bucket_ranges_are_correct() {
        let p = 2;
        let config = HistogramType::H2(LogHistogram {
            num_buckets: 1024,
            p,
            bucket_offset: 0,
        });

        // check precise buckets. There are 2^(p+1) precise buckets
        for i in 0..2_usize.pow(p + 1) {
            assert_eq!(
                config.value_to_bucket(i as u64),
                i,
                "{} should be in bucket {}",
                i,
                i
            );
        }

        let mut value = 2_usize.pow(p + 1);
        let current_bucket = value;
        while value < current_bucket * 2 {
            assert_eq!(
                config.value_to_bucket(value as u64),
                current_bucket + ((value - current_bucket) / 2),
                "bucket for {value}"
            );
            value += 1;
        }
    }

    // test buckets against known values
    #[test]
    fn bucket_computation_spot_check() {
        let p = 9;
        let config = HistogramType::H2(LogHistogram {
            num_buckets: 4096,
            p,
            bucket_offset: 0,
        });
        struct T {
            v: u64,
            bucket: usize,
        }
        let tests = [
            T { v: 1, bucket: 1 },
            T {
                v: 1023,
                bucket: 1023,
            },
            T {
                v: 1024,
                bucket: 1024,
            },
            T {
                v: 2048,
                bucket: 1536,
            },
            T {
                v: 2052,
                bucket: 1537,
            },
        ];
        for test in tests {
            assert_eq!(config.value_to_bucket(test.v), test.bucket);
        }
    }

    #[test]
    fn last_bucket_goes_to_infinity() {
        let conf = HistogramType::H2(LogHistogram::from_n_p(16, 3, 10));
        assert_eq!(conf.bucket_range(conf.num_buckets() - 1).end, u64::MAX);
    }

    #[test]
    fn bucket_offset() {
        // skip the first 10 buckets
        let conf = HistogramType::H2(LogHistogram::from_n_p(16, 3, 10));
        for i in 0..10 {
            assert_eq!(conf.value_to_bucket(i), 0);
        }
        // There are 16 1-element buckets. We skipped 10 of them. The first 2 element bucket starts
        // at 16
        assert_eq!(conf.value_to_bucket(10), 0);
        assert_eq!(conf.value_to_bucket(16), 6);
        assert_eq!(conf.value_to_bucket(17), 6);
        assert_eq!(conf.bucket_range(6), 16..18);
    }

    #[test]
    fn max_buckets_enforcement() {
        let error = LogHistogram::builder()
            .max_error(0.001)
            .max_buckets(5)
            .expect_err("this produces way more than 5 buckets");
        let num_buckets = match error {
            InvalidHistogramConfiguration::TooManyBuckets {
                required_bucket_count,
            } => required_bucket_count,
        };
        assert_eq!(num_buckets, 27291);
    }

    #[test]
    fn default_configuration_size() {
        let conf = LogHistogram::builder().build();
        assert_eq!(conf.num_buckets, 119);
    }
}
