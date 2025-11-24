// Copyright 2018-2024 Brian Smith.
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

use super::{nonce::Nonce, overlapping, quic::Sample, NONCE_LEN};
use crate::{
    bb,
    cpu::{self, GetFeature as _},
    error,
    polyfill::unwrap_const,
};
use cfg_if::cfg_if;
use core::num::NonZeroU32;

pub(super) use ffi::Counter;

#[macro_use]
mod ffi;

mod bs;
pub(super) mod fallback;
pub(super) mod hw;
pub(super) mod vp;

pub type Overlapping<'o> = overlapping::Overlapping<'o, u8>;
pub type OverlappingPartialBlock<'o> = overlapping::PartialBlock<'o, u8, BLOCK_LEN>;

cfg_if! {
    if #[cfg(any(all(target_arch = "aarch64", target_endian = "little"), target_arch = "x86_64"))] {
        pub(super) use ffi::AES_KEY;
    } else {
        use ffi::AES_KEY;
    }
}

#[derive(Clone)]
pub(super) enum Key {
    #[cfg(any(
        all(target_arch = "aarch64", target_endian = "little"),
        target_arch = "x86_64",
        target_arch = "x86"
    ))]
    Hw(hw::Key),

    #[cfg(any(
        all(target_arch = "aarch64", target_endian = "little"),
        all(target_arch = "arm", target_endian = "little"),
        target_arch = "x86",
        target_arch = "x86_64"
    ))]
    Vp(vp::Key),

    Fallback(fallback::Key),
}

impl Key {
    #[inline]
    pub fn new(
        bytes: KeyBytes<'_>,
        cpu_features: cpu::Features,
    ) -> Result<Self, error::Unspecified> {
        #[cfg(any(
            all(target_arch = "aarch64", target_endian = "little"),
            target_arch = "x86",
            target_arch = "x86_64"
        ))]
        if let Some(hw_features) = cpu_features.get_feature() {
            return Ok(Self::Hw(hw::Key::new(
                bytes,
                hw_features,
                cpu_features.get_feature(),
            )?));
        }

        #[cfg(any(
            all(target_arch = "aarch64", target_endian = "little"),
            all(target_arch = "arm", target_endian = "little"),
            target_arch = "x86_64",
            target_arch = "x86"
        ))]
        if let Some(vp_features) = cpu_features.get_feature() {
            return Ok(Self::Vp(vp::Key::new(bytes, vp_features)?));
        }

        let _ = cpu_features;

        Ok(Self::Fallback(fallback::Key::new(bytes)?))
    }

    #[inline]
    fn encrypt_block(&self, a: Block) -> Block {
        match self {
            #[cfg(any(
                all(target_arch = "aarch64", target_endian = "little"),
                target_arch = "x86_64",
                target_arch = "x86"
            ))]
            Key::Hw(inner) => inner.encrypt_block(a),

            #[cfg(any(
                all(target_arch = "aarch64", target_endian = "little"),
                all(target_arch = "arm", target_endian = "little"),
                target_arch = "x86",
                target_arch = "x86_64"
            ))]
            Key::Vp(inner) => inner.encrypt_block(a),

            Key::Fallback(inner) => inner.encrypt_block(a),
        }
    }

    pub fn new_mask(&self, sample: Sample) -> [u8; 5] {
        let [b0, b1, b2, b3, b4, ..] = self.encrypt_block(sample);
        [b0, b1, b2, b3, b4]
    }
}

pub const AES_128_KEY_LEN: usize = 128 / 8;
pub const AES_256_KEY_LEN: usize = 256 / 8;

