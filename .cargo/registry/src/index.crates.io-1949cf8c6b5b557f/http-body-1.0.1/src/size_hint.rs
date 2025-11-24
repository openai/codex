/// A `Body` size hint
///
/// The default implementation returns:
///
/// * 0 for `lower`
/// * `None` for `upper`.
#[derive(Debug, Default, Clone)]
pub struct SizeHint {
    lower: u64,
    upper: Option<u64>,
}

impl SizeHint {
    /// Returns a new `SizeHint` with default values
    #[inline]
    pub fn new() -> SizeHint {
        SizeHint::default()
    }

    /// Returns a new `SizeHint` with both upper and lower bounds set to the
    /// given value.
    #[inline]
    pub fn with_exact(value: u64) -> SizeHint {
        SizeHint {
            lower: value,
            upper: Some(value),
        }
    }

    /// Returns the lower bound of data that the `Body` will yield before
    /// completing.
    #[inline]
    pub fn lower(&self) -> u64 {
        self.lower
    }

    /// Set the value of the `lower` hint.
    ///
    /// # Panics
    ///
    /// The function panics if `value` is greater than `upper`.
    #[inline]
    pub fn set_lower(&mut self, value: u64) {
        assert!(value <= self.upper.unwrap_or(u64::MAX));
        self.lower = value;
    }

    /// Returns the upper bound of data the `Body` will yield before
    /// completing, or `None` if the value is unknown.
    #[inline]
    pub fn upper(&self) -> Option<u64> {
        self.upper
    }

    /// Set the value of the `upper` hint value.
    ///
    /// # Panics
    ///
    /// This function panics if `value` is less than `lower`.
    #[inline]
    pub fn set_upper(&mut self, value: u64) {
        assert!(value >= self.lower, "`value` is less than than `lower`");

        self.upper = Some(value);
    }

    /// Returns the exact size of data that will be yielded **if** the
    /// `lower` and `upper` bounds are equal.
    #[inline]
    pub fn exact(&self) -> Option<u64> {
        if Some(self.lower) == self.upper {
            self.upper
        } else {
            None
        }
    }

    /// Set the value of the `lower` and `upper` bounds to exactly the same.
    #[inline]
    pub fn set_exact(&mut self, value: u64) {
        self.lower = value;
        self.upper = Some(value);
    }
}
