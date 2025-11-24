use core::fmt;

/// Error of parsing an enum value its string representation.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct FromStrError {
    type_name: &'static str,
}

impl FromStrError {
    #[doc(hidden)]
    #[must_use]
    #[inline]
    pub const fn new(type_name: &'static str) -> Self {
        Self { type_name }
    }
}

impl fmt::Display for FromStrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid `{}` string representation", self.type_name)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FromStrError {}
