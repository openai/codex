//! Utilities to safely compare cryptographic values.
//!
//! Extra care must be taken when comparing values in
//! cryptographic code. If done incorrectly, it can lead
//! to a [timing attack](https://en.wikipedia.org/wiki/Timing_attack).
//! By analyzing the time taken to execute parts of a cryptographic
//! algorithm, and attacker can attempt to compromise the
//! cryptosystem.
//!
//! The utilities in this module are designed to be resistant
//! to this type of attack.
//!
//! # Examples
//!
//! To perform a constant-time comparison of two arrays of the same length but different
//! values:
//!
//! ```
//! use openssl::memcmp::eq;
//!
//! // We want to compare `a` to `b` and `c`, without giving
//! // away through timing analysis that `c` is more similar to `a`
//! // than `b`.
//! let a = [0, 0, 0];
//! let b = [1, 1, 1];
//! let c = [0, 0, 1];
//!
//! // These statements will execute in the same amount of time.
//! assert!(!eq(&a, &b));
//! assert!(!eq(&a, &c));
//! ```
use libc::size_t;
use openssl_macros::corresponds;

/// Returns `true` iff `a` and `b` contain the same bytes.
///
/// This operation takes an amount of time dependent on the length of the two
/// arrays given, but is independent of the contents of a and b.
///
/// # Panics
///
/// This function will panic the current task if `a` and `b` do not have the same
/// length.
///
/// # Examples
///
/// To perform a constant-time comparison of two arrays of the same length but different
/// values:
///
/// ```
/// use openssl::memcmp::eq;
///
/// // We want to compare `a` to `b` and `c`, without giving
/// // away through timing analysis that `c` is more similar to `a`
/// // than `b`.
/// let a = [0, 0, 0];
/// let b = [1, 1, 1];
/// let c = [0, 0, 1];
///
/// // These statements will execute in the same amount of time.
/// assert!(!eq(&a, &b));
/// assert!(!eq(&a, &c));
/// ```
#[corresponds(CRYPTO_memcmp)]
pub fn eq(a: &[u8], b: &[u8]) -> bool {
    assert!(a.len() == b.len());
    let ret = unsafe {
        ffi::CRYPTO_memcmp(
            a.as_ptr() as *const _,
            b.as_ptr() as *const _,
            a.len() as size_t,
        )
    };
    ret == 0
}

#[cfg(test)]
mod tests {
    use super::eq;

    #[test]
    fn test_eq() {
        assert!(eq(&[], &[]));
        assert!(eq(&[1], &[1]));
        assert!(!eq(&[1, 2, 3], &[1, 2, 4]));
    }

    #[test]
    #[should_panic]
    fn test_diff_lens() {
        eq(&[], &[1]);
    }
}
