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

//! HMAC is specified in [RFC 2104].
//!
//! After a `Key` is constructed, it can be used for multiple signing or
//! verification operations. Separating the construction of the key from the
//! rest of the HMAC operation allows the per-key precomputation to be done
//! only once, instead of it being done in every HMAC operation.
//!
//! Frequently all the data to be signed in a message is available in a single
//! contiguous piece. In that case, the module-level `sign` function can be
//! used. Otherwise, if the input is in multiple parts, `Context` should be
//! used.
//!
//! # Examples:
//!
//! ## Signing a value and verifying it wasn't tampered with
//!
//! ```
//! use ring::{hmac, rand};
//!
//! let rng = rand::SystemRandom::new();
//! let key = hmac::Key::generate(hmac::HMAC_SHA256, &rng)?;
//!
//! let msg = "hello, world";
//!
//! let tag = hmac::sign(&key, msg.as_bytes());
//!
//! // [We give access to the message to an untrusted party, and they give it
//! // back to us. We need to verify they didn't tamper with it.]
//!
//! hmac::verify(&key, msg.as_bytes(), tag.as_ref())?;
//!
//! # Ok::<(), ring::error::Unspecified>(())
//! ```
//!
//! ## Using the one-shot API:
//!
//! ```
//! use ring::{digest, hmac, rand};
//! use ring::rand::SecureRandom;
//!
//! let msg = "hello, world";
//!
//! // The sender generates a secure key value and signs the message with it.
//! // Note that in a real protocol, a key agreement protocol would be used to
//! // derive `key_value`.
//! let rng = rand::SystemRandom::new();
//! let key_value: [u8; digest::SHA256_OUTPUT_LEN] = rand::generate(&rng)?.expose();
//!
//! let s_key = hmac::Key::new(hmac::HMAC_SHA256, key_value.as_ref());
//! let tag = hmac::sign(&s_key, msg.as_bytes());
//!
//! // The receiver (somehow!) knows the key value, and uses it to verify the
//! // integrity of the message.
//! let v_key = hmac::Key::new(hmac::HMAC_SHA256, key_value.as_ref());
//! hmac::verify(&v_key, msg.as_bytes(), tag.as_ref())?;
//!
//! # Ok::<(), ring::error::Unspecified>(())
//! ```
//!
//! ## Using the multi-part API:
//! ```
//! use ring::{digest, hmac, rand};
//! use ring::rand::SecureRandom;
//!
//! let parts = ["hello", ", ", "world"];
//!
//! // The sender generates a secure key value and signs the message with it.
//! // Note that in a real protocol, a key agreement protocol would be used to
//! // derive `key_value`.
//! let rng = rand::SystemRandom::new();
//! let mut key_value: [u8; digest::SHA384_OUTPUT_LEN] = rand::generate(&rng)?.expose();
//!
//! let s_key = hmac::Key::new(hmac::HMAC_SHA384, key_value.as_ref());
//! let mut s_ctx = hmac::Context::with_key(&s_key);
//! for part in &parts {
//!     s_ctx.update(part.as_bytes());
//! }
//! let tag = s_ctx.sign();
//!
//! // The receiver (somehow!) knows the key value, and uses it to verify the
//! // integrity of the message.
//! let v_key = hmac::Key::new(hmac::HMAC_SHA384, key_value.as_ref());
//! let mut msg = Vec::<u8>::new();
//! for part in &parts {
//!     msg.extend(part.as_bytes());
//! }
//! hmac::verify(&v_key, &msg.as_ref(), tag.as_ref())?;
//!
//! # Ok::<(), ring::error::Unspecified>(())
//! ```
//!
//! [RFC 2104]: https://tools.ietf.org/html/rfc2104

use crate::{
    bb, cpu,
    digest::{self, Digest, FinishError},
    error, hkdf, rand,
};

pub(crate) use crate::digest::InputTooLongError;

/// An HMAC algorithm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Algorithm(&'static digest::Algorithm);

