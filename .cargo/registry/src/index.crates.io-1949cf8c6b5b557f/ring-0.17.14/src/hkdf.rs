// Copyright 2015 Brian Smith.
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

//! HMAC-based Extract-and-Expand Key Derivation Function.
//!
//! HKDF is specified in [RFC 5869].
//!
//! [RFC 5869]: https://tools.ietf.org/html/rfc5869

use crate::{error, hmac};

/// An HKDF algorithm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Algorithm(hmac::Algorithm);

impl Algorithm {
    /// The underlying HMAC algorithm.
    #[inline]
    pub fn hmac_algorithm(&self) -> hmac::Algorithm {
        self.0
    }
}

/// HKDF using HMAC-SHA-1. Obsolete.
pub static HKDF_SHA1_FOR_LEGACY_USE_ONLY: Algorithm =
    Algorithm(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY);

/// HKDF using HMAC-SHA-256.
pub static HKDF_SHA256: Algorithm = Algorithm(hmac::HMAC_SHA256);

/// HKDF using HMAC-SHA-384.
pub static HKDF_SHA384: Algorithm = Algorithm(hmac::HMAC_SHA384);

/// HKDF using HMAC-SHA-512.
pub static HKDF_SHA512: Algorithm = Algorithm(hmac::HMAC_SHA512);

impl KeyType for Algorithm {
    fn len(&self) -> usize {
        self.0.digest_algorithm().output_len()
    }
}

/// A salt for HKDF operations.
#[derive(Debug)]
pub struct Salt(hmac::Key);

impl Salt {
    /// Constructs a new `Salt` with the given value based on the given digest
    /// algorithm.
    ///
    /// Constructing a `Salt` is relatively expensive so it is good to reuse a
    /// `Salt` object instead of re-constructing `Salt`s with the same value.
    pub fn new(algorithm: Algorithm, value: &[u8]) -> Self {
        Self(hmac::Key::new(algorithm.0, value))
    }

    /// The [HKDF-Extract] operation.
    ///
    /// [HKDF-Extract]: https://tools.ietf.org/html/rfc5869#section-2.2
    pub fn extract(&self, secret: &[u8]) -> Prk {
        // The spec says that if no salt is provided then a key of
        // `digest_alg.output_len` bytes of zeros is used. But, HMAC keys are
        // already zero-padded to the block length, which is larger than the output
        // length of the extract step (the length of the digest). Consequently the
        // `Key` constructor will automatically do the right thing for a
        // zero-length string.
        let salt = &self.0;
        let prk = hmac::sign(salt, secret);
        Prk(hmac::Key::new(salt.algorithm(), prk.as_ref()))
    }

    /// The algorithm used to derive this salt.
    #[inline]
    pub fn algorithm(&self) -> Algorithm {
        Algorithm(self.0.algorithm())
    }
}

impl From<Okm<'_, Algorithm>> for Salt {
    fn from(okm: Okm<'_, Algorithm>) -> Self {
        Self(hmac::Key::from(Okm {
            prk: okm.prk,
            info: okm.info,
            len: okm.len().0,
            len_cached: okm.len_cached,
        }))
    }
}

/// The length of the OKM (Output Keying Material) for a `Prk::expand()` call.
pub trait KeyType {
    /// The length that `Prk::expand()` should expand its input to.
    fn len(&self) -> usize;
}

/// A HKDF PRK (pseudorandom key).
#[derive(Clone, Debug)]
pub struct Prk(hmac::Key);

impl Prk {
    /// Construct a new `Prk` directly with the given value.
    ///
    /// Usually one can avoid using this. It is useful when the application
    /// intentionally wants to leak the PRK secret, e.g. to implement
    /// `SSLKEYLOGFILE` functionality.
    pub fn new_less_safe(algorithm: Algorithm, value: &[u8]) -> Self {
        Self(hmac::Key::new(algorithm.hmac_algorithm(), value))
    }

    /// The [HKDF-Expand] operation.
    ///
    /// [HKDF-Expand]: https://tools.ietf.org/html/rfc5869#section-2.3
    ///
    /// Fails if (and only if) `len` is too large.
    #[inline]
    pub fn expand<'a, L: KeyType>(
        &'a self,
        info: &'a [&'a [u8]],
        len: L,
    ) -> Result<Okm<'a, L>, error::Unspecified> {
        let len_cached = len.len();
        if len_cached > 255 * self.0.algorithm().digest_algorithm().output_len() {
            return Err(error::Unspecified);
        }
        Ok(Okm {
            prk: self,
            info,
            len,
            len_cached,
        })
    }
}

impl From<Okm<'_, Algorithm>> for Prk {
    fn from(okm: Okm<Algorithm>) -> Self {
        Self(hmac::Key::from(Okm {
            prk: okm.prk,
            info: okm.info,
            len: okm.len().0,
            len_cached: okm.len_cached,
        }))
    }
}

/// An HKDF OKM (Output Keying Material)
///
/// Intentionally not `Clone` or `Copy` as an OKM is generally only safe to
/// use once.
#[derive(Debug)]
pub struct Okm<'a, L: KeyType> {
    prk: &'a Prk,
    info: &'a [&'a [u8]],
    len: L,
    len_cached: usize,
}

impl<L: KeyType> Okm<'_, L> {
    /// The `OkmLength` given to `Prk::expand()`.
    #[inline]
    pub fn len(&self) -> &L {
        &self.len
    }

    /// Fills `out` with the output of the HKDF-Expand operation for the given
    /// inputs.
    ///
    /// Fails if (and only if) the requested output length is larger than 255
    /// times the size of the digest algorithm's output. (This is the limit
    /// imposed by the HKDF specification due to the way HKDF's counter is
    /// constructed.)
    #[inline]
    pub fn fill(self, out: &mut [u8]) -> Result<(), error::Unspecified> {
        fill_okm(self.prk, self.info, out, self.len_cached)
    }
}

fn fill_okm(
    prk: &Prk,
    info: &[&[u8]],
    out: &mut [u8],
    len: usize,
) -> Result<(), error::Unspecified> {
    if out.len() != len {
        return Err(error::Unspecified);
    }

    let digest_alg = prk.0.algorithm().digest_algorithm();
    assert!(digest_alg.block_len() >= digest_alg.output_len());

    let mut ctx = hmac::Context::with_key(&prk.0);

    let mut n = 1u8;
    let mut out = out;
    loop {
        for info in info {
            ctx.update(info);
        }
        ctx.update(&[n]);

        let t = ctx.sign();
        let t = t.as_ref();

        // Append `t` to the output.
        out = if out.len() < digest_alg.output_len() {
            let len = out.len();
            out.copy_from_slice(&t[..len]);
            &mut []
        } else {
            let (this_chunk, rest) = out.split_at_mut(digest_alg.output_len());
            this_chunk.copy_from_slice(t);
            rest
        };

        if out.is_empty() {
            return Ok(());
        }

        ctx = hmac::Context::with_key(&prk.0);
        ctx.update(t);
        n = n.checked_add(1).unwrap();
    }
}
