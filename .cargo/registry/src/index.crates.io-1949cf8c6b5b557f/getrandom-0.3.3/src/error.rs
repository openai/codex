#[cfg(feature = "std")]
extern crate std;

use core::fmt;

// This private alias mirrors `std::io::RawOsError`:
// https://doc.rust-lang.org/std/io/type.RawOsError.html)
cfg_if::cfg_if!(
    if #[cfg(target_os = "uefi")] {
        // See the UEFI spec for more information:
        // https://uefi.org/specs/UEFI/2.10/Apx_D_Status_Codes.html
        type RawOsError = usize;
        type NonZeroRawOsError = core::num::NonZeroUsize;
        const UEFI_ERROR_FLAG: RawOsError = 1 << (RawOsError::BITS - 1);
    } else {
        type RawOsError = i32;
        type NonZeroRawOsError = core::num::NonZeroI32;
    }
);

/// A small and `no_std` compatible error type
///
/// The [`Error::raw_os_error()`] will indicate if the error is from the OS, and
/// if so, which error code the OS gave the application. If such an error is
/// encountered, please consult with your system documentation.
///
/// *If this crate's `"std"` Cargo feature is enabled*, then:
/// - [`getrandom::Error`][Error] implements
///   [`std::error::Error`](https://doc.rust-lang.org/std/error/trait.Error.html)
/// - [`std::io::Error`](https://doc.rust-lang.org/std/io/struct.Error.html) implements
///   [`From<getrandom::Error>`](https://doc.rust-lang.org/std/convert/trait.From.html).

// note: on non-UEFI targets OS errors are represented as negative integers,
// while on UEFI targets OS errors have the highest bit set to 1.
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Error(NonZeroRawOsError);

impl Error {
    /// This target/platform is not supported by `getrandom`.
    pub const UNSUPPORTED: Error = Self::new_internal(0);
    /// The platform-specific `errno` returned a non-positive value.
    pub const ERRNO_NOT_POSITIVE: Error = Self::new_internal(1);
    /// Encountered an unexpected situation which should not happen in practice.
    pub const UNEXPECTED: Error = Self::new_internal(2);

    /// Internal errors can be in the range of 2^16..2^17
    const INTERNAL_START: RawOsError = 1 << 16;
    /// Custom errors can be in the range of 2^17..(2^17 + 2^16)
    const CUSTOM_START: RawOsError = 1 << 17;

    /// Creates a new instance of an `Error` from a negative error code.
    #[cfg(not(target_os = "uefi"))]
    #[allow(dead_code)]
    pub(super) fn from_neg_error_code(code: RawOsError) -> Self {
        if code < 0 {
            let code = NonZeroRawOsError::new(code).expect("`code` is negative");
            Self(code)
        } else {
            Error::UNEXPECTED
        }
    }

    /// Creates a new instance of an `Error` from an UEFI error code.
    #[cfg(target_os = "uefi")]
    #[allow(dead_code)]
    pub(super) fn from_uefi_code(code: RawOsError) -> Self {
        if code & UEFI_ERROR_FLAG != 0 {
            let code = NonZeroRawOsError::new(code).expect("The highest bit of `code` is set to 1");
            Self(code)
        } else {
            Self::UNEXPECTED
        }
    }

    /// Extract the raw OS error code (if this error came from the OS)
    ///
    /// This method is identical to [`std::io::Error::raw_os_error()`][1], except
    /// that it works in `no_std` contexts. On most targets this method returns
    /// `Option<i32>`, but some platforms (e.g. UEFI) may use a different primitive
    /// type like `usize`. Consult with the [`RawOsError`] docs for more information.
    ///
    /// If this method returns `None`, the error value can still be formatted via
    /// the `Display` implementation.
    ///
    /// [1]: https://doc.rust-lang.org/std/io/struct.Error.html#method.raw_os_error
    /// [`RawOsError`]: https://doc.rust-lang.org/std/io/type.RawOsError.html
    #[inline]
    pub fn raw_os_error(self) -> Option<RawOsError> {
        let code = self.0.get();

        // note: in this method we need to cover only backends which rely on
        // `Error::{from_error_code, from_errno, from_uefi_code}` methods,
        // on all other backends this method always returns `None`.

        #[cfg(target_os = "uefi")]
        {
            if code & UEFI_ERROR_FLAG != 0 {
                Some(code)
            } else {
                None
            }
        }

        #[cfg(not(target_os = "uefi"))]
        {
            // On most targets `std` expects positive error codes while retrieving error strings:
            // - `libc`-based targets use `strerror_r` which expects positive error codes.
            // - Hermit relies on the `hermit-abi` crate, which expects positive error codes:
            //   https://docs.rs/hermit-abi/0.4.0/src/hermit_abi/errno.rs.html#400-532
            // - WASIp1 uses the same conventions as `libc`:
            //   https://github.com/rust-lang/rust/blob/1.85.0/library/std/src/sys/pal/wasi/os.rs#L57-L67
            //
            // The only exception is Solid, `std` expects negative system error codes, see:
            // https://github.com/rust-lang/rust/blob/1.85.0/library/std/src/sys/pal/solid/error.rs#L5-L31
            if code >= 0 {
                None
            } else if cfg!(not(target_os = "solid_asp3")) {
                code.checked_neg()
            } else {
                Some(code)
            }
        }
    }

