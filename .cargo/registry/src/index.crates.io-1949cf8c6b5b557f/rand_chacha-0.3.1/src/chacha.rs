// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The ChaCha random number generator.

#[cfg(not(feature = "std"))] use core;
#[cfg(feature = "std")] use std as core;

use self::core::fmt;
use crate::guts::ChaCha;
use rand_core::block::{BlockRng, BlockRngCore};
use rand_core::{CryptoRng, Error, RngCore, SeedableRng};

#[cfg(feature = "serde1")] use serde::{Serialize, Deserialize, Serializer, Deserializer};

// NB. this must remain consistent with some currently hard-coded numbers in this module
const BUF_BLOCKS: u8 = 4;
// number of 32-bit words per ChaCha block (fixed by algorithm definition)
const BLOCK_WORDS: u8 = 16;

#[repr(transparent)]
pub struct Array64<T>([T; 64]);
impl<T> Default for Array64<T>
where T: Default
{
    #[rustfmt::skip]
    fn default() -> Self {
        Self([
            T::default(), T::default(), T::default(), T::default(), T::default(), T::default(), T::default(), T::default(),
            T::default(), T::default(), T::default(), T::default(), T::default(), T::default(), T::default(), T::default(),
            T::default(), T::default(), T::default(), T::default(), T::default(), T::default(), T::default(), T::default(),
            T::default(), T::default(), T::default(), T::default(), T::default(), T::default(), T::default(), T::default(),
            T::default(), T::default(), T::default(), T::default(), T::default(), T::default(), T::default(), T::default(),
            T::default(), T::default(), T::default(), T::default(), T::default(), T::default(), T::default(), T::default(),
            T::default(), T::default(), T::default(), T::default(), T::default(), T::default(), T::default(), T::default(),
            T::default(), T::default(), T::default(), T::default(), T::default(), T::default(), T::default(), T::default(),
        ])
    }
}
impl<T> AsRef<[T]> for Array64<T> {
    fn as_ref(&self) -> &[T] {
        &self.0
    }
}
impl<T> AsMut<[T]> for Array64<T> {
    fn as_mut(&mut self) -> &mut [T] {
        &mut self.0
    }
}
impl<T> Clone for Array64<T>
where T: Copy + Default
{
    fn clone(&self) -> Self {
        let mut new = Self::default();
        new.0.copy_from_slice(&self.0);
        new
    }
}
impl<T> fmt::Debug for Array64<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Array64 {{}}")
    }
}

