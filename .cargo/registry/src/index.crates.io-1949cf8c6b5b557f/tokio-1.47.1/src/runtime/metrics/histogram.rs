mod h2_histogram;

pub use h2_histogram::{InvalidHistogramConfiguration, LogHistogram, LogHistogramBuilder};

use crate::util::metric_atomics::MetricAtomicU64;
use std::sync::atomic::Ordering::Relaxed;

use crate::runtime::metrics::batch::duration_as_u64;
use std::cmp;
use std::ops::Range;
use std::time::Duration;

#[derive(Debug)]
pub(crate) struct Histogram {
    /// The histogram buckets
    buckets: Box<[MetricAtomicU64]>,

    /// The type of the histogram
    ///
    /// This handles `fn(bucket) -> Range` and `fn(value) -> bucket`
    histogram_type: HistogramType,
}

#[derive(Debug, Clone)]
pub(crate) struct HistogramBuilder {
    pub(crate) histogram_type: HistogramType,
    pub(crate) legacy: Option<LegacyBuilder>,
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyBuilder {
    pub(crate) resolution: u64,
    pub(crate) scale: HistogramScale,
    pub(crate) num_buckets: usize,
}

impl Default for LegacyBuilder {
    fn default() -> Self {
        Self {
            resolution: 100_000,
            num_buckets: 10,
            scale: HistogramScale::Linear,
        }
    }
}

#[derive(Debug)]
pub(crate) struct HistogramBatch {
    buckets: Box<[u64]>,
    configuration: HistogramType,
}

cfg_unstable! {
    /// Whether the histogram used to aggregate a metric uses a linear or
    /// logarithmic scale.
    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    #[non_exhaustive]
    pub enum HistogramScale {
        /// Linear bucket scale
        Linear,

        /// Logarithmic bucket scale
        Log,
    }

    /// Configuration for the poll count histogram
    #[derive(Debug, Clone)]
    pub struct HistogramConfiguration {
        pub(crate) inner: HistogramType
    }

    impl HistogramConfiguration {
        /// Create a linear bucketed histogram
        ///
        /// # Arguments
        ///
        /// * `bucket_width`: The width of each bucket
        /// * `num_buckets`: The number of buckets
        pub fn linear(bucket_width: Duration, num_buckets: usize) -> Self {
            Self {
                inner: HistogramType::Linear(LinearHistogram {
                    num_buckets,
                    bucket_width: duration_as_u64(bucket_width),
                }),
            }
        }

        /// Creates a log-scaled bucketed histogram
        ///
        /// See [`LogHistogramBuilder`] for information about configuration & defaults
        pub fn log(configuration: impl Into<LogHistogram>) -> Self {
            Self {
                inner: HistogramType::H2(configuration.into()),
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum HistogramType {
    /// Linear histogram with fixed width buckets
    Linear(LinearHistogram),

    /// Old log histogram where each bucket doubles in size
    LogLegacy(LegacyLogHistogram),

    /// Log histogram implementation based on H2 Histograms
    H2(LogHistogram),
}

impl HistogramType {
    pub(crate) fn num_buckets(&self) -> usize {
        match self {
            HistogramType::Linear(linear) => linear.num_buckets,
            HistogramType::LogLegacy(log) => log.num_buckets,
            HistogramType::H2(h2) => h2.num_buckets,
        }
    }
    fn value_to_bucket(&self, value: u64) -> usize {
        match self {
            HistogramType::Linear(LinearHistogram {
                num_buckets,
                bucket_width,
            }) => {
                let max = num_buckets - 1;
                cmp::min(value / *bucket_width, max as u64) as usize
            }
            HistogramType::LogLegacy(LegacyLogHistogram {
                num_buckets,
                first_bucket_width,
            }) => {
                let max = num_buckets - 1;
                if value < *first_bucket_width {
                    0
                } else {
                    let significant_digits = 64 - value.leading_zeros();
                    let bucket_digits = 64 - (first_bucket_width - 1).leading_zeros();
                    cmp::min(significant_digits as usize - bucket_digits as usize, max)
                }
            }
            HistogramType::H2(log_histogram) => log_histogram.value_to_bucket(value),
        }
    }

    fn bucket_range(&self, bucket: usize) -> Range<u64> {
        match self {
            HistogramType::Linear(LinearHistogram {
                num_buckets,
                bucket_width,
            }) => Range {
                start: bucket_width * bucket as u64,
                end: if bucket == num_buckets - 1 {
                    u64::MAX
                } else {
                    bucket_width * (bucket as u64 + 1)
                },
            },
            HistogramType::LogLegacy(LegacyLogHistogram {
                num_buckets,
                first_bucket_width,
            }) => Range {
                start: if bucket == 0 {
                    0
                } else {
                    first_bucket_width << (bucket - 1)
                },
                end: if bucket == num_buckets - 1 {
                    u64::MAX
                } else {
                    first_bucket_width << bucket
                },
            },
            HistogramType::H2(log) => log.bucket_range(bucket),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct LinearHistogram {
    num_buckets: usize,
    bucket_width: u64,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct LegacyLogHistogram {
    num_buckets: usize,
    first_bucket_width: u64,
}

impl Histogram {
    pub(crate) fn num_buckets(&self) -> usize {
        self.buckets.len()
    }

    cfg_64bit_metrics! {
        pub(crate) fn get(&self, bucket: usize) -> u64 {
            self.buckets[bucket].load(Relaxed)
        }
    }

    pub(crate) fn bucket_range(&self, bucket: usize) -> Range<u64> {
        self.histogram_type.bucket_range(bucket)
    }
}

impl HistogramBatch {
    pub(crate) fn from_histogram(histogram: &Histogram) -> HistogramBatch {
        let buckets = vec![0; histogram.buckets.len()].into_boxed_slice();

        HistogramBatch {
            buckets,
            configuration: histogram.histogram_type,
        }
    }

    pub(crate) fn measure(&mut self, value: u64, count: u64) {
        self.buckets[self.value_to_bucket(value)] += count;
    }

    pub(crate) fn submit(&self, histogram: &Histogram) {
        debug_assert_eq!(self.configuration, histogram.histogram_type);
        debug_assert_eq!(self.buckets.len(), histogram.buckets.len());

        for i in 0..self.buckets.len() {
            histogram.buckets[i].store(self.buckets[i], Relaxed);
        }
    }

    fn value_to_bucket(&self, value: u64) -> usize {
        self.configuration.value_to_bucket(value)
    }
}

impl HistogramBuilder {
    pub(crate) fn new() -> HistogramBuilder {
        HistogramBuilder {
            histogram_type: HistogramType::Linear(LinearHistogram {
                num_buckets: 10,
                bucket_width: 100_000,
            }),
            legacy: None,
        }
    }

    pub(crate) fn legacy_mut(&mut self, f: impl Fn(&mut LegacyBuilder)) {
        let legacy = self.legacy.get_or_insert_with(|| LegacyBuilder::default());
        f(legacy);
    }

    pub(crate) fn build(&self) -> Histogram {
        let histogram_type = match &self.legacy {
            Some(legacy) => {
                assert!(legacy.resolution > 0);
                match legacy.scale {
                    HistogramScale::Linear => HistogramType::Linear(LinearHistogram {
                        num_buckets: legacy.num_buckets,
                        bucket_width: legacy.resolution,
                    }),
                    HistogramScale::Log => HistogramType::LogLegacy(LegacyLogHistogram {
                        num_buckets: legacy.num_buckets,
                        first_bucket_width: legacy.resolution.next_power_of_two(),
                    }),
                }
            }
            None => self.histogram_type,
        };
        let num_buckets = histogram_type.num_buckets();

        Histogram {
            buckets: (0..num_buckets)
                .map(|_| MetricAtomicU64::new(0))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            histogram_type,
        }
    }
}

impl Default for HistogramBuilder {
    fn default() -> HistogramBuilder {
        HistogramBuilder::new()
    }
}

#[cfg(all(test, target_has_atomic = "64"))]
mod test {
    use super::*;

    macro_rules! assert_bucket_eq {
        ($h:expr, $bucket:expr, $val:expr) => {{
            assert_eq!($h.buckets[$bucket], $val);
        }};
    }

    fn linear(resolution: u64, num_buckets: usize) -> Histogram {
        HistogramBuilder {
            histogram_type: HistogramType::Linear(LinearHistogram {
                bucket_width: resolution,
                num_buckets,
            }),
            legacy: None,
        }
        .build()
    }

    #[test]
    fn test_legacy_builder() {
        let mut builder = HistogramBuilder::new();
        builder.legacy_mut(|b| b.num_buckets = 20);
        assert_eq!(builder.build().num_buckets(), 20);
    }

    #[test]
    fn log_scale_resolution_1() {
        let h = HistogramBuilder {
            histogram_type: HistogramType::LogLegacy(LegacyLogHistogram {
                first_bucket_width: 1,
                num_buckets: 10,
            }),
            legacy: None,
        }
        .build();

        assert_eq!(h.bucket_range(0), 0..1);
        assert_eq!(h.bucket_range(1), 1..2);
        assert_eq!(h.bucket_range(2), 2..4);
        assert_eq!(h.bucket_range(3), 4..8);
        assert_eq!(h.bucket_range(9), 256..u64::MAX);

        let mut b = HistogramBatch::from_histogram(&h);

        b.measure(0, 1);
        assert_bucket_eq!(b, 0, 1);
        assert_bucket_eq!(b, 1, 0);

        b.measure(1, 1);
        assert_bucket_eq!(b, 0, 1);
        assert_bucket_eq!(b, 1, 1);
        assert_bucket_eq!(b, 2, 0);

        b.measure(2, 1);
        assert_bucket_eq!(b, 0, 1);
        assert_bucket_eq!(b, 1, 1);
        assert_bucket_eq!(b, 2, 1);

        b.measure(3, 1);
        assert_bucket_eq!(b, 0, 1);
        assert_bucket_eq!(b, 1, 1);
        assert_bucket_eq!(b, 2, 2);

        b.measure(4, 1);
        assert_bucket_eq!(b, 0, 1);
        assert_bucket_eq!(b, 1, 1);
        assert_bucket_eq!(b, 2, 2);
        assert_bucket_eq!(b, 3, 1);

        b.measure(100, 1);
        assert_bucket_eq!(b, 7, 1);

        b.measure(128, 1);
        assert_bucket_eq!(b, 8, 1);

        b.measure(4096, 1);
        assert_bucket_eq!(b, 9, 1);

        b.measure(u64::MAX, 1);
        assert_bucket_eq!(b, 9, 2);
    }

    #[test]
    fn log_scale_resolution_2() {
        let h = HistogramBuilder {
            histogram_type: HistogramType::LogLegacy(LegacyLogHistogram {
                num_buckets: 10,
                first_bucket_width: 2,
            }),
            legacy: None,
        }
        .build();

        assert_eq!(h.bucket_range(0), 0..2);
        assert_eq!(h.bucket_range(1), 2..4);
        assert_eq!(h.bucket_range(2), 4..8);
        assert_eq!(h.bucket_range(3), 8..16);
        assert_eq!(h.bucket_range(9), 512..u64::MAX);

        let mut b = HistogramBatch::from_histogram(&h);

        b.measure(0, 1);
        assert_bucket_eq!(b, 0, 1);
        assert_bucket_eq!(b, 1, 0);

        b.measure(1, 1);
        assert_bucket_eq!(b, 0, 2);
        assert_bucket_eq!(b, 1, 0);

        b.measure(2, 1);
        assert_bucket_eq!(b, 0, 2);
        assert_bucket_eq!(b, 1, 1);
        assert_bucket_eq!(b, 2, 0);

        b.measure(3, 1);
        assert_bucket_eq!(b, 0, 2);
        assert_bucket_eq!(b, 1, 2);
        assert_bucket_eq!(b, 2, 0);

        b.measure(4, 1);
        assert_bucket_eq!(b, 0, 2);
        assert_bucket_eq!(b, 1, 2);
        assert_bucket_eq!(b, 2, 1);

        b.measure(5, 1);
        assert_bucket_eq!(b, 0, 2);
        assert_bucket_eq!(b, 1, 2);
        assert_bucket_eq!(b, 2, 2);

        b.measure(6, 1);
        assert_bucket_eq!(b, 0, 2);
        assert_bucket_eq!(b, 1, 2);
        assert_bucket_eq!(b, 2, 3);

        b.measure(7, 1);
        assert_bucket_eq!(b, 0, 2);
        assert_bucket_eq!(b, 1, 2);
        assert_bucket_eq!(b, 2, 4);

        b.measure(8, 1);
        assert_bucket_eq!(b, 0, 2);
        assert_bucket_eq!(b, 1, 2);
        assert_bucket_eq!(b, 2, 4);
        assert_bucket_eq!(b, 3, 1);

        b.measure(100, 1);
        assert_bucket_eq!(b, 6, 1);

        b.measure(128, 1);
        assert_bucket_eq!(b, 7, 1);

        b.measure(4096, 1);
        assert_bucket_eq!(b, 9, 1);

        for bucket in h.buckets.iter() {
            assert_eq!(bucket.load(Relaxed), 0);
        }

        b.submit(&h);

        for i in 0..h.buckets.len() {
            assert_eq!(h.buckets[i].load(Relaxed), b.buckets[i]);
        }

        b.submit(&h);

        for i in 0..h.buckets.len() {
            assert_eq!(h.buckets[i].load(Relaxed), b.buckets[i]);
        }
    }

    #[test]
    fn linear_scale_resolution_1() {
        let h = linear(1, 10);

        assert_eq!(h.bucket_range(0), 0..1);
        assert_eq!(h.bucket_range(1), 1..2);
        assert_eq!(h.bucket_range(2), 2..3);
        assert_eq!(h.bucket_range(3), 3..4);
        assert_eq!(h.bucket_range(9), 9..u64::MAX);

        let mut b = HistogramBatch::from_histogram(&h);

        b.measure(0, 1);
        assert_bucket_eq!(b, 0, 1);
        assert_bucket_eq!(b, 1, 0);

        b.measure(1, 1);
        assert_bucket_eq!(b, 0, 1);
        assert_bucket_eq!(b, 1, 1);
        assert_bucket_eq!(b, 2, 0);

        b.measure(2, 1);
        assert_bucket_eq!(b, 0, 1);
        assert_bucket_eq!(b, 1, 1);
        assert_bucket_eq!(b, 2, 1);
        assert_bucket_eq!(b, 3, 0);

        b.measure(3, 1);
        assert_bucket_eq!(b, 0, 1);
        assert_bucket_eq!(b, 1, 1);
        assert_bucket_eq!(b, 2, 1);
        assert_bucket_eq!(b, 3, 1);

        b.measure(5, 1);
        assert_bucket_eq!(b, 5, 1);

        b.measure(4096, 1);
        assert_bucket_eq!(b, 9, 1);

        for bucket in h.buckets.iter() {
            assert_eq!(bucket.load(Relaxed), 0);
        }

        b.submit(&h);

        for i in 0..h.buckets.len() {
            assert_eq!(h.buckets[i].load(Relaxed), b.buckets[i]);
        }

        b.submit(&h);

        for i in 0..h.buckets.len() {
            assert_eq!(h.buckets[i].load(Relaxed), b.buckets[i]);
        }
    }

    #[test]
    fn linear_scale_resolution_100() {
        let h = linear(100, 10);

        assert_eq!(h.bucket_range(0), 0..100);
        assert_eq!(h.bucket_range(1), 100..200);
        assert_eq!(h.bucket_range(2), 200..300);
        assert_eq!(h.bucket_range(3), 300..400);
        assert_eq!(h.bucket_range(9), 900..u64::MAX);

        let mut b = HistogramBatch::from_histogram(&h);

        b.measure(0, 1);
        assert_bucket_eq!(b, 0, 1);
        assert_bucket_eq!(b, 1, 0);

        b.measure(50, 1);
        assert_bucket_eq!(b, 0, 2);
        assert_bucket_eq!(b, 1, 0);

        b.measure(100, 1);
        assert_bucket_eq!(b, 0, 2);
        assert_bucket_eq!(b, 1, 1);
        assert_bucket_eq!(b, 2, 0);

        b.measure(101, 1);
        assert_bucket_eq!(b, 0, 2);
        assert_bucket_eq!(b, 1, 2);
        assert_bucket_eq!(b, 2, 0);

        b.measure(200, 1);
        assert_bucket_eq!(b, 0, 2);
        assert_bucket_eq!(b, 1, 2);
        assert_bucket_eq!(b, 2, 1);

        b.measure(299, 1);
        assert_bucket_eq!(b, 0, 2);
        assert_bucket_eq!(b, 1, 2);
        assert_bucket_eq!(b, 2, 2);

        b.measure(222, 1);
        assert_bucket_eq!(b, 0, 2);
        assert_bucket_eq!(b, 1, 2);
        assert_bucket_eq!(b, 2, 3);

        b.measure(300, 1);
        assert_bucket_eq!(b, 0, 2);
        assert_bucket_eq!(b, 1, 2);
        assert_bucket_eq!(b, 2, 3);
        assert_bucket_eq!(b, 3, 1);

        b.measure(888, 1);
        assert_bucket_eq!(b, 8, 1);

        b.measure(4096, 1);
        assert_bucket_eq!(b, 9, 1);

        for bucket in h.buckets.iter() {
            assert_eq!(bucket.load(Relaxed), 0);
        }

        b.submit(&h);

        for i in 0..h.buckets.len() {
            assert_eq!(h.buckets[i].load(Relaxed), b.buckets[i]);
        }

        b.submit(&h);

        for i in 0..h.buckets.len() {
            assert_eq!(h.buckets[i].load(Relaxed), b.buckets[i]);
        }
    }

    #[test]
    fn inc_by_more_than_one() {
        let h = linear(100, 10);

        let mut b = HistogramBatch::from_histogram(&h);

        b.measure(0, 3);
        assert_bucket_eq!(b, 0, 3);
        assert_bucket_eq!(b, 1, 0);

        b.measure(50, 5);
        assert_bucket_eq!(b, 0, 8);
        assert_bucket_eq!(b, 1, 0);

        b.measure(100, 2);
        assert_bucket_eq!(b, 0, 8);
        assert_bucket_eq!(b, 1, 2);
        assert_bucket_eq!(b, 2, 0);

        b.measure(101, 19);
        assert_bucket_eq!(b, 0, 8);
        assert_bucket_eq!(b, 1, 21);
        assert_bucket_eq!(b, 2, 0);

        for bucket in h.buckets.iter() {
            assert_eq!(bucket.load(Relaxed), 0);
        }

        b.submit(&h);

        for i in 0..h.buckets.len() {
            assert_eq!(h.buckets[i].load(Relaxed), b.buckets[i]);
        }

        b.submit(&h);

        for i in 0..h.buckets.len() {
            assert_eq!(h.buckets[i].load(Relaxed), b.buckets[i]);
        }
    }
}
