//! Utilities for formatting (especially `Display` trait).
//!
//! This module contains utilities for [`Display`][`core::fmt::Display`]-able
//! types.

use core::fmt::{self, Write as _};

#[cfg(feature = "alloc")]
use alloc::collections::TryReserveError;
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::string::String;

/// Output buffer capacity overflow error.
#[derive(Debug, Clone, Copy)]
pub struct CapacityOverflowError;

impl fmt::Display for CapacityOverflowError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("buffer capacity overflow")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CapacityOverflowError {}

/// Writer to the bytes buffer.
struct ByteBufWriter<'b> {
    /// Destination buffer.
    buffer: &'b mut [u8],
    /// Position to write the next string fragment.
    cursor: usize,
}

impl fmt::Write for ByteBufWriter<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let dest = &mut self.buffer[self.cursor..];
        if dest.len() < s.len() {
            return Err(fmt::Error);
        }
        dest[..s.len()].copy_from_slice(s.as_bytes());
        self.cursor += s.len();
        Ok(())
    }
}

/// Writes to the bytes buffer.
pub fn write_to_slice<'a, T: fmt::Display>(
    buf: &'a mut [u8],
    value: &T,
) -> Result<&'a str, CapacityOverflowError> {
    let mut writer = ByteBufWriter {
        buffer: buf,
        cursor: 0,
    };
    if write!(writer, "{}", value).is_err() {
        return Err(CapacityOverflowError);
    }
    let len = writer.cursor;
    let result = core::str::from_utf8(&buf[..len])
        .expect("[validity] fmt::Display writes valid UTF-8 byte sequence");
    Ok(result)
}

/// Writer that fails (not panics) on OOM.
#[cfg(feature = "alloc")]
struct StringWriter<'a> {
    /// Destination buffer.
    buffer: &'a mut String,
    /// Memory allocation error.
    error: Option<TryReserveError>,
}

#[cfg(feature = "alloc")]
impl fmt::Write for StringWriter<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.error.is_some() {
            return Err(fmt::Error);
        }
        if let Err(e) = self.buffer.try_reserve(s.len()) {
            self.error = Some(e);
            return Err(fmt::Error);
        }
        // This should never fail since `.try_reserve(s.len())` succeeded.
        self.buffer.push_str(s);
        Ok(())
    }
}

/// Appends the data to the string.
///
/// When allocation failure happens, incompletely appended strings won't be
/// stripped. Callers are responsible to clean up the destination if necessary.
#[cfg(feature = "alloc")]
pub fn try_append_to_string<T: fmt::Display>(
    dest: &mut String,
    value: &T,
) -> Result<(), TryReserveError> {
    let mut writer = StringWriter {
        buffer: dest,
        error: None,
    };
    if write!(writer, "{}", value).is_err() {
        let e = writer
            .error
            .expect("[consistency] allocation error should be set on formatting failure");
        return Err(e);
    }
    Ok(())
}

/// Returns true if the two equals after they are converted to strings.
pub(crate) fn eq_str_display<T>(s: &str, d: &T) -> bool
where
    T: ?Sized + fmt::Display,
{
    /// Dummy writer to compare the formatted object to the given string.
    struct CmpWriter<'a>(&'a str);
    impl fmt::Write for CmpWriter<'_> {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            if self.0.len() < s.len() {
                return Err(fmt::Error);
            }
            let (prefix, rest) = self.0.split_at(s.len());
            self.0 = rest;
            if prefix == s {
                Ok(())
            } else {
                Err(fmt::Error)
            }
        }
    }

    let mut writer = CmpWriter(s);
    let succeeded = write!(writer, "{}", d).is_ok();
    succeeded && writer.0.is_empty()
}

/// A debug-printable type to hide the sensitive information.
#[derive(Clone, Copy)]
pub(crate) struct Censored;

impl core::fmt::Debug for Censored {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("{censored}")
    }
}

/// [`ToString`][`alloc::string::ToString`], but without panic.
#[cfg(feature = "alloc")]
pub trait ToStringFallible: alloc::string::ToString {
    /// [`ToString::to_string`][`alloc::string::ToString::to_string`], but without panic on OOM.
    fn try_to_string(&self) -> Result<String, TryReserveError>;
}

#[cfg(feature = "alloc")]
impl<T: fmt::Display> ToStringFallible for T {
    /// [`ToString::to_string`][`alloc::string::ToString::to_string`], but without panic on OOM.
    #[inline]
    fn try_to_string(&self) -> Result<String, TryReserveError> {
        let mut buf = String::new();
        try_append_to_string(&mut buf, self)?;
        Ok(buf)
    }
}

/// A trait for types that can be converted to a dedicated allocated string types.
#[cfg(feature = "alloc")]
pub trait ToDedicatedString {
    /// Conversion target type.
    type Target;

    /// Converts the value to the allocated string.
    fn try_to_dedicated_string(&self) -> Result<Self::Target, TryReserveError>;

    /// Converts the value to the allocated string.
    ///
    /// # Panics
    ///
    /// Panics if memory allocation error occured.
    #[inline]
    #[must_use]
    fn to_dedicated_string(&self) -> Self::Target {
        self.try_to_dedicated_string()
            .expect("failed to allocate enough memory")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eq_str_display_1() {
        assert!(eq_str_display("hello", "hello"));
        assert!(eq_str_display("42", &42));

        assert!(eq_str_display(
            r#"\x00\t\r\n\xff\\"#,
            &b"\x00\t\r\n\xff\\".escape_ascii()
        ));

        assert!(!eq_str_display("hello", "world"));
        assert!(!eq_str_display("hello world", "hello"));
        assert!(!eq_str_display("hello", "hello world"));
        assert!(!eq_str_display("42", &4));
        assert!(!eq_str_display("4", &42));
    }
}