macro_rules! chacha_impl {
    ($ChaChaXCore:ident, $ChaChaXRng:ident, $rounds:expr, $doc:expr, $abst:ident) => {
        #[doc=$doc]
        #[derive(Clone, PartialEq, Eq)]
        pub struct $ChaChaXCore {
            state: ChaCha,
        }

        // Custom Debug implementation that does not expose the internal state
        impl fmt::Debug for $ChaChaXCore {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "ChaChaXCore {{}}")
            }
        }

        impl BlockRngCore for $ChaChaXCore {
            type Item = u32;
            type Results = Array64<u32>;
            #[inline]
            fn generate(&mut self, r: &mut Self::Results) {
                // Fill slice of words by writing to equivalent slice of bytes, then fixing endianness.
                self.state.refill4($rounds, unsafe {
                    &mut *(&mut *r as *mut Array64<u32> as *mut [u8; 256])
                });
                for x in r.as_mut() {
                    *x = x.to_le();
                }
            }
        }

        impl SeedableRng for $ChaChaXCore {
            type Seed = [u8; 32];
            #[inline]
            fn from_seed(seed: Self::Seed) -> Self {
                $ChaChaXCore { state: ChaCha::new(&seed, &[0u8; 8]) }
            }
        }

        impl CryptoRng for $ChaChaXCore {}

        /// A cryptographically secure random number generator that uses the ChaCha algorithm.
        ///
        /// ChaCha is a stream cipher designed by Daniel J. Bernstein[^1], that we use as an RNG. It is
        /// an improved variant of the Salsa20 cipher family, which was selected as one of the "stream
        /// ciphers suitable for widespread adoption" by eSTREAM[^2].
        ///
        /// ChaCha uses add-rotate-xor (ARX) operations as its basis. These are safe against timing
        /// attacks, although that is mostly a concern for ciphers and not for RNGs. We provide a SIMD
        /// implementation to support high throughput on a variety of common hardware platforms.
        ///
        /// With the ChaCha algorithm it is possible to choose the number of rounds the core algorithm
        /// should run. The number of rounds is a tradeoff between performance and security, where 8
        /// rounds is the minimum potentially secure configuration, and 20 rounds is widely used as a
        /// conservative choice.
        ///
        /// We use a 64-bit counter and 64-bit stream identifier as in Bernstein's implementation[^1]
        /// except that we use a stream identifier in place of a nonce. A 64-bit counter over 64-byte
        /// (16 word) blocks allows 1 ZiB of output before cycling, and the stream identifier allows
        /// 2<sup>64</sup> unique streams of output per seed. Both counter and stream are initialized
        /// to zero but may be set via the `set_word_pos` and `set_stream` methods.
        ///
        /// The word layout is:
        ///
        /// ```text
        /// constant  constant  constant  constant
        /// seed      seed      seed      seed
        /// seed      seed      seed      seed
        /// counter   counter   stream_id stream_id
        /// ```
        ///
        /// This implementation uses an output buffer of sixteen `u32` words, and uses
        /// [`BlockRng`] to implement the [`RngCore`] methods.
        ///
        /// [^1]: D. J. Bernstein, [*ChaCha, a variant of Salsa20*](
        ///       https://cr.yp.to/chacha.html)
        ///
        /// [^2]: [eSTREAM: the ECRYPT Stream Cipher Project](
        ///       http://www.ecrypt.eu.org/stream/)
        #[derive(Clone, Debug)]
        pub struct $ChaChaXRng {
            rng: BlockRng<$ChaChaXCore>,
        }

        impl SeedableRng for $ChaChaXRng {
            type Seed = [u8; 32];
            #[inline]
            fn from_seed(seed: Self::Seed) -> Self {
                let core = $ChaChaXCore::from_seed(seed);
                Self {
                    rng: BlockRng::new(core),
                }
            }
        }

        impl RngCore for $ChaChaXRng {
            #[inline]
            fn next_u32(&mut self) -> u32 {
                self.rng.next_u32()
            }
            #[inline]
            fn next_u64(&mut self) -> u64 {
                self.rng.next_u64()
            }
            #[inline]
            fn fill_bytes(&mut self, bytes: &mut [u8]) {
                self.rng.fill_bytes(bytes)
            }
            #[inline]
            fn try_fill_bytes(&mut self, bytes: &mut [u8]) -> Result<(), Error> {
                self.rng.try_fill_bytes(bytes)
            }
        }

        impl $ChaChaXRng {
            // The buffer is a 4-block window, i.e. it is always at a block-aligned position in the
            // stream but if the stream has been seeked it may not be self-aligned.

            /// Get the offset from the start of the stream, in 32-bit words.
            ///
            /// Since the generated blocks are 16 words (2<sup>4</sup>) long and the
            /// counter is 64-bits, the offset is a 68-bit number. Sub-word offsets are
            /// not supported, hence the result can simply be multiplied by 4 to get a
            /// byte-offset.
            #[inline]
            pub fn get_word_pos(&self) -> u128 {
                let buf_start_block = {
                    let buf_end_block = self.rng.core.state.get_block_pos();
                    u64::wrapping_sub(buf_end_block, BUF_BLOCKS.into())
                };
                let (buf_offset_blocks, block_offset_words) = {
                    let buf_offset_words = self.rng.index() as u64;
                    let blocks_part = buf_offset_words / u64::from(BLOCK_WORDS);
                    let words_part = buf_offset_words % u64::from(BLOCK_WORDS);
                    (blocks_part, words_part)
                };
                let pos_block = u64::wrapping_add(buf_start_block, buf_offset_blocks);
                let pos_block_words = u128::from(pos_block) * u128::from(BLOCK_WORDS);
                pos_block_words + u128::from(block_offset_words)
            }

            /// Set the offset from the start of the stream, in 32-bit words.
            ///
            /// As with `get_word_pos`, we use a 68-bit number. Since the generator
            /// simply cycles at the end of its period (1 ZiB), we ignore the upper
            /// 60 bits.
            #[inline]
            pub fn set_word_pos(&mut self, word_offset: u128) {
                let block = (word_offset / u128::from(BLOCK_WORDS)) as u64;
                self.rng
                    .core
                    .state
                    .set_block_pos(block);
                self.rng.generate_and_set((word_offset % u128::from(BLOCK_WORDS)) as usize);
            }

            /// Set the stream number.
            ///
            /// This is initialized to zero; 2<sup>64</sup> unique streams of output
            /// are available per seed/key.
            ///
            /// Note that in order to reproduce ChaCha output with a specific 64-bit
            /// nonce, one can convert that nonce to a `u64` in little-endian fashion
            /// and pass to this function. In theory a 96-bit nonce can be used by
            /// passing the last 64-bits to this function and using the first 32-bits as
            /// the most significant half of the 64-bit counter (which may be set
            /// indirectly via `set_word_pos`), but this is not directly supported.
            #[inline]
            pub fn set_stream(&mut self, stream: u64) {
                self.rng
                    .core
                    .state
                    .set_nonce(stream);
                if self.rng.index() != 64 {
                    let wp = self.get_word_pos();
                    self.set_word_pos(wp);
                }
            }

            /// Get the stream number.
            #[inline]
            pub fn get_stream(&self) -> u64 {
                self.rng
                    .core
                    .state
                    .get_nonce()
            }

            /// Get the seed.
            #[inline]
            pub fn get_seed(&self) -> [u8; 32] {
                self.rng
                    .core
                    .state
                    .get_seed()
            }
        }

        impl CryptoRng for $ChaChaXRng {}

        impl From<$ChaChaXCore> for $ChaChaXRng {
            fn from(core: $ChaChaXCore) -> Self {
                $ChaChaXRng {
                    rng: BlockRng::new(core),
                }
            }
        }

        impl PartialEq<$ChaChaXRng> for $ChaChaXRng {
            fn eq(&self, rhs: &$ChaChaXRng) -> bool {
                let a: $abst::$ChaChaXRng = self.into();
                let b: $abst::$ChaChaXRng = rhs.into();
                a == b
            }
        }
        impl Eq for $ChaChaXRng {}

        #[cfg(feature = "serde1")]
        impl Serialize for $ChaChaXRng {
            fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
            where S: Serializer {
                $abst::$ChaChaXRng::from(self).serialize(s)
            }
        }
        #[cfg(feature = "serde1")]
        impl<'de> Deserialize<'de> for $ChaChaXRng {
            fn deserialize<D>(d: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
                $abst::$ChaChaXRng::deserialize(d).map(|x| Self::from(&x))
            }
        }

        mod $abst {
            #[cfg(feature = "serde1")] use serde::{Serialize, Deserialize};

            // The abstract state of a ChaCha stream, independent of implementation choices. The
            // comparison and serialization of this object is considered a semver-covered part of
            // the API.
            #[derive(Debug, PartialEq, Eq)]
            #[cfg_attr(
                feature = "serde1",
                derive(Serialize, Deserialize),
            )]
            pub(crate) struct $ChaChaXRng {
                seed: [u8; 32],
                stream: u64,
                word_pos: u128,
            }

            impl From<&super::$ChaChaXRng> for $ChaChaXRng {
                // Forget all information about the input except what is necessary to determine the
                // outputs of any sequence of pub API calls.
                fn from(r: &super::$ChaChaXRng) -> Self {
                    Self {
                        seed: r.get_seed(),
                        stream: r.get_stream(),
                        word_pos: r.get_word_pos(),
                    }
                }
            }

            impl From<&$ChaChaXRng> for super::$ChaChaXRng {
                // Construct one of the possible concrete RNGs realizing an abstract state.
                fn from(a: &$ChaChaXRng) -> Self {
                    use rand_core::SeedableRng;
                    let mut r = Self::from_seed(a.seed);
                    r.set_stream(a.stream);
                    r.set_word_pos(a.word_pos);
                    r
                }
            }
        }
    }
}

