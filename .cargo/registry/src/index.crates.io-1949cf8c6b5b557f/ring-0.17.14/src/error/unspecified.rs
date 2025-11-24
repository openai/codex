// Copyright 2016-2024 Brian Smith.
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

#[cfg(feature = "std")]
extern crate std;

/// An error with absolutely no details.
///
/// *ring* uses this unit type as the error type in most of its results
/// because (a) usually the specific reasons for a failure are obvious or are
/// not useful to know, and/or (b) providing more details about a failure might
/// provide a dangerous side channel, and/or (c) it greatly simplifies the
/// error handling logic.
///
/// `Result<T, ring::error::Unspecified>` is mostly equivalent to
/// `Result<T, ()>`. However, `ring::error::Unspecified` implements
/// [`std::error::Error`] and users of *ring* can implement
/// `From<ring::error::Unspecified>` to map this to their own error types, as
/// described in [“Error Handling” in the Rust Book]:
///
/// ```
/// use ring::rand::{self, SecureRandom};
///
/// enum Error {
///     CryptoError,
///
/// #  #[cfg(feature = "alloc")]
///     IOError(std::io::Error),
///     // [...]
/// }
///
/// impl From<ring::error::Unspecified> for Error {
///     fn from(_: ring::error::Unspecified) -> Self { Error::CryptoError }
/// }
///
/// fn eight_random_bytes() -> Result<[u8; 8], Error> {
///     let rng = rand::SystemRandom::new();
///     let mut bytes = [0; 8];
///
///     // The `From<ring::error::Unspecified>` implementation above makes this
///     // equivalent to
///     // `rng.fill(&mut bytes).map_err(|_| Error::CryptoError)?`.
///     rng.fill(&mut bytes)?;
///
///     Ok(bytes)
/// }
///
/// assert!(eight_random_bytes().is_ok());
/// ```
///
/// Experience with using and implementing other crypto libraries like has
/// shown that sophisticated error reporting facilities often cause significant
/// bugs themselves, both within the crypto library and within users of the
/// crypto library. This approach attempts to minimize complexity in the hopes
/// of avoiding such problems. In some cases, this approach may be too extreme,
/// and it may be important for an operation to provide some details about the
/// cause of a failure. Users of *ring* are encouraged to report such cases so
/// that they can be addressed individually.
///
/// [`std::error::Error`]: https://doc.rust-lang.org/std/error/trait.Error.html
/// [“Error Handling” in the Rust Book]:
///     https://doc.rust-lang.org/book/first-edition/error-handling.html#the-from-trait
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Unspecified;

// This is required for the implementation of `std::error::Error`.
impl core::fmt::Display for Unspecified {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.write_str("ring::error::Unspecified")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Unspecified {}
