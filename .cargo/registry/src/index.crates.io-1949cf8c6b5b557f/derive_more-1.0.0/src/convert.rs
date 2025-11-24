//! Definitions used in derived implementations of [`core::convert`] traits.

#[cfg(feature = "try_from")]
pub use self::try_from::TryFromReprError;
#[cfg(feature = "try_into")]
pub use self::try_into::TryIntoError;

#[cfg(feature = "try_from")]
mod try_from {
    use core::fmt;

    /// Error returned by the derived [`TryFrom`] implementation on enums to
    /// convert from their repr.
    ///
    /// [`TryFrom`]: macro@crate::TryFrom
    #[derive(Clone, Copy, Debug)]
    pub struct TryFromReprError<T> {
        /// Original input value which failed to convert via the derived
        /// [`TryFrom`] implementation.
        ///
        /// [`TryFrom`]: macro@crate::TryFrom
        pub input: T,
    }

    impl<T> TryFromReprError<T> {
        #[doc(hidden)]
        #[must_use]
        #[inline]
        pub const fn new(input: T) -> Self {
            Self { input }
        }
    }

    // `T`, as a discriminant, should only be an integer type, and therefore be `Debug`.
    impl<T: fmt::Debug> fmt::Display for TryFromReprError<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "`{:?}` does not correspond to a unit variant",
                self.input
            )
        }
    }

    #[cfg(feature = "std")]
    // `T` should only be an integer type and therefore be debug
    impl<T: fmt::Debug> std::error::Error for TryFromReprError<T> {}
}

#[cfg(feature = "try_into")]
mod try_into {
    use core::fmt;

    /// Error returned by the derived [`TryInto`] implementation.
    ///
    /// [`TryInto`]: macro@crate::TryInto
    #[derive(Clone, Copy, Debug)]
    pub struct TryIntoError<T> {
        /// Original input value which failed to convert via the derived
        /// [`TryInto`] implementation.
        ///
        /// [`TryInto`]: macro@crate::TryInto
        pub input: T,
        variant_names: &'static str,
        output_type: &'static str,
    }

    impl<T> TryIntoError<T> {
        #[doc(hidden)]
        #[must_use]
        #[inline]
        pub const fn new(
            input: T,
            variant_names: &'static str,
            output_type: &'static str,
        ) -> Self {
            Self {
                input,
                variant_names,
                output_type,
            }
        }
    }

    impl<T> fmt::Display for TryIntoError<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "Only {} can be converted to {}",
                self.variant_names, self.output_type,
            )
        }
    }

    #[cfg(feature = "std")]
    impl<T: fmt::Debug> std::error::Error for TryIntoError<T> {}
}