impl Algorithm {
    /// The digest algorithm this HMAC algorithm is based on.
    #[inline]
    pub fn digest_algorithm(&self) -> &'static digest::Algorithm {
        self.0
    }
}

/// HMAC using SHA-1. Obsolete.
pub static HMAC_SHA1_FOR_LEGACY_USE_ONLY: Algorithm = Algorithm(&digest::SHA1_FOR_LEGACY_USE_ONLY);

/// HMAC using SHA-256.
pub static HMAC_SHA256: Algorithm = Algorithm(&digest::SHA256);

/// HMAC using SHA-384.
pub static HMAC_SHA384: Algorithm = Algorithm(&digest::SHA384);

/// HMAC using SHA-512.
pub static HMAC_SHA512: Algorithm = Algorithm(&digest::SHA512);

/// An HMAC tag.
///
/// For a given tag `t`, use `t.as_ref()` to get the tag value as a byte slice.
#[derive(Clone, Copy, Debug)]
pub struct Tag(Digest);

impl AsRef<[u8]> for Tag {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

/// A key to use for HMAC signing.
#[derive(Clone)]
pub struct Key {
    inner: digest::BlockContext,
    outer: digest::BlockContext,
}

impl core::fmt::Debug for Key {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> Result<(), core::fmt::Error> {
        f.debug_struct("Key")
            .field("algorithm", self.algorithm().digest_algorithm())
            .finish()
    }
}

impl Key {
    /// Generate an HMAC signing key using the given digest algorithm with a
    /// random value generated from `rng`.
    ///
    /// The key will be `digest_alg.output_len` bytes long, based on the
    /// recommendation in [RFC 2104 Section 3].
    ///
    /// [RFC 2104 Section 3]: https://tools.ietf.org/html/rfc2104#section-3
    pub fn generate(
        algorithm: Algorithm,
        rng: &dyn rand::SecureRandom,
    ) -> Result<Self, error::Unspecified> {
        Self::construct(algorithm, |buf| rng.fill(buf), cpu::features())
    }

    fn construct<F>(
        algorithm: Algorithm,
        fill: F,
        cpu: cpu::Features,
    ) -> Result<Self, error::Unspecified>
    where
        F: FnOnce(&mut [u8]) -> Result<(), error::Unspecified>,
    {
        let mut key_bytes = [0; digest::MAX_OUTPUT_LEN];
        let key_bytes = &mut key_bytes[..algorithm.0.output_len()];
        fill(key_bytes)?;
        Self::try_new(algorithm, key_bytes, cpu).map_err(error::erase::<InputTooLongError>)
    }

    /// Construct an HMAC signing key using the given digest algorithm and key
    /// value.
    ///
    /// `key_value` should be a value generated using a secure random number
    /// generator (e.g. the `key_value` output by
    /// `SealingKey::generate_serializable()`) or derived from a random key by
    /// a key derivation function (e.g. `ring::hkdf`). In particular,
    /// `key_value` shouldn't be a password.
    ///
    /// As specified in RFC 2104, if `key_value` is shorter than the digest
    /// algorithm's block length (as returned by `digest::Algorithm::block_len()`,
    /// not the digest length returned by `digest::Algorithm::output_len()`) then
    /// it will be padded with zeros. Similarly, if it is longer than the block
    /// length then it will be compressed using the digest algorithm.
    ///
    /// You should not use keys larger than the `digest_alg.block_len` because
    /// the truncation described above reduces their strength to only
    /// `digest_alg.output_len * 8` bits. Support for such keys is likely to be
    /// removed in a future version of *ring*.
    pub fn new(algorithm: Algorithm, key_value: &[u8]) -> Self {
        Self::try_new(algorithm, key_value, cpu::features())
            .map_err(error::erase::<InputTooLongError>)
            .unwrap()
    }

