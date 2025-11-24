//! The `basic` API flavor provides barebones UTF-8 checking at the highest speed.
//!
//! It is fastest on valid UTF-8, but only checks for errors after processing the whole byte sequence
//! and does not provide detailed information if the data is not valid UTF-8. [`Utf8Error`] is a zero-sized error struct.
//!
//! If you need detailed error information use the functions from the [`crate::compat`] module instead.

use core::str::{from_utf8_unchecked, from_utf8_unchecked_mut};

use crate::implementation::validate_utf8_basic;

/// Simple zero-sized UTF-8 error.
///
/// No information is provided where the error occurred or how long the invalid byte
/// byte sequence is.
#[derive(Copy, Eq, PartialEq, Clone, Debug)]
pub struct Utf8Error;

impl core::fmt::Display for Utf8Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("invalid utf-8 sequence")
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
/// Will return the zero-sized Err([`Utf8Error`]) on if the input contains invalid UTF-8.
#[inline]
pub fn from_utf8(input: &[u8]) -> Result<&str, Utf8Error> {
    unsafe {
        validate_utf8_basic(input)?;
        Ok(from_utf8_unchecked(input))
    }
}

/// Analogue to [`std::str::from_utf8_mut()`].
///
/// Checks if the passed mutable byte sequence is valid UTF-8 and returns a mutable
/// [`std::str`] reference to the passed byte slice wrapped in `Ok()` if it is.
///
/// # Errors
/// Will return the zero-sized Err([`Utf8Error`]) on if the input contains invalid UTF-8.
#[inline]
pub fn from_utf8_mut(input: &mut [u8]) -> Result<&mut str, Utf8Error> {
    unsafe {
        validate_utf8_basic(input)?;
        Ok(from_utf8_unchecked_mut(input))
    }
}

/// Allows direct access to the platform-specific unsafe validation implementations.
#[cfg(feature = "public_imp")]
pub mod imp {
    use crate::basic;

    /// A low-level interface for streaming validation of UTF-8 data. It is meant to be integrated
    /// in high-performance data processing pipelines.
    ///
    /// Data can be streamed in arbitrarily-sized chunks using the [`Self::update()`] method. There is
    /// no way to find out if the input so far was valid UTF-8 during the validation. Only when
    /// the validation is completed with the [`Self::finalize()`] method the result of the validation is
    /// returned. Use [`ChunkedUtf8Validator`] if possible for highest performance.
    ///
    /// This implementation requires CPU SIMD features specified by the module it resides in.
    /// It is undefined behavior to use it if the required CPU features are not available which
    /// is why all trait methods are `unsafe`.
    ///
    /// General usage:
    /// ```rust
    /// use simdutf8::basic::imp::Utf8Validator;
    /// use std::io::{stdin, Read, Result};
    ///
    /// # #[cfg(target_arch = "x86_64")]
    /// fn main() -> Result<()> {
    ///     unsafe {
    ///         if !std::is_x86_feature_detected!("avx2") {
    ///             panic!("This example only works with CPUs supporting AVX 2");
    ///         }
    ///
    ///         let mut validator = simdutf8::basic::imp::x86::avx2::Utf8ValidatorImp::new();
    ///         let mut buf = vec![0; 8192];
    ///         loop {
    ///             let bytes_read = stdin().read(buf.as_mut())?;
    ///             if bytes_read == 0 {
    ///                 break;
    ///             }
    ///             validator.update(&buf);
    ///         }
    ///
    ///         if validator.finalize().is_ok() {
    ///             println!("Input is valid UTF-8");
    ///         } else {
    ///             println!("Input is not valid UTF-8");
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    ///
    /// # #[cfg(not(target_arch = "x86_64"))]
    /// # fn main() { }
    /// ```
    ///
    pub trait Utf8Validator {
        /// Creates a new validator.
        ///
        /// # Safety
        /// This implementation requires CPU SIMD features specified by the module it resides in.
        /// It is undefined behavior to call it if the required CPU features are not available.
        #[must_use]
        unsafe fn new() -> Self
        where
            Self: Sized;

        /// Updates the validator with `input`.
        ///
        /// # Safety
        /// This implementation requires CPU SIMD features specified by the module it resides in.
        /// It is undefined behavior to call it if the required CPU features are not available.
        unsafe fn update(&mut self, input: &[u8]);

        /// Finishes the validation and returns `Ok(())` if the input was valid UTF-8.
        ///
        /// # Errors
        /// A [`basic::Utf8Error`] is returned if the input was not valid UTF-8. No
        /// further information about the location of the error is provided.
        ///
        /// # Safety
        /// This implementation requires CPU SIMD features specified by the module it resides in.
        /// It is undefined behavior to call it if the required CPU features are not available.
        unsafe fn finalize(self) -> core::result::Result<(), basic::Utf8Error>;
    }

