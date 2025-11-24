//! FIPS 140-2 support.
//!
//! See [OpenSSL's documentation] for details.
//!
//! [OpenSSL's documentation]: https://www.openssl.org/docs/fips/UserGuide-2.0.pdf
use crate::cvt;
use crate::error::ErrorStack;
use openssl_macros::corresponds;

/// Moves the library into or out of the FIPS 140-2 mode of operation.
#[corresponds(FIPS_mode_set)]
pub fn enable(enabled: bool) -> Result<(), ErrorStack> {
    ffi::init();
    unsafe { cvt(ffi::FIPS_mode_set(enabled as _)).map(|_| ()) }
}

/// Determines if the library is running in the FIPS 140-2 mode of operation.
#[corresponds(FIPS_mode)]
pub fn enabled() -> bool {
    unsafe { ffi::FIPS_mode() != 0 }
}
