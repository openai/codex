//! The `compat` API flavor provides full compatibility with [`std::str::from_utf8()`] and detailed validation errors.
//!
//! In particular, [`from_utf8()`]
//! returns an [`Utf8Error`], which has the [`valid_up_to()`](Utf8Error#method.valid_up_to) and
//! [`error_len()`](Utf8Error#method.error_len) methods. The first is useful for verification of streamed data. The
//! second is useful e.g. for replacing invalid byte sequences with a replacement character.
//!
//! The functions in this module also fail early: errors are checked on-the-fly as the string is processed and once
//! an invalid UTF-8 sequence is encountered, it returns without processing the rest of the data.
//! This comes at a slight performance penalty compared to the [`crate::basic`] module if the input is valid UTF-8.

use core::fmt::Display;
use core::fmt::Formatter;

use core::str::{from_utf8_unchecked, from_utf8_unchecked_mut};

use crate::implementation::validate_utf8_compat;

/// UTF-8 error information compatible with [`std::str::Utf8Error`].
///
/// Contains information on the location of the encountered validation error and the length of the
/// invalid UTF-8 sequence.
#[derive(Copy, Eq, PartialEq, Clone, Debug)]
pub struct Utf8Error {
    pub(crate) valid_up_to: usize,
    pub(crate) error_len: Option<u8>,
}

impl Utf8Error {
    /// Analogue to [`std::str::Utf8Error::valid_up_to()`](std::str::Utf8Error#method.valid_up_to).
    ///
    /// ...
    #[inline]
    #[must_use]
    pub fn valid_up_to(&self) -> usize {
        self.valid_up_to
    }

    /// Analogue to [`std::str::Utf8Error::error_len()`](std::str::Utf8Error#method.error_len).
    ///
    /// ...
    #[inline]
    #[must_use]
    pub fn error_len(&self) -> Option<usize> {
        self.error_len.map(|len| len as usize)
    }
}

impl Display for Utf8Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        if let Some(error_len) = self.error_len {
            write!(
                f,
                "invalid utf-8 sequence of {} bytes from index {}",
                error_len, self.valid_up_to
            )
        } else {
            write!(
                f,
                "incomplete utf-8 byte sequence from index {}",
                self.valid_up_to
            )
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Utf8Error {}

/// Analogue to [`std::str::from_utf8()`].
///
/// Checks if the passed byte sequence is valid UTF-8 and returns an
/// [`std::str`] reference to the passed byte slice wrapped in `Ok()` if it is.
///
/// # Errors
/// Will return Err([`Utf8Error`]) on if the input contains invalid UTF-8 with
/// detailed error information.
#[inline]
pub fn from_utf8(input: &[u8]) -> Result<&str, Utf8Error> {
    unsafe {
        validate_utf8_compat(input)?;
        Ok(from_utf8_unchecked(input))
    }
}

/// Analogue to [`std::str::from_utf8_mut()`].
///
/// Checks if the passed mutable byte sequence is valid UTF-8 and returns a mutable
/// [`std::str`] reference to the passed byte slice wrapped in `Ok()` if it is.
///
/// # Errors
/// Will return Err([`Utf8Error`]) on if the input contains invalid UTF-8 with
/// detailed error information.
#[inline]
pub fn from_utf8_mut(input: &mut [u8]) -> Result<&mut str, Utf8Error> {
    unsafe {
        validate_utf8_compat(input)?;
        Ok(from_utf8_unchecked_mut(input))
    }
}

/// Allows direct access to the platform-specific unsafe validation implementations.
#[cfg(feature = "public_imp")]
pub mod imp {
    /// Includes the x86/x86-64 SIMD implementations.
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub mod x86 {
        /// Includes the validation implementation for AVX 2-compatible CPUs.
        pub mod avx2 {
            pub use crate::implementation::x86::avx2::validate_utf8_compat as validate_utf8;
        }
        /// Includes the validation implementation for SSE 4.2-compatible CPUs.
        pub mod sse42 {
            pub use crate::implementation::x86::sse42::validate_utf8_compat as validate_utf8;
        }
    }

    /// Includes the aarch64 SIMD implementations.
    #[cfg(all(feature = "aarch64_neon", target_arch = "aarch64"))]
    pub mod aarch64 {
        /// Includes the validation implementation for Neon SIMD.
        pub mod neon {
            pub use crate::implementation::aarch64::neon::validate_utf8_compat as validate_utf8;
        }
    }

    /// Includes the wasm32 SIMD implementations.
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    pub mod wasm32 {
        /// Includes the validation implementation for WASM simd128.
        pub mod simd128 {
            pub use crate::implementation::wasm32::simd128::validate_utf8_compat as validate_utf8;
        }
    }
}
