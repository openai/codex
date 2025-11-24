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

use super::PublicKey;
use core::iter::FromIterator;

/// RSA public key components.
///
/// `B` must implement `AsRef<[u8]>` like `&[u8]` or `Vec<u8>`.
#[derive(Clone, Copy)]
pub struct PublicKeyComponents<B> {
    /// The public modulus, encoded in big-endian bytes without leading zeros.
    pub n: B,

    /// The public exponent, encoded in big-endian bytes without leading zeros.
    pub e: B,
}

impl<B> core::fmt::Debug for PublicKeyComponents<B>
where
    B: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        f.debug_struct("PublicKeyComponents")
            .field("n", &self.n)
            .field("e", &self.e)
            .finish()
    }
}

impl<B> From<&PublicKey> for PublicKeyComponents<B>
where
    B: FromIterator<u8>,
{
    fn from(public_key: &PublicKey) -> Self {
        Self {
            n: public_key.inner().n().be_bytes().collect(),
            e: public_key.inner().e().be_bytes().collect(),
        }
    }
}
