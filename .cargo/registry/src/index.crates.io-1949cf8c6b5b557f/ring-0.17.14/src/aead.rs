// Copyright 2015-2024 Brian Smith.
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

//! Authenticated Encryption with Associated Data (AEAD).
//!
//! See [Authenticated encryption: relations among notions and analysis of the
//! generic composition paradigm][AEAD] for an introduction to the concept of
//! AEADs.
//!
//! [AEAD]: https://eprint.iacr.org/2000/025.pdf
//! [`crypto.cipher.AEAD`]: https://golang.org/pkg/crypto/cipher/#AEAD

use crate::{
    cpu, error,
    polyfill::{u64_from_usize, usize_from_u64_saturated},
};

pub use self::{
    algorithm::{Algorithm, AES_128_GCM, AES_256_GCM, CHACHA20_POLY1305},
    less_safe_key::LessSafeKey,
    nonce::{Nonce, NONCE_LEN},
    opening_key::OpeningKey,
    sealing_key::SealingKey,
    unbound_key::UnboundKey,
};

/// A sequences of unique nonces.
///
/// A given `NonceSequence` must never return the same `Nonce` twice from
/// `advance()`.
///
/// A simple counter is a reasonable (but probably not ideal) `NonceSequence`.
///
/// Intentionally not `Clone` or `Copy` since cloning would allow duplication
/// of the sequence.
pub trait NonceSequence {
    /// Returns the next nonce in the sequence.
    ///
    /// This may fail if "too many" nonces have been requested, where how many
    /// is too many is up to the implementation of `NonceSequence`. An
    /// implementation may that enforce a maximum number of records are
    /// sent/received under a key this way. Once `advance()` fails, it must
    /// fail for all subsequent calls.
    fn advance(&mut self) -> Result<Nonce, error::Unspecified>;
}

/// An AEAD key bound to a nonce sequence.
pub trait BoundKey<N: NonceSequence>: core::fmt::Debug {
    /// Constructs a new key from the given `UnboundKey` and `NonceSequence`.
    fn new(key: UnboundKey, nonce_sequence: N) -> Self;

    /// The key's AEAD algorithm.
    fn algorithm(&self) -> &'static Algorithm;
}

/// The additionally authenticated data (AAD) for an opening or sealing
/// operation. This data is authenticated but is **not** encrypted.
///
/// The type `A` could be a byte slice `&[u8]`, a byte array `[u8; N]`
/// for some constant `N`, `Vec<u8>`, etc.
#[derive(Clone, Copy)]
pub struct Aad<A>(A);

impl<A: AsRef<[u8]>> Aad<A> {
    /// Construct the `Aad` from the given bytes.
    #[inline]
    pub fn from(aad: A) -> Self {
        Self(aad)
    }
}

impl<A> AsRef<[u8]> for Aad<A>
where
    A: AsRef<[u8]>,
{
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl Aad<[u8; 0]> {
    /// Construct an empty `Aad`.
    pub fn empty() -> Self {
        Self::from([])
    }
}

impl<A> core::fmt::Debug for Aad<A>
where
    A: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Aad").field(&self.0).finish()
    }
}

impl<A> PartialEq for Aad<A>
where
    A: PartialEq,
{
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl<A> Eq for Aad<A> where A: Eq {}

#[allow(clippy::large_enum_variant, variant_size_differences)]
#[derive(Clone)]
enum KeyInner {
    AesGcm(aes_gcm::Key),
    ChaCha20Poly1305(chacha20_poly1305::Key),
}

const fn max_input_len(block_len: usize, overhead_blocks_per_nonce: usize) -> usize {
    // Each of our AEADs use a 32-bit block counter so the maximum is the
    // largest input that will not overflow the counter.
    usize_from_u64_saturated(
        ((1u64 << 32) - u64_from_usize(overhead_blocks_per_nonce)) * u64_from_usize(block_len),
    )
}

/// A possibly valid authentication tag.
#[must_use]
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Tag([u8; TAG_LEN]);

impl AsRef<[u8]> for Tag {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl TryFrom<&[u8]> for Tag {
    type Error = error::Unspecified;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let raw_tag: [u8; TAG_LEN] = value.try_into().map_err(|_| error::Unspecified)?;
        Ok(Self::from(raw_tag))
    }
}

impl From<[u8; TAG_LEN]> for Tag {
    #[inline]
    fn from(value: [u8; TAG_LEN]) -> Self {
        Self(value)
    }
}

const MAX_KEY_LEN: usize = 32;

// All the AEADs we support use 128-bit tags.
const TAG_LEN: usize = 16;

/// The maximum length of a tag for the algorithms in this module.
pub const MAX_TAG_LEN: usize = TAG_LEN;

mod aes;
mod aes_gcm;
mod algorithm;
mod chacha;
mod chacha20_poly1305;
pub mod chacha20_poly1305_openssh;
mod gcm;
mod less_safe_key;
mod nonce;
mod opening_key;
mod overlapping;
mod poly1305;
pub mod quic;
mod sealing_key;
mod shift;
mod unbound_key;