    pub(crate) fn try_new(
        algorithm: Algorithm,
        key_value: &[u8],
        cpu_features: cpu::Features,
    ) -> Result<Self, InputTooLongError> {
        let digest_alg = algorithm.0;
        let mut key = Self {
            inner: digest::BlockContext::new(digest_alg),
            outer: digest::BlockContext::new(digest_alg),
        };

        let block_len = digest_alg.block_len();

        let key_hash;
        let key_value = if key_value.len() <= block_len {
            key_value
        } else {
            key_hash = Digest::compute_from(digest_alg, key_value, cpu_features)?;
            key_hash.as_ref()
        };

        const IPAD: u8 = 0x36;

        let mut padded_key = [IPAD; digest::MAX_BLOCK_LEN];
        let padded_key = &mut padded_key[..block_len];

        // If the key is shorter than one block then we're supposed to act like
        // it is padded with zero bytes up to the block length. `x ^ 0 == x` so
        // we can just leave the trailing bytes of `padded_key` untouched.
        bb::xor_assign_at_start(&mut padded_key[..], key_value);

        let leftover = key.inner.update(padded_key, cpu_features);
        debug_assert_eq!(leftover.len(), 0);

        const OPAD: u8 = 0x5C;

        // Remove the `IPAD` masking, leaving the unmasked padded key, then
        // mask with `OPAD`, all in one step.
        bb::xor_assign(&mut padded_key[..], IPAD ^ OPAD);
        let leftover = key.outer.update(padded_key, cpu_features);
        debug_assert_eq!(leftover.len(), 0);

        Ok(key)
    }

    /// The digest algorithm for the key.
    #[inline]
    pub fn algorithm(&self) -> Algorithm {
        Algorithm(self.inner.algorithm)
    }

    pub(crate) fn sign(&self, data: &[u8], cpu: cpu::Features) -> Result<Tag, InputTooLongError> {
        let mut ctx = Context::with_key(self);
        ctx.update(data);
        ctx.try_sign(cpu)
    }

    fn verify(&self, data: &[u8], tag: &[u8], cpu: cpu::Features) -> Result<(), VerifyError> {
        let computed = self
            .sign(data, cpu)
            .map_err(VerifyError::InputTooLongError)?;
        bb::verify_slices_are_equal(computed.as_ref(), tag)
            .map_err(|_: error::Unspecified| VerifyError::Mismatch)
    }
}

impl hkdf::KeyType for Algorithm {
    fn len(&self) -> usize {
        self.digest_algorithm().output_len()
    }
}

impl From<hkdf::Okm<'_, Algorithm>> for Key {
    fn from(okm: hkdf::Okm<Algorithm>) -> Self {
        Self::construct(*okm.len(), |buf| okm.fill(buf), cpu::features()).unwrap()
    }
}

/// A context for multi-step (Init-Update-Finish) HMAC signing.
///
/// Use `sign` for single-step HMAC signing.
#[derive(Clone)]
pub struct Context {
    inner: digest::Context,
    outer: digest::BlockContext,
}

impl core::fmt::Debug for Context {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> Result<(), core::fmt::Error> {
        f.debug_struct("Context")
            .field("algorithm", self.inner.algorithm())
            .finish()
    }
}

impl Context {
    /// Constructs a new HMAC signing context using the given digest algorithm
    /// and key.
    pub fn with_key(signing_key: &Key) -> Self {
        Self {
            inner: digest::Context::clone_from(&signing_key.inner),
            outer: signing_key.outer.clone(),
        }
    }

