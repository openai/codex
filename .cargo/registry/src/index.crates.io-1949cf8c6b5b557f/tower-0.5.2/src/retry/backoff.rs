//! This module contains generic [backoff] utilities to be used with the retry
//! layer.
//!
//! The [`Backoff`] trait is a generic way to represent backoffs that can use
//! any timer type.
//!
//! [`ExponentialBackoffMaker`] implements the maker type for  
//! [`ExponentialBackoff`] which implements the [`Backoff`] trait and provides
//! a batteries included exponential backoff and jitter strategy.
//!
//! [backoff]: https://en.wikipedia.org/wiki/Exponential_backoff

use std::fmt::Display;
use std::future::Future;
use std::time::Duration;
use tokio::time;

use crate::util::rng::{HasherRng, Rng};

/// Trait used to construct [`Backoff`] trait implementors.
pub trait MakeBackoff {
    /// The backoff type produced by this maker.
    type Backoff: Backoff;

    /// Constructs a new backoff type.
    fn make_backoff(&mut self) -> Self::Backoff;
}

/// A backoff trait where a single mutable reference represents a single
/// backoff session. Implementors must also implement [`Clone`] which will
/// reset the backoff back to the default state for the next session.
pub trait Backoff {
    /// The future associated with each backoff. This usually will be some sort
    /// of timer.
    type Future: Future<Output = ()>;

    /// Initiate the next backoff in the sequence.
    fn next_backoff(&mut self) -> Self::Future;
}

/// A maker type for [`ExponentialBackoff`].
#[derive(Debug, Clone)]
pub struct ExponentialBackoffMaker<R = HasherRng> {
    /// The minimum amount of time to wait before resuming an operation.
    min: time::Duration,
    /// The maximum amount of time to wait before resuming an operation.
    max: time::Duration,
    /// The ratio of the base timeout that may be randomly added to a backoff.
    ///
    /// Must be greater than or equal to 0.0.
    jitter: f64,
    rng: R,
}

/// A jittered [exponential backoff] strategy.
///
/// The backoff duration will increase exponentially for every subsequent
/// backoff, up to a maximum duration. A small amount of [random jitter] is
/// added to each backoff duration, in order to avoid retry spikes.
///
/// [exponential backoff]: https://en.wikipedia.org/wiki/Exponential_backoff
/// [random jitter]: https://aws.amazon.com/blogs/architecture/exponential-backoff-and-jitter/
#[derive(Debug, Clone)]
pub struct ExponentialBackoff<R = HasherRng> {
    min: time::Duration,
    max: time::Duration,
    jitter: f64,
    rng: R,
    iterations: u32,
}

impl<R> ExponentialBackoffMaker<R>
where
    R: Rng,
{
    /// Create a new `ExponentialBackoff`.
    ///
    /// # Error
    ///
    /// Returns a config validation error if:
    /// - `min` > `max`
    /// - `max` > 0
    /// - `jitter` >= `0.0`
    /// - `jitter` < `100.0`
    /// - `jitter` is finite
    pub fn new(
        min: time::Duration,
        max: time::Duration,
        jitter: f64,
        rng: R,
    ) -> Result<Self, InvalidBackoff> {
        if min > max {
            return Err(InvalidBackoff("maximum must not be less than minimum"));
        }
        if max == time::Duration::from_millis(0) {
            return Err(InvalidBackoff("maximum must be non-zero"));
        }
        if jitter < 0.0 {
            return Err(InvalidBackoff("jitter must not be negative"));
        }
        if jitter > 100.0 {
            return Err(InvalidBackoff("jitter must not be greater than 100"));
        }
        if !jitter.is_finite() {
            return Err(InvalidBackoff("jitter must be finite"));
        }

        Ok(ExponentialBackoffMaker {
            min,
            max,
            jitter,
            rng,
        })
    }
}

impl<R> MakeBackoff for ExponentialBackoffMaker<R>
where
    R: Rng + Clone,
{
    type Backoff = ExponentialBackoff<R>;

    fn make_backoff(&mut self) -> Self::Backoff {
        ExponentialBackoff {
            max: self.max,
            min: self.min,
            jitter: self.jitter,
            rng: self.rng.clone(),
            iterations: 0,
        }
    }
}