chacha_impl!(ChaCha20Core, ChaCha20Rng, 10, "ChaCha with 20 rounds", abstract20);
chacha_impl!(ChaCha12Core, ChaCha12Rng, 6, "ChaCha with 12 rounds", abstract12);
chacha_impl!(ChaCha8Core, ChaCha8Rng, 4, "ChaCha with 8 rounds", abstract8);

#[cfg(test)]
mod test {
    use rand_core::{RngCore, SeedableRng};

    #[cfg(feature = "serde1")] use super::{ChaCha20Rng, ChaCha12Rng, ChaCha8Rng};

    type ChaChaRng = super::ChaCha20Rng;

    #[cfg(feature = "serde1")]
    #[test]
    fn test_chacha_serde_roundtrip() {
        let seed = [
            1, 0, 52, 0, 0, 0, 0, 0, 1, 0, 10, 0, 22, 32, 0, 0, 2, 0, 55, 49, 0, 11, 0, 0, 3, 0, 0, 0, 0,
            0, 2, 92,
        ];
        let mut rng1 = ChaCha20Rng::from_seed(seed);
        let mut rng2 = ChaCha12Rng::from_seed(seed);
        let mut rng3 = ChaCha8Rng::from_seed(seed);

        let encoded1 = serde_json::to_string(&rng1).unwrap();
        let encoded2 = serde_json::to_string(&rng2).unwrap();
        let encoded3 = serde_json::to_string(&rng3).unwrap();

        let mut decoded1: ChaCha20Rng = serde_json::from_str(&encoded1).unwrap();
        let mut decoded2: ChaCha12Rng = serde_json::from_str(&encoded2).unwrap();
        let mut decoded3: ChaCha8Rng = serde_json::from_str(&encoded3).unwrap();

        assert_eq!(rng1, decoded1);
        assert_eq!(rng2, decoded2);
        assert_eq!(rng3, decoded3);

        assert_eq!(rng1.next_u32(), decoded1.next_u32());
        assert_eq!(rng2.next_u32(), decoded2.next_u32());
        assert_eq!(rng3.next_u32(), decoded3.next_u32());
    }

