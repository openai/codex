// Copyright 2015-2016 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

//! Cryptographic pseudo-random number generation.
//!
//! *ring* functions that generate random bytes take a `&dyn SecureRandom`
//! parameter to make it clear which functions are non-deterministic.

use crate::error;

/// A secure random number generator.
pub trait SecureRandom: sealed::SecureRandom {
    /// Fills `dest` with random bytes.
    fn fill(&self, dest: &mut [u8]) -> Result<(), error::Unspecified>;
}

impl<T> SecureRandom for T
where
    T: sealed::SecureRandom,
{
    #[inline(always)]
    fn fill(&self, dest: &mut [u8]) -> Result<(), error::Unspecified> {
        self.fill_impl(dest)
    }
}

/// A random value constructed from a `SecureRandom` that hasn't been exposed
/// through any safe Rust interface.
///
/// Intentionally does not implement any traits other than `Sized`.
pub struct Random<T: RandomlyConstructable>(T);

impl<T: RandomlyConstructable> Random<T> {
    /// Expose the random value.
    #[inline]
    pub fn expose(self) -> T {
        self.0
    }
}

/// Generate the new random value using `rng`.
#[inline]
pub fn generate<T: RandomlyConstructable>(
    rng: &dyn SecureRandom,
) -> Result<Random<T>, error::Unspecified> {
    let mut r = T::zero();
    rng.fill(r.as_mut_bytes())?;
    Ok(Random(r))
}

pub(crate) mod sealed {
    use crate::error;

    pub trait SecureRandom: core::fmt::Debug {
        /// Fills `dest` with random bytes.
        fn fill_impl(&self, dest: &mut [u8]) -> Result<(), error::Unspecified>;
    }

    pub trait RandomlyConstructable: Sized {
        fn zero() -> Self; // `Default::default()`
        fn as_mut_bytes(&mut self) -> &mut [u8]; // `AsMut<[u8]>::as_mut`
    }

    impl<const N: usize> RandomlyConstructable for [u8; N] {
        #[inline]
        fn zero() -> Self {
            [0; N]
        }

        #[inline]
        fn as_mut_bytes(&mut self) -> &mut [u8] {
            &mut self[..]
        }
    }
}

/// A type that can be returned by `ring::rand::generate()`.
pub trait RandomlyConstructable: sealed::RandomlyConstructable {}
impl<T> RandomlyConstructable for T where T: sealed::RandomlyConstructable {}

/// A secure random number generator where the random values come directly
/// from the operating system.
///
/// "Directly from the operating system" here presently means "whatever the
/// `getrandom` crate does" but that may change in the future. That roughly
/// means calling libc's `getrandom` function or whatever is analogous to that;
/// see the `getrandom` crate's documentation for more info.
///
/// A single `SystemRandom` may be shared across multiple threads safely.
///
/// `new()` is guaranteed to always succeed and to have low latency; it won't
/// try to open or read from a file or do similar things. The first call to
/// `fill()` may block a substantial amount of time since any and all
/// initialization is deferred to it. Therefore, it may be a good idea to call
/// `fill()` once at a non-latency-sensitive time to minimize latency for
/// future calls.
#[derive(Clone, Debug)]
pub struct SystemRandom(());

impl SystemRandom {
    /// Constructs a new `SystemRandom`.
    #[inline(always)]
    pub fn new() -> Self {
        Self(())
    }
}

impl crate::sealed::Sealed for SystemRandom {}

// Use the `getrandom` crate whenever it is using the environment's (operating
// system's) CSPRNG. Avoid using it on targets where it uses the `rdrand`
// implementation.
#[cfg(any(
    all(feature = "less-safe-getrandom-custom-or-rdrand", target_os = "none"),
    all(feature = "less-safe-getrandom-espidf", target_os = "espidf"),
    target_os = "aix",
    target_os = "android",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "haiku",
    target_os = "hermit",
    target_os = "hurd",
    target_os = "horizon",
    target_os = "illumos",
    target_os = "linux",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "redox",
    target_os = "solaris",
    target_os = "vita",
    target_os = "windows",
    all(
        target_vendor = "apple",
        any(
            target_os = "ios",
            target_os = "macos",
            target_os = "tvos",
            target_os = "visionos",
            target_os = "watchos",
        )
    ),
    all(
        target_arch = "wasm32",
        any(
            target_os = "wasi",
            all(target_os = "unknown", feature = "wasm32_unknown_unknown_js")
        )
    ),
))]
impl sealed::SecureRandom for SystemRandom {
    #[inline(always)]
    fn fill_impl(&self, dest: &mut [u8]) -> Result<(), error::Unspecified> {
        getrandom::getrandom(dest).map_err(|_| error::Unspecified)
    }
}