impl<R: Rng> ExponentialBackoff<R> {
    fn base(&self) -> time::Duration {
        debug_assert!(
            self.min <= self.max,
            "maximum backoff must not be less than minimum backoff"
        );
        debug_assert!(
            self.max > time::Duration::from_millis(0),
            "Maximum backoff must be non-zero"
        );
        self.min
            .checked_mul(2_u32.saturating_pow(self.iterations))
            .unwrap_or(self.max)
            .min(self.max)
    }

    /// Returns a random, uniform duration on `[0, base*self.jitter]` no greater
    /// than `self.max`.
    fn jitter(&mut self, base: time::Duration) -> time::Duration {
        if self.jitter == 0.0 {
            time::Duration::default()
        } else {
            let jitter_factor = self.rng.next_f64();
            debug_assert!(
                jitter_factor > 0.0,
                "rng returns values between 0.0 and 1.0"
            );
            let rand_jitter = jitter_factor * self.jitter;
            let secs = (base.as_secs() as f64) * rand_jitter;
            let nanos = (base.subsec_nanos() as f64) * rand_jitter;
            let remaining = self.max - base;
            time::Duration::new(secs as u64, nanos as u32).min(remaining)
        }
    }
}

impl<R> Backoff for ExponentialBackoff<R>
where
    R: Rng,
{
    type Future = tokio::time::Sleep;

    fn next_backoff(&mut self) -> Self::Future {
        let base = self.base();
        let next = base + self.jitter(base);

        self.iterations += 1;

        tokio::time::sleep(next)
    }
}

impl Default for ExponentialBackoffMaker {
    fn default() -> Self {
        ExponentialBackoffMaker::new(
            Duration::from_millis(50),
            Duration::from_millis(u64::MAX),
            0.99,
            HasherRng::default(),
        )
        .expect("Unable to create ExponentialBackoff")
    }
}

/// Backoff validation error.
#[derive(Debug)]
pub struct InvalidBackoff(&'static str);

impl Display for InvalidBackoff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid backoff: {}", self.0)
    }
}

impl std::error::Error for InvalidBackoff {}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::*;

    quickcheck! {
        fn backoff_base_first(min_ms: u64, max_ms: u64) -> TestResult {
            let min = time::Duration::from_millis(min_ms);
            let max = time::Duration::from_millis(max_ms);
            let rng = HasherRng::default();
            let mut backoff = match ExponentialBackoffMaker::new(min, max, 0.0, rng) {
                Err(_) => return TestResult::discard(),
                Ok(backoff) => backoff,
            };
            let backoff = backoff.make_backoff();

            let delay = backoff.base();
            TestResult::from_bool(min == delay)
        }

        fn backoff_base(min_ms: u64, max_ms: u64, iterations: u32) -> TestResult {
            let min = time::Duration::from_millis(min_ms);
            let max = time::Duration::from_millis(max_ms);
            let rng = HasherRng::default();
            let mut backoff = match ExponentialBackoffMaker::new(min, max, 0.0, rng) {
                Err(_) => return TestResult::discard(),
                Ok(backoff) => backoff,
            };
            let mut backoff = backoff.make_backoff();

            backoff.iterations = iterations;
            let delay = backoff.base();
            TestResult::from_bool(min <= delay && delay <= max)
        }

        fn backoff_jitter(base_ms: u64, max_ms: u64, jitter: f64) -> TestResult {
            let base = time::Duration::from_millis(base_ms);
            let max = time::Duration::from_millis(max_ms);
            let rng = HasherRng::default();
            let mut backoff = match ExponentialBackoffMaker::new(base, max, jitter, rng) {
                Err(_) => return TestResult::discard(),
                Ok(backoff) => backoff,
            };
            let mut backoff = backoff.make_backoff();

            let j = backoff.jitter(base);
            if jitter == 0.0 || base_ms == 0 || max_ms == base_ms {
                TestResult::from_bool(j == time::Duration::default())
            } else {
                TestResult::from_bool(j > time::Duration::default())
            }
        }
    }
}
