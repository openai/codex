// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::*;
use crate::parts_write_adapter::CoreWriteAsPartsWrite;
use core::convert::Infallible;

/// A writeable object that can fail while writing.
///
/// The default [`Writeable`] trait returns a [`fmt::Error`], which originates from the sink.
/// In contrast, this trait allows the _writeable itself_ to trigger an error as well.
///
/// Implementations are expected to always make a _best attempt_ at writing to the sink
/// and should write replacement values in the error state. Therefore, the returned `Result`
/// can be safely ignored to emulate a "lossy" mode.
///
/// Any error substrings should be annotated with [`Part::ERROR`].
///
/// # Implementer Notes
///
/// This trait requires that implementers make a _best attempt_ at writing to the sink,
/// _even in the error state_, such as with a placeholder or fallback string.
///
/// In [`TryWriteable::try_write_to_parts()`], error substrings should be annotated with
/// [`Part::ERROR`]. Because of this, writing to parts is not default-implemented like
/// it is on [`Writeable`].
///
/// The trait is implemented on [`Result<T, E>`] where `T` and `E` both implement [`Writeable`];
/// In the `Ok` case, `T` is written, and in the `Err` case, `E` is written as a fallback value.
/// This impl, which writes [`Part::ERROR`], can be used as a basis for more advanced impls.
///
/// # Examples
///
/// Implementing on a custom type:
///
/// ```
/// use core::fmt;
/// use writeable::LengthHint;
/// use writeable::PartsWrite;
/// use writeable::TryWriteable;
///
/// #[derive(Debug, PartialEq, Eq)]
/// enum HelloWorldWriteableError {
///     MissingName,
/// }
///
/// #[derive(Debug, PartialEq, Eq)]
/// struct HelloWorldWriteable {
///     pub name: Option<&'static str>,
/// }
///
/// impl TryWriteable for HelloWorldWriteable {
///     type Error = HelloWorldWriteableError;
///
///     fn try_write_to_parts<S: PartsWrite + ?Sized>(
///         &self,
///         sink: &mut S,
///     ) -> Result<Result<(), Self::Error>, fmt::Error> {
///         sink.write_str("Hello, ")?;
///         // Use `impl TryWriteable for Result` to generate the error part:
///         let err = self.name.ok_or("nobody").try_write_to_parts(sink)?.err();
///         sink.write_char('!')?;
///         // Return a doubly-wrapped Result.
///         // The outer Result is for fmt::Error, handled by the `?`s above.
///         // The inner Result is for our own Self::Error.
///         if err.is_none() {
///             Ok(Ok(()))
///         } else {
///             Ok(Err(HelloWorldWriteableError::MissingName))
///         }
///     }
///
///     fn writeable_length_hint(&self) -> LengthHint {
///         self.name.ok_or("nobody").writeable_length_hint() + 8
///     }
/// }
///
/// // Success case:
/// writeable::assert_try_writeable_eq!(
///     HelloWorldWriteable {
///         name: Some("Alice")
///     },
///     "Hello, Alice!"
/// );
///
/// // Failure case, including the ERROR part:
/// writeable::assert_try_writeable_parts_eq!(
///     HelloWorldWriteable { name: None },
///     "Hello, nobody!",
///     Err(HelloWorldWriteableError::MissingName),
///     [(7, 13, writeable::Part::ERROR)]
/// );
/// ```
pub trait TryWriteable {
    type Error;