pub enum KeyBytes<'a> {
    AES_128(&'a [u8; AES_128_KEY_LEN]),
    AES_256(&'a [u8; AES_256_KEY_LEN]),
}

// `Counter` is `ffi::Counter` as its representation is dictated by its use in
// the FFI.
impl Counter {
    pub fn one(nonce: Nonce) -> Self {
        let mut value = [0u8; BLOCK_LEN];
        value[..NONCE_LEN].copy_from_slice(nonce.as_ref());
        value[BLOCK_LEN - 1] = 1;
        Self(value)
    }

    pub fn increment(&mut self) -> Iv {
        const ONE: NonZeroU32 = unwrap_const(NonZeroU32::new(1));

        let iv = Iv(self.0);
        self.increment_by_less_safe(ONE);
        iv
    }

    pub(super) fn increment_by_less_safe(&mut self, increment_by: NonZeroU32) {
        let [.., c0, c1, c2, c3] = &mut self.0;
        let old_value: u32 = u32::from_be_bytes([*c0, *c1, *c2, *c3]);
        let new_value = old_value.wrapping_add(increment_by.get());
        [*c0, *c1, *c2, *c3] = u32::to_be_bytes(new_value);
    }
}

/// The IV for a single block encryption.
///
/// Intentionally not `Clone` to ensure each is used only once.
pub struct Iv(Block);

impl From<Counter> for Iv {
    fn from(counter: Counter) -> Self {
        Self(counter.0)
    }
}

pub(super) type Block = [u8; BLOCK_LEN];
pub(super) const BLOCK_LEN: usize = 16;
pub(super) const ZERO_BLOCK: Block = [0u8; BLOCK_LEN];

pub(super) trait EncryptBlock {
    fn encrypt_block(&self, block: Block) -> Block;
    fn encrypt_iv_xor_block(&self, iv: Iv, block: Block) -> Block;
}

pub(super) trait EncryptCtr32 {
    fn ctr32_encrypt_within(&self, in_out: Overlapping<'_>, ctr: &mut Counter);
}

#[allow(dead_code)]
fn encrypt_block_using_encrypt_iv_xor_block(key: &impl EncryptBlock, block: Block) -> Block {
    key.encrypt_iv_xor_block(Iv(block), ZERO_BLOCK)
}

fn encrypt_iv_xor_block_using_encrypt_block(
    key: &impl EncryptBlock,
    iv: Iv,
    block: Block,
) -> Block {
    let encrypted_iv = key.encrypt_block(iv.0);
    bb::xor_16(encrypted_iv, block)
}

#[allow(dead_code)]
fn encrypt_iv_xor_block_using_ctr32(key: &impl EncryptCtr32, iv: Iv, mut block: Block) -> Block {
    let mut ctr = Counter(iv.0); // This is OK because we're only encrypting one block.
    key.ctr32_encrypt_within(block.as_mut().into(), &mut ctr);
    block
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil as test;

    #[test]
    pub fn test_aes() {
        test::run(test_vector_file!("aes_tests.txt"), |section, test_case| {
            assert_eq!(section, "");
            let key = consume_key(test_case, "Key");
            let input = test_case.consume_bytes("Input");
            let block: Block = input.as_slice().try_into()?;
            let expected_output = test_case.consume_bytes("Output");

            let output = key.encrypt_block(block);
            assert_eq!(output.as_ref(), &expected_output[..]);

            Ok(())
        })
    }

    fn consume_key(test_case: &mut test::TestCase, name: &str) -> Key {
        let key = test_case.consume_bytes(name);
        let key = &key[..];
        let key = match key.len() {
            16 => KeyBytes::AES_128(key.try_into().unwrap()),
            32 => KeyBytes::AES_256(key.try_into().unwrap()),
            _ => unreachable!(),
        };
        Key::new(key, cpu::features()).unwrap()
    }
}

// These AES-GCM-specific tests are here instead of in `aes_gcm` because
// `Counter`'s API isn't visible (enough) to aes_gcm.
#[cfg(test)]
mod aes_gcm_tests {
    use super::{super::aes_gcm::MAX_IN_OUT_LEN, *};
    use core::num::NonZeroU32;

    #[test]
    fn test_aes_gcm_counter_blocks_max() {
        test_aes_gcm_counter_blocks(MAX_IN_OUT_LEN, &[0, 0, 0, 0]);
    }

    #[test]
    fn test_aes_gcm_counter_blocks_max_minus_one() {
        test_aes_gcm_counter_blocks(MAX_IN_OUT_LEN - BLOCK_LEN, &[0xff, 0xff, 0xff, 0xff]);
    }

    fn test_aes_gcm_counter_blocks(in_out_len: usize, expected_final_counter: &[u8; 4]) {
        fn ctr32(ctr: &Counter) -> &[u8; 4] {
            (&ctr.0[12..]).try_into().unwrap()
        }

        let rounded_down = in_out_len / BLOCK_LEN;
        let blocks = rounded_down + (if in_out_len % BLOCK_LEN == 0 { 0 } else { 1 });
        let blocks = u32::try_from(blocks)
            .ok()
            .and_then(NonZeroU32::new)
            .unwrap();

        let nonce = Nonce::assume_unique_for_key([1; 12]);
        let mut ctr = Counter::one(nonce);
        assert_eq!(ctr32(&ctr), &[0, 0, 0, 1]);
        let _tag_iv = ctr.increment();
        assert_eq!(ctr32(&ctr), &[0, 0, 0, 2]);
        ctr.increment_by_less_safe(blocks);

        // `MAX_IN_OUT_LEN` is less on 32-bit targets, so we don't even get
        // close to wrapping, but run the tests on them anyway.
        #[cfg(target_pointer_width = "64")]
        assert_eq!(ctr32(&ctr), expected_final_counter);

        #[cfg(target_pointer_width = "32")]
        let _ = expected_final_counter;
    }
}