    /// Creates a new instance of an `Error` from a particular custom error code.
    pub const fn new_custom(n: u16) -> Error {
        // SAFETY: code > 0 as CUSTOM_START > 0 and adding `n` won't overflow `RawOsError`.
        let code = Error::CUSTOM_START + (n as RawOsError);
        Error(unsafe { NonZeroRawOsError::new_unchecked(code) })
    }

    /// Creates a new instance of an `Error` from a particular internal error code.
    pub(crate) const fn new_internal(n: u16) -> Error {
        // SAFETY: code > 0 as INTERNAL_START > 0 and adding `n` won't overflow `RawOsError`.
        let code = Error::INTERNAL_START + (n as RawOsError);
        Error(unsafe { NonZeroRawOsError::new_unchecked(code) })
    }

    fn internal_desc(&self) -> Option<&'static str> {
        let desc = match *self {
            Error::UNSUPPORTED => "getrandom: this target is not supported",
            Error::ERRNO_NOT_POSITIVE => "errno: did not return a positive value",
            Error::UNEXPECTED => "unexpected situation",
            #[cfg(any(
                target_os = "ios",
                target_os = "visionos",
                target_os = "watchos",
                target_os = "tvos",
            ))]
            Error::IOS_RANDOM_GEN => "SecRandomCopyBytes: iOS Security framework failure",
            #[cfg(all(windows, target_vendor = "win7"))]
            Error::WINDOWS_RTL_GEN_RANDOM => "RtlGenRandom: Windows system function failure",
            #[cfg(all(feature = "wasm_js", getrandom_backend = "wasm_js"))]
            Error::WEB_CRYPTO => "Web Crypto API is unavailable",
            #[cfg(target_os = "vxworks")]
            Error::VXWORKS_RAND_SECURE => "randSecure: VxWorks RNG module is not initialized",

            #[cfg(any(
                getrandom_backend = "rdrand",
                all(target_arch = "x86_64", target_env = "sgx")
            ))]
            Error::FAILED_RDRAND => "RDRAND: failed multiple times: CPU issue likely",
            #[cfg(any(
                getrandom_backend = "rdrand",
                all(target_arch = "x86_64", target_env = "sgx")
            ))]
            Error::NO_RDRAND => "RDRAND: instruction not supported",

            #[cfg(getrandom_backend = "rndr")]
            Error::RNDR_FAILURE => "RNDR: Could not generate a random number",
            #[cfg(getrandom_backend = "rndr")]
            Error::RNDR_NOT_AVAILABLE => "RNDR: Register not supported",
            _ => return None,
        };
        Some(desc)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = f.debug_struct("Error");
        if let Some(errno) = self.raw_os_error() {
            dbg.field("os_error", &errno);
            #[cfg(feature = "std")]
            dbg.field("description", &std::io::Error::from_raw_os_error(errno));
        } else if let Some(desc) = self.internal_desc() {
            dbg.field("internal_code", &self.0.get());
            dbg.field("description", &desc);
        } else {
            dbg.field("unknown_code", &self.0.get());
        }
        dbg.finish()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(errno) = self.raw_os_error() {
            cfg_if! {
                if #[cfg(feature = "std")] {
                    std::io::Error::from_raw_os_error(errno).fmt(f)
                } else {
                    write!(f, "OS Error: {}", errno)
                }
            }
        } else if let Some(desc) = self.internal_desc() {
            f.write_str(desc)
        } else {
            write!(f, "Unknown Error: {}", self.0.get())
        }
    }
}
