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

//! QUIC Header Protection.
//!
//! See draft-ietf-quic-tls.

use crate::{
    aead::{aes, chacha},
    cpu, error, hkdf,
};

/// A key for generating QUIC Header Protection masks.
pub struct HeaderProtectionKey {
    inner: KeyInner,
    algorithm: &'static Algorithm,
}

#[allow(clippy::large_enum_variant, variant_size_differences)]
enum KeyInner {
    Aes(aes::Key),
    ChaCha20(chacha::Key),
}

impl From<hkdf::Okm<'_, &'static Algorithm>> for HeaderProtectionKey {
    fn from(okm: hkdf::Okm<&'static Algorithm>) -> Self {
        let mut key_bytes = [0; super::MAX_KEY_LEN];
        let algorithm = *okm.len();
        let key_bytes = &mut key_bytes[..algorithm.key_len()];
        okm.fill(key_bytes).unwrap();
        Self::new(algorithm, key_bytes).unwrap()
    }
}

impl HeaderProtectionKey {
    /// Create a new header protection key.
    ///
    /// `key_bytes` must be exactly `algorithm.key_len` bytes long.
    pub fn new(
        algorithm: &'static Algorithm,
        key_bytes: &[u8],
    ) -> Result<Self, error::Unspecified> {
        Ok(Self {
            inner: (algorithm.init)(key_bytes, cpu::features())?,
            algorithm,
        })
    }

    /// Generate a new QUIC Header Protection mask.
    ///
    /// `sample` must be exactly `self.algorithm().sample_len()` bytes long.
    pub fn new_mask(&self, sample: &[u8]) -> Result<[u8; 5], error::Unspecified> {
        let sample = <&[u8; SAMPLE_LEN]>::try_from(sample)?;

        let out = (self.algorithm.new_mask)(&self.inner, *sample);
        Ok(out)
    }

    /// The key's algorithm.
    #[inline(always)]
    pub fn algorithm(&self) -> &'static Algorithm {
        self.algorithm
    }
}

const SAMPLE_LEN: usize = super::TAG_LEN;

/// QUIC sample for new key masks
pub type Sample = [u8; SAMPLE_LEN];

/// A QUIC Header Protection Algorithm.
pub struct Algorithm {
    init: fn(key: &[u8], cpu_features: cpu::Features) -> Result<KeyInner, error::Unspecified>,

    new_mask: fn(key: &KeyInner, sample: Sample) -> [u8; 5],

    key_len: usize,
    id: AlgorithmID,
}

impl hkdf::KeyType for &'static Algorithm {
    #[inline]
    fn len(&self) -> usize {
        self.key_len()
    }
}

impl Algorithm {
    /// The length of the key.
    #[inline(always)]
    pub fn key_len(&self) -> usize {
        self.key_len
    }

    /// The required sample length.
    #[inline(always)]
    pub fn sample_len(&self) -> usize {
        SAMPLE_LEN
    }
}

derive_debug_via_id!(Algorithm);

#[derive(Debug, Eq, PartialEq)]
enum AlgorithmID {
    AES_128,
    AES_256,
    CHACHA20,
}

impl PartialEq for Algorithm {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Algorithm {}

/// AES-128.
pub static AES_128: Algorithm = Algorithm {
    key_len: 16,
    init: aes_init_128,
    new_mask: aes_new_mask,
    id: AlgorithmID::AES_128,
};

/// AES-256.
pub static AES_256: Algorithm = Algorithm {
    key_len: 32,
    init: aes_init_256,
    new_mask: aes_new_mask,
    id: AlgorithmID::AES_256,
};

fn aes_init_128(key: &[u8], cpu_features: cpu::Features) -> Result<KeyInner, error::Unspecified> {
    let key = key.try_into().map_err(|_| error::Unspecified)?;
    let aes_key = aes::Key::new(aes::KeyBytes::AES_128(key), cpu_features)?;
    Ok(KeyInner::Aes(aes_key))
}

fn aes_init_256(key: &[u8], cpu_features: cpu::Features) -> Result<KeyInner, error::Unspecified> {
    let key = key.try_into().map_err(|_| error::Unspecified)?;
    let aes_key = aes::Key::new(aes::KeyBytes::AES_256(key), cpu_features)?;
    Ok(KeyInner::Aes(aes_key))
}

fn aes_new_mask(key: &KeyInner, sample: Sample) -> [u8; 5] {
    let aes_key = match key {
        KeyInner::Aes(key) => key,
        _ => unreachable!(),
    };

    aes_key.new_mask(sample)
}

/// ChaCha20.
pub static CHACHA20: Algorithm = Algorithm {
    key_len: chacha::KEY_LEN,
    init: chacha20_init,
    new_mask: chacha20_new_mask,
    id: AlgorithmID::CHACHA20,
};

fn chacha20_init(key: &[u8], _cpu_features: cpu::Features) -> Result<KeyInner, error::Unspecified> {
    let chacha20_key: [u8; chacha::KEY_LEN] = key.try_into()?;
    Ok(KeyInner::ChaCha20(chacha::Key::new(chacha20_key)))
}

fn chacha20_new_mask(key: &KeyInner, sample: Sample) -> [u8; 5] {
    let chacha20_key = match key {
        KeyInner::ChaCha20(key) => key,
        _ => unreachable!(),
    };

    chacha20_key.new_mask(sample)
}