    // This test validates that:
    // 1. a hard-coded serialization demonstrating the format at time of initial release can still
    //    be deserialized to a ChaChaRng
    // 2. re-serializing the resultant object produces exactly the original string
    //
    // Condition 2 is stronger than necessary: an equivalent serialization (e.g. with field order
    // permuted, or whitespace differences) would also be admissible, but would fail this test.
    // However testing for equivalence of serialized data is difficult, and there shouldn't be any
    // reason we need to violate the stronger-than-needed condition, e.g. by changing the field
    // definition order.
    #[cfg(feature = "serde1")]
    #[test]
    fn test_chacha_serde_format_stability() {
        let j = r#"{"seed":[4,8,15,16,23,42,4,8,15,16,23,42,4,8,15,16,23,42,4,8,15,16,23,42,4,8,15,16,23,42,4,8],"stream":27182818284,"word_pos":314159265359}"#;
        let r: ChaChaRng = serde_json::from_str(&j).unwrap();
        let j1 = serde_json::to_string(&r).unwrap();
        assert_eq!(j, j1);
    }

    #[test]
    fn test_chacha_construction() {
        let seed = [
            0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let mut rng1 = ChaChaRng::from_seed(seed);
        assert_eq!(rng1.next_u32(), 137206642);

        let mut rng2 = ChaChaRng::from_rng(rng1).unwrap();
        assert_eq!(rng2.next_u32(), 1325750369);
    }

    #[test]
    fn test_chacha_true_values_a() {
        // Test vectors 1 and 2 from
        // https://tools.ietf.org/html/draft-nir-cfrg-chacha20-poly1305-04
        let seed = [0u8; 32];
        let mut rng = ChaChaRng::from_seed(seed);

        let mut results = [0u32; 16];
        for i in results.iter_mut() {
            *i = rng.next_u32();
        }
        let expected = [
            0xade0b876, 0x903df1a0, 0xe56a5d40, 0x28bd8653, 0xb819d2bd, 0x1aed8da0, 0xccef36a8,
            0xc70d778b, 0x7c5941da, 0x8d485751, 0x3fe02477, 0x374ad8b8, 0xf4b8436a, 0x1ca11815,
            0x69b687c3, 0x8665eeb2,
        ];
        assert_eq!(results, expected);

        for i in results.iter_mut() {
            *i = rng.next_u32();
        }
        let expected = [
            0xbee7079f, 0x7a385155, 0x7c97ba98, 0x0d082d73, 0xa0290fcb, 0x6965e348, 0x3e53c612,
            0xed7aee32, 0x7621b729, 0x434ee69c, 0xb03371d5, 0xd539d874, 0x281fed31, 0x45fb0a51,
            0x1f0ae1ac, 0x6f4d794b,
        ];
        assert_eq!(results, expected);
    }

    #[test]
    fn test_chacha_true_values_b() {
        // Test vector 3 from
        // https://tools.ietf.org/html/draft-nir-cfrg-chacha20-poly1305-04
        let seed = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 1,
        ];
        let mut rng = ChaChaRng::from_seed(seed);

        // Skip block 0
        for _ in 0..16 {
            rng.next_u32();
        }

        let mut results = [0u32; 16];
        for i in results.iter_mut() {
            *i = rng.next_u32();
        }
        let expected = [
            0x2452eb3a, 0x9249f8ec, 0x8d829d9b, 0xddd4ceb1, 0xe8252083, 0x60818b01, 0xf38422b8,
            0x5aaa49c9, 0xbb00ca8e, 0xda3ba7b4, 0xc4b592d1, 0xfdf2732f, 0x4436274e, 0x2561b3c8,
            0xebdd4aa6, 0xa0136c00,
        ];
        assert_eq!(results, expected);
    }