    /// Writes the content of this writeable to a sink.
    ///
    /// If the sink hits an error, writing immediately ends,
    /// `Err(`[`fmt::Error`]`)` is returned, and the sink does not contain valid output.
    ///
    /// If the writeable hits an error, writing is continued with a replacement value,
    /// `Ok(Err(`[`TryWriteable::Error`]`))` is returned, and the caller may continue using the sink.
    ///
    /// # Lossy Mode
    ///
    /// The [`fmt::Error`] should always be handled, but the [`TryWriteable::Error`] can be
    /// ignored if a fallback string is desired instead of an error.
    ///
    /// To handle the sink error, but not the writeable error, write:
    ///
    /// ```
    /// # use writeable::TryWriteable;
    /// # let my_writeable: Result<&str, &str> = Ok("");
    /// # let mut sink = String::new();
    /// let _ = my_writeable.try_write_to(&mut sink)?;
    /// # Ok::<(), core::fmt::Error>(())
    /// ```
    ///
    /// # Examples
    ///
    /// The following examples use `Result<&str, usize>`, which implements [`TryWriteable`] because both `&str` and `usize` do.
    ///
    /// Success case:
    ///
    /// ```
    /// use writeable::TryWriteable;
    ///
    /// let w: Result<&str, usize> = Ok("success");
    /// let mut sink = String::new();
    /// let result = w.try_write_to(&mut sink);
    ///
    /// assert_eq!(result, Ok(Ok(())));
    /// assert_eq!(sink, "success");
    /// ```
    ///
    /// Failure case:
    ///
    /// ```
    /// use writeable::TryWriteable;
    ///
    /// let w: Result<&str, usize> = Err(44);
    /// let mut sink = String::new();
    /// let result = w.try_write_to(&mut sink);
    ///
    /// assert_eq!(result, Ok(Err(44)));
    /// assert_eq!(sink, "44");
    /// ```
    fn try_write_to<W: fmt::Write + ?Sized>(
        &self,
        sink: &mut W,
    ) -> Result<Result<(), Self::Error>, fmt::Error> {
        self.try_write_to_parts(&mut CoreWriteAsPartsWrite(sink))
    }

    /// Writes the content of this writeable to a sink with parts (annotations).
    ///
    /// For more information, see:
    ///
    /// - [`TryWriteable::try_write_to()`] for the general behavior.
    /// - [`TryWriteable`] for an example with parts.
    /// - [`Part`] for more about parts.
    fn try_write_to_parts<S: PartsWrite + ?Sized>(
        &self,
        sink: &mut S,
    ) -> Result<Result<(), Self::Error>, fmt::Error>;

    /// Returns a hint for the number of UTF-8 bytes that will be written to the sink.
    ///
    /// This function returns the length of the "lossy mode" string; for more information,
    /// see [`TryWriteable::try_write_to()`].
    fn writeable_length_hint(&self) -> LengthHint {
        LengthHint::undefined()
    }

    /// Writes the content of this writeable to a string.
    ///
    /// In the failure case, this function returns the error and the best-effort string ("lossy mode").
    ///
    /// Examples
    ///
    /// ```
    /// # use std::borrow::Cow;
    /// # use writeable::TryWriteable;
    /// // use the best-effort string
    /// let r1: Cow<str> = Ok::<&str, u8>("ok")
    ///     .try_write_to_string()
    ///     .unwrap_or_else(|(_, s)| s);
    /// // propagate the error
    /// let r2: Result<Cow<str>, u8> = Ok::<&str, u8>("ok")
    ///     .try_write_to_string()
    ///     .map_err(|(e, _)| e);
    /// ```
    #[cfg(feature = "alloc")]
    fn try_write_to_string(&self) -> Result<Cow<'_, str>, (Self::Error, Cow<'_, str>)> {
        let hint = self.writeable_length_hint();
        if hint.is_zero() {
            return Ok(Cow::Borrowed(""));
        }
        let mut output = String::with_capacity(hint.capacity());
        match self
            .try_write_to(&mut output)
            .unwrap_or_else(|fmt::Error| Ok(()))
        {
            Ok(()) => Ok(Cow::Owned(output)),
            Err(e) => Err((e, Cow::Owned(output))),
        }
    }
}