    /// Updates the HMAC with all the data in `data`. `update` may be called
    /// zero or more times until `finish` is called.
    pub fn update(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    /// Finalizes the HMAC calculation and returns the HMAC value. `sign`
    /// consumes the context so it cannot be (mis-)used after `sign` has been
    /// called.
    ///
    /// It is generally not safe to implement HMAC verification by comparing
    /// the return value of `sign` to a tag. Use `verify` for verification
    /// instead.
    pub fn sign(self) -> Tag {
        self.try_sign(cpu::features())
            .map_err(error::erase::<InputTooLongError>)
            .unwrap()
    }

    pub(crate) fn try_sign(self, cpu_features: cpu::Features) -> Result<Tag, InputTooLongError> {
        // Consequently, `num_pending` is valid.
        debug_assert_eq!(self.inner.algorithm(), self.outer.algorithm);
        debug_assert!(self.inner.algorithm().output_len() < self.outer.algorithm.block_len());

        let inner = self.inner.try_finish(cpu_features)?;
        let inner = inner.as_ref();
        let num_pending = inner.len();
        let buffer = &mut [0u8; digest::MAX_BLOCK_LEN];
        const _BUFFER_IS_LARGE_ENOUGH_TO_HOLD_INNER: () =
            assert!(digest::MAX_OUTPUT_LEN < digest::MAX_BLOCK_LEN);
        buffer[..num_pending].copy_from_slice(inner);

        self.outer
            .try_finish(buffer, num_pending, cpu_features)
            .map(Tag)
            .map_err(|err| match err {
                FinishError::InputTooLong(i) => {
                    // Unreachable, as we gave the inner context exactly the
                    // same input we gave the outer context, and
                    // `inner.try_finish` already succeeded. However, it is
                    // quite difficult to prove this, and we already return
                    // `InputTooLongError`, so just forward it along.
                    i
                }
                FinishError::PendingNotAPartialBlock(_) => {
                    // Follows from the assertions above.
                    unreachable!()
                }
            })
    }
}

/// Calculates the HMAC of `data` using the key `key` in one step.
///
/// Use `Context` to calculate HMACs where the input is in multiple parts.
///
/// It is generally not safe to implement HMAC verification by comparing the
/// return value of `sign` to a tag. Use `verify` for verification instead.
pub fn sign(key: &Key, data: &[u8]) -> Tag {
    key.sign(data, cpu::features())
        .map_err(error::erase::<InputTooLongError>)
        .unwrap()
}

/// Calculates the HMAC of `data` using the signing key `key`, and verifies
/// whether the resultant value equals `tag`, in one step.
///
/// This is logically equivalent to, but more efficient than, constructing a
/// `Key` with the same value as `key` and then using `verify`.
///
/// The verification will be done in constant time to prevent timing attacks.
pub fn verify(key: &Key, data: &[u8], tag: &[u8]) -> Result<(), error::Unspecified> {
    key.verify(data, tag, cpu::features())
        .map_err(|_: VerifyError| error::Unspecified)
}

enum VerifyError {
    // Theoretically somebody could have calculated a valid tag with a gigantic
    // input that we do not support. If we were to support every theoretically
    // valid input length, for *every* digest algorithm, then we could argue
    // that hitting the input length limit implies a mismatch since nobody
    // could have calculated such a tag with the given input.
    #[allow(dead_code)]
    InputTooLongError(InputTooLongError),

    Mismatch,
}

#[cfg(test)]
mod tests {
    use crate::{hmac, rand};

    // Make sure that `Key::generate` and `verify_with_own_key` aren't
    // completely wacky.
    #[test]
    pub fn hmac_signing_key_coverage() {
        let rng = rand::SystemRandom::new();

        const HELLO_WORLD_GOOD: &[u8] = b"hello, world";
        const HELLO_WORLD_BAD: &[u8] = b"hello, worle";

        for algorithm in &[
            hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY,
            hmac::HMAC_SHA256,
            hmac::HMAC_SHA384,
            hmac::HMAC_SHA512,
        ] {
            let key = hmac::Key::generate(*algorithm, &rng).unwrap();
            let tag = hmac::sign(&key, HELLO_WORLD_GOOD);
            assert!(hmac::verify(&key, HELLO_WORLD_GOOD, tag.as_ref()).is_ok());
            assert!(hmac::verify(&key, HELLO_WORLD_BAD, tag.as_ref()).is_err())
        }
    }
}
