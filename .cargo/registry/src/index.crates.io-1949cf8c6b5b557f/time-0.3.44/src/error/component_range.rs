//! Component range error

use core::{fmt, hash};

use crate::error;

/// An error type indicating that a component provided to a method was out of range, causing a
/// failure.
// i64 is the narrowest type fitting all use cases. This eliminates the need for a type parameter.
#[derive(Debug, Clone, Copy, Eq)]
pub struct ComponentRange {
    /// Name of the component.
    pub(crate) name: &'static str,
    /// Minimum allowed value, inclusive.
    pub(crate) minimum: i64,
    /// Maximum allowed value, inclusive.
    pub(crate) maximum: i64,
    /// Value that was provided.
    pub(crate) value: i64,
    /// The minimum and/or maximum value is conditional on the value of other
    /// parameters.
    pub(crate) conditional_message: Option<&'static str>,
}

impl ComponentRange {
    /// Obtain the name of the component whose value was out of range.
    #[inline]
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Whether the value's permitted range is conditional, i.e. whether an input with this
    /// value could have succeeded if the values of other components were different.
    #[inline]
    pub const fn is_conditional(self) -> bool {
        self.conditional_message.is_some()
    }
}

impl PartialEq for ComponentRange {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
        && self.minimum == other.minimum
        && self.maximum == other.maximum
        && self.value == other.value
        // Skip the contents of the message when comparing for equality.
        && self.conditional_message.is_some() == other.conditional_message.is_some()
    }
}

impl hash::Hash for ComponentRange {
    #[inline]
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.minimum.hash(state);
        self.maximum.hash(state);
        self.value.hash(state);
        // Skip the contents of the message when comparing for equality.
        self.conditional_message.is_some().hash(state);
    }
}

impl fmt::Display for ComponentRange {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} must be in the range {}..={}",
            self.name, self.minimum, self.maximum
        )?;

        if let Some(message) = self.conditional_message {
            write!(f, " {message}")?;
        }

        Ok(())
    }
}

impl From<ComponentRange> for crate::Error {
    #[inline]
    fn from(original: ComponentRange) -> Self {
        Self::ComponentRange(original)
    }
}

impl TryFrom<crate::Error> for ComponentRange {
    type Error = error::DifferentVariant;

    #[inline]
    fn try_from(err: crate::Error) -> Result<Self, Self::Error> {
        match err {
            crate::Error::ComponentRange(err) => Ok(err),
            _ => Err(error::DifferentVariant),
        }
    }
}

/// **This trait implementation is deprecated and will be removed in a future breaking release.**
#[cfg(feature = "serde")]
impl serde::de::Expected for ComponentRange {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a value in the range {}..={}",
            self.minimum, self.maximum
        )
    }
}

#[cfg(feature = "serde")]
impl ComponentRange {
    /// Convert the error to a deserialization error.
    #[inline]
    pub(crate) fn into_de_error<E: serde::de::Error>(self) -> E {
        E::invalid_value(serde::de::Unexpected::Signed(self.value), &self)
    }
}

impl core::error::Error for ComponentRange {}