impl<T, E> TryWriteable for Result<T, E>
where
    T: Writeable,
    E: Writeable + Clone,
{
    type Error = E;

    #[inline]
    fn try_write_to<W: fmt::Write + ?Sized>(
        &self,
        sink: &mut W,
    ) -> Result<Result<(), Self::Error>, fmt::Error> {
        match self {
            Ok(t) => t.write_to(sink).map(Ok),
            Err(e) => e.write_to(sink).map(|()| Err(e.clone())),
        }
    }

    #[inline]
    fn try_write_to_parts<S: PartsWrite + ?Sized>(
        &self,
        sink: &mut S,
    ) -> Result<Result<(), Self::Error>, fmt::Error> {
        match self {
            Ok(t) => t.write_to_parts(sink).map(Ok),
            Err(e) => sink
                .with_part(Part::ERROR, |sink| e.write_to_parts(sink))
                .map(|()| Err(e.clone())),
        }
    }

    #[inline]
    fn writeable_length_hint(&self) -> LengthHint {
        match self {
            Ok(t) => t.writeable_length_hint(),
            Err(e) => e.writeable_length_hint(),
        }
    }

    #[inline]
    #[cfg(feature = "alloc")]
    fn try_write_to_string(&self) -> Result<Cow<'_, str>, (Self::Error, Cow<'_, str>)> {
        match self {
            Ok(t) => Ok(t.write_to_string()),
            Err(e) => Err((e.clone(), e.write_to_string())),
        }
    }
}

/// A wrapper around [`TryWriteable`] that implements [`Writeable`]
/// if [`TryWriteable::Error`] is [`Infallible`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
#[allow(clippy::exhaustive_structs)] // transparent newtype
pub struct TryWriteableInfallibleAsWriteable<T>(pub T);

impl<T> Writeable for TryWriteableInfallibleAsWriteable<T>
where
    T: TryWriteable<Error = Infallible>,
{
    #[inline]
    fn write_to<W: fmt::Write + ?Sized>(&self, sink: &mut W) -> fmt::Result {
        match self.0.try_write_to(sink) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(infallible)) => match infallible {},
            Err(e) => Err(e),
        }
    }

    #[inline]
    fn write_to_parts<S: PartsWrite + ?Sized>(&self, sink: &mut S) -> fmt::Result {
        match self.0.try_write_to_parts(sink) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(infallible)) => match infallible {},
            Err(e) => Err(e),
        }
    }

    #[inline]
    fn writeable_length_hint(&self) -> LengthHint {
        self.0.writeable_length_hint()
    }

    #[inline]
    #[cfg(feature = "alloc")]
    fn write_to_string(&self) -> Cow<'_, str> {
        match self.0.try_write_to_string() {
            Ok(s) => s,
            Err((infallible, _)) => match infallible {},
        }
    }
}

impl<T> fmt::Display for TryWriteableInfallibleAsWriteable<T>
where
    T: TryWriteable<Error = Infallible>,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_to(f)
    }
}

/// A wrapper around [`Writeable`] that implements [`TryWriteable`]
/// with [`TryWriteable::Error`] set to [`Infallible`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
#[allow(clippy::exhaustive_structs)] // transparent newtype
pub struct WriteableAsTryWriteableInfallible<T>(pub T);

impl<T> TryWriteable for WriteableAsTryWriteableInfallible<T>
where
    T: Writeable,
{
    type Error = Infallible;

    #[inline]
    fn try_write_to<W: fmt::Write + ?Sized>(
        &self,
        sink: &mut W,
    ) -> Result<Result<(), Infallible>, fmt::Error> {
        self.0.write_to(sink).map(Ok)
    }

    #[inline]
    fn try_write_to_parts<S: PartsWrite + ?Sized>(
        &self,
        sink: &mut S,
    ) -> Result<Result<(), Infallible>, fmt::Error> {
        self.0.write_to_parts(sink).map(Ok)
    }

    #[inline]
    fn writeable_length_hint(&self) -> LengthHint {
        self.0.writeable_length_hint()
    }

    #[inline]
    #[cfg(feature = "alloc")]
    fn try_write_to_string(&self) -> Result<Cow<'_, str>, (Infallible, Cow<'_, str>)> {
        Ok(self.0.write_to_string())
    }
}

