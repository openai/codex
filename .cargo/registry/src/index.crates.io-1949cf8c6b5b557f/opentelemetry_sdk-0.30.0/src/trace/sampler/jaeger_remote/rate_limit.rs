use opentelemetry::time::now;
use std::time::SystemTime;
// leaky bucket based rate limit
// should be Send+Sync
pub(crate) struct LeakyBucket {
    span_per_sec: f64,
    available: f64,
    bucket_size: f64,
    last_time: SystemTime,
}
use opentelemetry::otel_debug;

impl LeakyBucket {
    pub(crate) fn new(bucket_size: f64, span_per_sec: f64) -> LeakyBucket {
        LeakyBucket {
            span_per_sec,
            available: bucket_size,
            bucket_size,
            last_time: now(),
        }
    }

    pub(crate) fn update(&mut self, span_per_sec: f64) {
        self.span_per_sec = span_per_sec;
    }

    pub(crate) fn should_sample(&mut self) -> bool {
        self.check_availability(now)
    }

    fn check_availability<F>(&mut self, now: F) -> bool
    where
        F: Fn() -> SystemTime,
    {
        if self.available >= 1.0 {
            self.available -= 1.0;
            true
        } else {
            let cur_time = now();
            let elapsed = cur_time.duration_since(self.last_time);
            match elapsed {
                Ok(dur) => {
                    self.last_time = cur_time;
                    self.available = f64::min(
                        dur.as_secs() as f64 * self.span_per_sec + self.available,
                        self.bucket_size,
                    );

                    if self.available >= 1.0 {
                        self.available -= 1.0;
                        true
                    } else {
                        false
                    }
                }
                Err(err) => {
                    otel_debug!(
                        name: "JaegerRemoteSampler.LeakyBucket.ClockAdjustment",
                        message = "Jaeger remote sampler detected a rewind in system clock",
                        reason = format!("{:?}", err),
                    );
                    true
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::trace::sampler::jaeger_remote::rate_limit::LeakyBucket;
    use opentelemetry::time::now;
    use std::ops::{Add, Sub};
    use std::time::Duration;

    #[test]
    fn test_leaky_bucket() {
        // maximum bucket size 2, add 1 allowance every 10 seconds
        let mut leaky_bucket = LeakyBucket::new(2.0, 0.1);
        let current_time = now();
        leaky_bucket.last_time = current_time;

        let test_cases = vec![
            (0, vec![true, true, false]),
            (1, vec![false]),
            (5, vec![false]),
            (10, vec![true, false]),
            (60, vec![true, true, false]), // maximum allowance is 2
        ];

        for (elapsed_sec, cases) in test_cases.into_iter() {
            for should_pass in cases {
                assert_eq!(
                    should_pass,
                    leaky_bucket.check_availability(|| {
                        current_time.add(Duration::from_secs(elapsed_sec))
                    })
                )
            }
        }
    }

    #[test]
    fn test_rewind_clock_should_pass() {
        let mut leaky_bucket = LeakyBucket::new(2.0, 0.1);
        let current_time = now();
        leaky_bucket.last_time = current_time;

        assert!(leaky_bucket.check_availability(|| { current_time.sub(Duration::from_secs(10)) }))
    }
}