    /// Like [`Utf8Validator`] this low-level API is for streaming validation of UTF-8 data.
    ///
    /// It has additional restrictions imposed on how the input is passed in to allow
    /// validation with as little overhead as possible.
    ///
    /// To feed it data you need to call the [`Self::update_from_chunks()`] method which takes slices which
    /// have to be a multiple of 64 bytes long. The method will panic otherwise.  There is
    /// no way to find out if the input so far was valid UTF-8 during the validation. Only when
    /// the validation is completed with the [`Self::finalize()`] method the result of the validation is
    /// returned.
    ///
    /// The `Self::finalize()` method can be fed the rest of the data. There is no restriction on the
    /// data passed to it.
    ///
    /// This implementation requires CPU SIMD features specified by the module it resides in.
    /// It is undefined behavior to use it if the required CPU features are not available which
    /// is why all trait methods are `unsafe`.
    pub trait ChunkedUtf8Validator {
        /// Creates a new validator.
        ///
        /// # Safety
        /// This implementation requires CPU SIMD features specified by the module it resides in.
        /// It is undefined behavior to call it if the required CPU features are not available.
        #[must_use]
        unsafe fn new() -> Self
        where
            Self: Sized;

        /// Updates the validator with `input`.
        ///
        /// # Panics
        /// If `input.len()` is not a multiple of 64.
        ///
        /// # Safety
        /// This implementation requires CPU SIMD features specified by the module it resides in.
        /// It is undefined behavior to call it if the required CPU features are not available.
        unsafe fn update_from_chunks(&mut self, input: &[u8]);

        /// Updates the validator with remaining input if any. There is no restriction on the
        /// data provided.
        ///
        /// Finishes the validation and returns `Ok(())` if the input was valid UTF-8.
        ///
        /// # Errors
        /// A [`basic::Utf8Error`] is returned if the input was not valid UTF-8. No
        /// further information about the location of the error is provided.
        ///
        /// # Safety
        /// This implementation requires CPU SIMD features specified by the module it resides in.
        /// It is undefined behavior to call it if the required CPU features are not available.
        unsafe fn finalize(
            self,
            remaining_input: core::option::Option<&[u8]>,
        ) -> core::result::Result<(), basic::Utf8Error>;
    }

    /// Includes the x86/x86-64 SIMD implementations.
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub mod x86 {
        /// Includes the validation implementation for AVX 2-compatible CPUs.
        ///
        /// Using the provided functionality on CPUs which do not support AVX 2 is undefined
        /// behavior and will very likely cause a crash.
        pub mod avx2 {
            pub use crate::implementation::x86::avx2::validate_utf8_basic as validate_utf8;
            pub use crate::implementation::x86::avx2::ChunkedUtf8ValidatorImp;
            pub use crate::implementation::x86::avx2::Utf8ValidatorImp;
        }
        /// Includes the validation implementation for SSE 4.2-compatible CPUs.
        ///
        /// Using the provided functionality on CPUs which do not support SSE 4.2 is undefined
        /// behavior and will very likely cause a crash.
        pub mod sse42 {
            pub use crate::implementation::x86::sse42::validate_utf8_basic as validate_utf8;
            pub use crate::implementation::x86::sse42::ChunkedUtf8ValidatorImp;
            pub use crate::implementation::x86::sse42::Utf8ValidatorImp;
        }
    }

    /// Includes the aarch64 SIMD implementations.
    #[cfg(all(feature = "aarch64_neon", target_arch = "aarch64"))]
    pub mod aarch64 {
        /// Includes the Neon-based validation implementation for aarch64 CPUs.
        ///
        /// Should be supported on all ARM64 CPUSs. If it is not supported by the operating
        /// system using it is undefined behavior and will likely cause a crash.
        pub mod neon {
            pub use crate::implementation::aarch64::neon::validate_utf8_basic as validate_utf8;
            pub use crate::implementation::aarch64::neon::ChunkedUtf8ValidatorImp;
            pub use crate::implementation::aarch64::neon::Utf8ValidatorImp;
        }
    }

    /// Includes the wasm32 SIMD implementations.
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    pub mod wasm32 {
        /// Includes the simd128-based validation implementation for WASM runtimes.
        ///
        /// Using the provided functionality on WASM runtimes that do not support SIMD
        /// instructions will likely cause a crash.
        pub mod simd128 {
            pub use crate::implementation::wasm32::simd128::validate_utf8_basic as validate_utf8;
            pub use crate::implementation::wasm32::simd128::ChunkedUtf8ValidatorImp;
            pub use crate::implementation::wasm32::simd128::Utf8ValidatorImp;
        }
    }
}