/// Testing macros for types implementing [`TryWriteable`].
///
/// Arguments, in order:
///
/// 1. The [`TryWriteable`] under test
/// 2. The expected string value
/// 3. The expected result value, or `Ok(())` if omitted
/// 3. [`*_parts_eq`] only: a list of parts (`[(start, end, Part)]`)
///
/// Any remaining arguments get passed to `format!`
///
/// The macros tests the following:
///
/// - Equality of string content
/// - Equality of parts ([`*_parts_eq`] only)
/// - Validity of size hint
///
/// For a usage example, see [`TryWriteable`].
///
/// [`*_parts_eq`]: assert_try_writeable_parts_eq
#[macro_export]
macro_rules! assert_try_writeable_eq {
    ($actual_writeable:expr, $expected_str:expr $(,)?) => {
        $crate::assert_try_writeable_eq!($actual_writeable, $expected_str, Ok(()))
    };
    ($actual_writeable:expr, $expected_str:expr, $expected_result:expr $(,)?) => {
        $crate::assert_try_writeable_eq!($actual_writeable, $expected_str, $expected_result, "")
    };
    ($actual_writeable:expr, $expected_str:expr, $expected_result:expr, $($arg:tt)+) => {{
        $crate::assert_try_writeable_eq!(@internal, $actual_writeable, $expected_str, $expected_result, $($arg)*);
    }};
    (@internal, $actual_writeable:expr, $expected_str:expr, $expected_result:expr, $($arg:tt)+) => {{
        use $crate::TryWriteable;
        let actual_writeable = &$actual_writeable;
        let (actual_str, actual_parts, actual_error) = $crate::_internal::try_writeable_to_parts_for_test(actual_writeable);
        assert_eq!(actual_str, $expected_str, $($arg)*);
        assert_eq!(actual_error, Result::<(), _>::from($expected_result).err(), $($arg)*);
        let actual_result = match actual_writeable.try_write_to_string() {
            Ok(actual_cow_str) => {
                assert_eq!(actual_cow_str, $expected_str, $($arg)+);
                Ok(())
            }
            Err((e, actual_cow_str)) => {
                assert_eq!(actual_cow_str, $expected_str, $($arg)+);
                Err(e)
            }
        };
        assert_eq!(actual_result, Result::<(), _>::from($expected_result), $($arg)*);
        let length_hint = actual_writeable.writeable_length_hint();
        assert!(
            length_hint.0 <= actual_str.len(),
            "hint lower bound {} larger than actual length {}: {}",
            length_hint.0, actual_str.len(), format!($($arg)*),
        );
        if let Some(upper) = length_hint.1 {
            assert!(
                actual_str.len() <= upper,
                "hint upper bound {} smaller than actual length {}: {}",
                length_hint.0, actual_str.len(), format!($($arg)*),
            );
        }
        actual_parts // return for assert_try_writeable_parts_eq
    }};
}

/// See [`assert_try_writeable_eq`].
#[macro_export]
macro_rules! assert_try_writeable_parts_eq {
    ($actual_writeable:expr, $expected_str:expr, $expected_parts:expr $(,)?) => {
        $crate::assert_try_writeable_parts_eq!($actual_writeable, $expected_str, Ok(()), $expected_parts)
    };
    ($actual_writeable:expr, $expected_str:expr, $expected_result:expr, $expected_parts:expr $(,)?) => {
        $crate::assert_try_writeable_parts_eq!($actual_writeable, $expected_str, $expected_result, $expected_parts, "")
    };
    ($actual_writeable:expr, $expected_str:expr, $expected_result:expr, $expected_parts:expr, $($arg:tt)+) => {{
        let actual_parts = $crate::assert_try_writeable_eq!(@internal, $actual_writeable, $expected_str, $expected_result, $($arg)*);
        assert_eq!(actual_parts, $expected_parts, $($arg)+);
    }};
}

#[test]
fn test_result_try_writeable() {
    let mut result: Result<&str, usize> = Ok("success");
    assert_try_writeable_eq!(result, "success");
    result = Err(44);
    assert_try_writeable_eq!(result, "44", Err(44));
    assert_try_writeable_parts_eq!(result, "44", Err(44), [(0, 2, Part::ERROR)])
}
