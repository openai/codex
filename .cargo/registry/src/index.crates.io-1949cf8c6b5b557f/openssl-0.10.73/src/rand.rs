//! Utilities for secure random number generation.
//!
//! # Examples
//!
//! To generate a buffer with cryptographically strong bytes:
//!
//! ```
//! use openssl::rand::rand_bytes;
//!
//! let mut buf = [0; 256];
//! rand_bytes(&mut buf).unwrap();
//! ```
use libc::c_int;

use crate::error::ErrorStack;
use crate::{cvt, LenType};
use openssl_macros::corresponds;

/// Fill buffer with cryptographically strong pseudo-random bytes.
///
/// # Examples
///
/// To generate a buffer with cryptographically strong random bytes:
///
/// ```
/// use openssl::rand::rand_bytes;
///
/// let mut buf = [0; 256];
/// rand_bytes(&mut buf).unwrap();
/// ```
#[corresponds(RAND_bytes)]
pub fn rand_bytes(buf: &mut [u8]) -> Result<(), ErrorStack> {
    unsafe {
        ffi::init();
        assert!(buf.len() <= c_int::MAX as usize);
        cvt(ffi::RAND_bytes(buf.as_mut_ptr(), buf.len() as LenType)).map(|_| ())
    }
}

/// Fill buffer with cryptographically strong pseudo-random bytes. It is
/// intended to be used for generating values that should remain private.
///
/// # Examples
///
/// To generate a buffer with cryptographically strong random bytes:
///
/// ```
/// use openssl::rand::rand_priv_bytes;
///
/// let mut buf = [0; 256];
/// rand_priv_bytes(&mut buf).unwrap();
/// ```
///
/// Requires OpenSSL 1.1.1 or newer.
#[corresponds(RAND_priv_bytes)]
#[cfg(ossl111)]
pub fn rand_priv_bytes(buf: &mut [u8]) -> Result<(), ErrorStack> {
    unsafe {
        ffi::init();
        assert!(buf.len() <= c_int::MAX as usize);
        cvt(ffi::RAND_priv_bytes(buf.as_mut_ptr(), buf.len() as LenType)).map(|_| ())
    }
}

/// Controls random device file descriptor behavior.
///
/// Requires OpenSSL 1.1.1 or newer.
#[corresponds(RAND_keep_random_devices_open)]
#[cfg(ossl111)]
pub fn keep_random_devices_open(keep: bool) {
    unsafe {
        ffi::RAND_keep_random_devices_open(keep as LenType);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_rand_bytes() {
        let mut buf = [0; 32];
        super::rand_bytes(&mut buf).unwrap();
    }

    #[test]
    #[cfg(ossl111)]
    fn test_rand_priv_bytes() {
        let mut buf = [0; 32];
        super::rand_priv_bytes(&mut buf).unwrap();
    }
}
