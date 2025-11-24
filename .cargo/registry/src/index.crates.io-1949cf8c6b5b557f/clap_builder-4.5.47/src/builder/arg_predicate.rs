use crate::builder::OsStr;

/// Operations to perform on argument values
///
/// These do not apply to [`ValueSource::DefaultValue`][crate::parser::ValueSource::DefaultValue]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "unstable-v5", non_exhaustive)]
pub enum ArgPredicate {
    /// Is the argument present?
    IsPresent,
    /// Does the argument match the specified value?
    Equals(OsStr),
}

impl<S: Into<OsStr>> From<S> for ArgPredicate {
    fn from(other: S) -> Self {
        Self::Equals(other.into())
    }
}
