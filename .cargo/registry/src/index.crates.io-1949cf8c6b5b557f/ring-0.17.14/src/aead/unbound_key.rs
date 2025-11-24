// Copyright 2015-2021 Brian Smith.
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

use super::{Algorithm, LessSafeKey, MAX_KEY_LEN};
use crate::{cpu, error, hkdf};

/// An AEAD key without a designated role or nonce sequence.
pub struct UnboundKey {
    inner: LessSafeKey,
}

impl UnboundKey {
    /// Constructs a `UnboundKey`.
    ///
    /// Fails if `key_bytes.len() != algorithm.key_len()`.
    #[inline]
    pub fn new(
        algorithm: &'static Algorithm,
        key_bytes: &[u8],
    ) -> Result<Self, error::Unspecified> {
        Ok(Self {
            inner: LessSafeKey::new_(algorithm, key_bytes, cpu::features())?,
        })
    }

    /// The key's AEAD algorithm.
    #[inline]
    pub fn algorithm(&self) -> &'static Algorithm {
        self.inner.algorithm()
    }

    #[inline]
    pub(super) fn into_inner(self) -> LessSafeKey {
        self.inner
    }
}

impl core::fmt::Debug for UnboundKey {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> Result<(), core::fmt::Error> {
        self.inner.fmt_debug("UnboundKey", f)
    }
}

impl From<hkdf::Okm<'_, &'static Algorithm>> for UnboundKey {
    fn from(okm: hkdf::Okm<&'static Algorithm>) -> Self {
        let mut key_bytes = [0; MAX_KEY_LEN];
        let key_bytes = &mut key_bytes[..okm.len().key_len()];
        let algorithm = *okm.len();
        okm.fill(key_bytes).unwrap();
        Self {
            inner: LessSafeKey::new_(algorithm, key_bytes, cpu::features()).unwrap(),
        }
    }
}
