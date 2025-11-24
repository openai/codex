use std::time::Duration;

/// A rate of requests per time period.
#[derive(Debug, Copy, Clone)]
pub struct Rate {
    num: u64,
    per: Duration,
}

impl Rate {
    /// Create a new rate.
    ///
    /// # Panics
    ///
    /// This function panics if `num` or `per` is 0.
    pub const fn new(num: u64, per: Duration) -> Self {
        assert!(num > 0);
        assert!(per.as_nanos() > 0);

        Rate { num, per }
    }

    pub(crate) fn num(&self) -> u64 {
        self.num
    }

    pub(crate) fn per(&self) -> Duration {
        self.per
    }
}
