// Copyright 2018 Brian Smith.
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

use crate::error;

/// A nonce for a single AEAD opening or sealing operation.
///
/// The user must ensure, for a particular key, that each nonce is unique.
///
/// `Nonce` intentionally doesn't implement `Clone` to ensure that each one is
/// consumed at most once.
pub struct Nonce([u8; NONCE_LEN]);

impl Nonce {
    /// Constructs a `Nonce` with the given value, assuming that the value is
    /// unique for the lifetime of the key it is being used with.
    ///
    /// Fails if `value` isn't `NONCE_LEN` bytes long.
    #[inline]
    pub fn try_assume_unique_for_key(value: &[u8]) -> Result<Self, error::Unspecified> {
        let value: &[u8; NONCE_LEN] = value.try_into()?;
        Ok(Self::assume_unique_for_key(*value))
    }

    /// Constructs a `Nonce` with the given value, assuming that the value is
    /// unique for the lifetime of the key it is being used with.
    #[inline]
    pub fn assume_unique_for_key(value: [u8; NONCE_LEN]) -> Self {
        Self(value)
    }
}

impl AsRef<[u8; NONCE_LEN]> for Nonce {
    fn as_ref(&self) -> &[u8; NONCE_LEN] {
        &self.0
    }
}

/// All the AEADs we support use 96-bit nonces.
pub const NONCE_LEN: usize = 96 / 8;