    #[test]
    fn test_chacha_true_values_c() {
        // Test vector 4 from
        // https://tools.ietf.org/html/draft-nir-cfrg-chacha20-poly1305-04
        let seed = [
            0, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        let expected = [
            0xfb4dd572, 0x4bc42ef1, 0xdf922636, 0x327f1394, 0xa78dea8f, 0x5e269039, 0xa1bebbc1,
            0xcaf09aae, 0xa25ab213, 0x48a6b46c, 0x1b9d9bcb, 0x092c5be6, 0x546ca624, 0x1bec45d5,
            0x87f47473, 0x96f0992e,
        ];
        let expected_end = 3 * 16;
        let mut results = [0u32; 16];

        // Test block 2 by skipping block 0 and 1
        let mut rng1 = ChaChaRng::from_seed(seed);
        for _ in 0..32 {
            rng1.next_u32();
        }
        for i in results.iter_mut() {
            *i = rng1.next_u32();
        }
        assert_eq!(results, expected);
        assert_eq!(rng1.get_word_pos(), expected_end);

        // Test block 2 by using `set_word_pos`
        let mut rng2 = ChaChaRng::from_seed(seed);
        rng2.set_word_pos(2 * 16);
        for i in results.iter_mut() {
            *i = rng2.next_u32();
        }
        assert_eq!(results, expected);
        assert_eq!(rng2.get_word_pos(), expected_end);

        // Test skipping behaviour with other types
        let mut buf = [0u8; 32];
        rng2.fill_bytes(&mut buf[..]);
        assert_eq!(rng2.get_word_pos(), expected_end + 8);
        rng2.fill_bytes(&mut buf[0..25]);
        assert_eq!(rng2.get_word_pos(), expected_end + 15);
        rng2.next_u64();
        assert_eq!(rng2.get_word_pos(), expected_end + 17);
        rng2.next_u32();
        rng2.next_u64();
        assert_eq!(rng2.get_word_pos(), expected_end + 20);
        rng2.fill_bytes(&mut buf[0..1]);
        assert_eq!(rng2.get_word_pos(), expected_end + 21);
    }

    #[test]
    fn test_chacha_multiple_blocks() {
        let seed = [
            0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0, 5, 0, 0, 0, 6, 0, 0, 0, 7,
            0, 0, 0,
        ];
        let mut rng = ChaChaRng::from_seed(seed);

        // Store the 17*i-th 32-bit word,
        // i.e., the i-th word of the i-th 16-word block
        let mut results = [0u32; 16];
        for i in results.iter_mut() {
            *i = rng.next_u32();
            for _ in 0..16 {
                rng.next_u32();
            }
        }
        let expected = [
            0xf225c81a, 0x6ab1be57, 0x04d42951, 0x70858036, 0x49884684, 0x64efec72, 0x4be2d186,
            0x3615b384, 0x11cfa18e, 0xd3c50049, 0x75c775f6, 0x434c6530, 0x2c5bad8f, 0x898881dc,
            0x5f1c86d9, 0xc1f8e7f4,
        ];
        assert_eq!(results, expected);
    }

    #[test]
    fn test_chacha_true_bytes() {
        let seed = [0u8; 32];
        let mut rng = ChaChaRng::from_seed(seed);
        let mut results = [0u8; 32];
        rng.fill_bytes(&mut results);
        let expected = [
            118, 184, 224, 173, 160, 241, 61, 144, 64, 93, 106, 229, 83, 134, 189, 40, 189, 210,
            25, 184, 160, 141, 237, 26, 168, 54, 239, 204, 139, 119, 13, 199,
        ];
        assert_eq!(results, expected);
    }

    #[test]
    fn test_chacha_nonce() {
        // Test vector 5 from
        // https://tools.ietf.org/html/draft-nir-cfrg-chacha20-poly1305-04
        // Although we do not support setting a nonce, we try it here anyway so
        // we can use this test vector.
        let seed = [0u8; 32];
        let mut rng = ChaChaRng::from_seed(seed);
        // 96-bit nonce in LE order is: 0,0,0,0, 0,0,0,0, 0,0,0,2
        rng.set_stream(2u64 << (24 + 32));

        let mut results = [0u32; 16];
        for i in results.iter_mut() {
            *i = rng.next_u32();
        }
        let expected = [
            0x374dc6c2, 0x3736d58c, 0xb904e24a, 0xcd3f93ef, 0x88228b1a, 0x96a4dfb3, 0x5b76ab72,
            0xc727ee54, 0x0e0e978a, 0xf3145c95, 0x1b748ea8, 0xf786c297, 0x99c28f5f, 0x628314e8,
            0x398a19fa, 0x6ded1b53,
        ];
        assert_eq!(results, expected);
    }

    #[test]
    fn test_chacha_clone_streams() {
        let seed = [
            0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0, 5, 0, 0, 0, 6, 0, 0, 0, 7,
            0, 0, 0,
        ];
        let mut rng = ChaChaRng::from_seed(seed);
        let mut clone = rng.clone();
        for _ in 0..16 {
            assert_eq!(rng.next_u64(), clone.next_u64());
        }

        rng.set_stream(51);
        for _ in 0..7 {
            assert!(rng.next_u32() != clone.next_u32());
        }
        clone.set_stream(51); // switch part way through block
        for _ in 7..16 {
            assert_eq!(rng.next_u32(), clone.next_u32());
        }
    }

    #[test]
    fn test_chacha_word_pos_wrap_exact() {
        use super::{BUF_BLOCKS, BLOCK_WORDS};
        let mut rng = ChaChaRng::from_seed(Default::default());
        // refilling the buffer in set_word_pos will wrap the block counter to 0
        let last_block = (1 << 68) - u128::from(BUF_BLOCKS * BLOCK_WORDS);
        rng.set_word_pos(last_block);
        assert_eq!(rng.get_word_pos(), last_block);
    }

    #[test]
    fn test_chacha_word_pos_wrap_excess() {
        use super::BLOCK_WORDS;
        let mut rng = ChaChaRng::from_seed(Default::default());
        // refilling the buffer in set_word_pos will wrap the block counter past 0
        let last_block = (1 << 68) - u128::from(BLOCK_WORDS);
        rng.set_word_pos(last_block);
        assert_eq!(rng.get_word_pos(), last_block);
    }

    #[test]
    fn test_chacha_word_pos_zero() {
        let mut rng = ChaChaRng::from_seed(Default::default());
        assert_eq!(rng.get_word_pos(), 0);
        rng.set_word_pos(0);
        assert_eq!(rng.get_word_pos(), 0);
    }
}
