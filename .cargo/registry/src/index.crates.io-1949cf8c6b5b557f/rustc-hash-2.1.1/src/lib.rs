//! A speedy, non-cryptographic hashing algorithm used by `rustc`.
//!
//! # Example
//!
//! ```rust
//! # #[cfg(feature = "std")]
//! # fn main() {
//! use rustc_hash::FxHashMap;
//!
//! let mut map: FxHashMap<u32, u32> = FxHashMap::default();
//! map.insert(22, 44);
//! # }
//! # #[cfg(not(feature = "std"))]
//! # fn main() { }
//! ```

#![no_std]
#![cfg_attr(feature = "nightly", feature(hasher_prefixfree_extras))]

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "rand")]
extern crate rand;

#[cfg(feature = "rand")]
mod random_state;

mod seeded_state;

use core::default::Default;
use core::hash::{BuildHasher, Hasher};
#[cfg(feature = "std")]
use std::collections::{HashMap, HashSet};

/// Type alias for a hash map that uses the Fx hashing algorithm.
#[cfg(feature = "std")]
pub type FxHashMap<K, V> = HashMap<K, V, FxBuildHasher>;

/// Type alias for a hash set that uses the Fx hashing algorithm.
#[cfg(feature = "std")]
pub type FxHashSet<V> = HashSet<V, FxBuildHasher>;

#[cfg(feature = "rand")]
pub use random_state::{FxHashMapRand, FxHashSetRand, FxRandomState};

pub use seeded_state::FxSeededState;
#[cfg(feature = "std")]
pub use seeded_state::{FxHashMapSeed, FxHashSetSeed};

/// A speedy hash algorithm for use within rustc. The hashmap in liballoc
/// by default uses SipHash which isn't quite as speedy as we want. In the
/// compiler we're not really worried about DOS attempts, so we use a fast
/// non-cryptographic hash.
///
/// The current implementation is a fast polynomial hash with a single
/// bit rotation as a finishing step designed by Orson Peters.
#[derive(Clone)]
pub struct FxHasher {
    hash: usize,
}

// One might view a polynomial hash
//    m[0] * k    + m[1] * k^2  + m[2] * k^3  + ...
// as a multilinear hash with keystream k[..]
//    m[0] * k[0] + m[1] * k[1] + m[2] * k[2] + ...
// where keystream k just happens to be generated using a multiplicative
// congruential pseudorandom number generator (MCG). For that reason we chose a
// constant that was found to be good for a MCG in:
//     "Computationally Easy, Spectrally Good Multipliers for Congruential
//     Pseudorandom Number Generators" by Guy Steele and Sebastiano Vigna.
#[cfg(target_pointer_width = "64")]
const K: usize = 0xf1357aea2e62a9c5;
#[cfg(target_pointer_width = "32")]
const K: usize = 0x93d765dd;

impl FxHasher {
    /// Creates a `fx` hasher with a given seed.
    pub const fn with_seed(seed: usize) -> FxHasher {
        FxHasher { hash: seed }
    }

    /// Creates a default `fx` hasher.
    pub const fn default() -> FxHasher {
        FxHasher { hash: 0 }
    }
}

impl Default for FxHasher {
    #[inline]
    fn default() -> FxHasher {
        Self::default()
    }
}

impl FxHasher {
    #[inline]
    fn add_to_hash(&mut self, i: usize) {
        self.hash = self.hash.wrapping_add(i).wrapping_mul(K);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        // Compress the byte string to a single u64 and add to our hash.
        self.write_u64(hash_bytes(bytes));
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add_to_hash(i as usize);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add_to_hash(i as usize);
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add_to_hash(i as usize);
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add_to_hash(i as usize);
        #[cfg(target_pointer_width = "32")]
        self.add_to_hash((i >> 32) as usize);
    }

    #[inline]
    fn write_u128(&mut self, i: u128) {
        self.add_to_hash(i as usize);
        #[cfg(target_pointer_width = "32")]
        self.add_to_hash((i >> 32) as usize);
        self.add_to_hash((i >> 64) as usize);
        #[cfg(target_pointer_width = "32")]
        self.add_to_hash((i >> 96) as usize);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add_to_hash(i);
    }

    #[cfg(feature = "nightly")]
    #[inline]
    fn write_length_prefix(&mut self, _len: usize) {
        // Most cases will specialize hash_slice to call write(), which encodes
        // the length already in a more efficient manner than we could here. For
        // HashDoS-resistance you would still need to include this for the
        // non-slice collection hashes, but for the purposes of rustc we do not
        // care and do not wish to pay the performance penalty of mixing in len
        // for those collections.
    }

    #[cfg(feature = "nightly")]
    #[inline]
    fn write_str(&mut self, s: &str) {
        // Similarly here, write already encodes the length, so nothing special
        // is needed.
        self.write(s.as_bytes())
    }

    #[inline]
    fn finish(&self) -> u64 {
        // Since we used a multiplicative hash our top bits have the most
        // entropy (with the top bit having the most, decreasing as you go).
        // As most hash table implementations (including hashbrown) compute
        // the bucket index from the bottom bits we want to move bits from the
        // top to the bottom. Ideally we'd rotate left by exactly the hash table
        // size, but as we don't know this we'll choose 26 bits, giving decent
        // entropy up until 2^26 table sizes. On 32-bit hosts we'll dial it
        // back down a bit to 15 bits.

        #[cfg(target_pointer_width = "64")]
        const ROTATE: u32 = 26;
        #[cfg(target_pointer_width = "32")]
        const ROTATE: u32 = 15;

        self.hash.rotate_left(ROTATE) as u64

        // A bit reversal would be even better, except hashbrown also expects
        // good entropy in the top 7 bits and a bit reverse would fill those
        // bits with low entropy. More importantly, bit reversals are very slow
        // on x86-64. A byte reversal is relatively fast, but still has a 2
        // cycle latency on x86-64 compared to the 1 cycle latency of a rotate.
        // It also suffers from the hashbrown-top-7-bit-issue.
    }
}

// Nothing special, digits of pi.
const SEED1: u64 = 0x243f6a8885a308d3;
const SEED2: u64 = 0x13198a2e03707344;
const PREVENT_TRIVIAL_ZERO_COLLAPSE: u64 = 0xa4093822299f31d0;

#[inline]
fn multiply_mix(x: u64, y: u64) -> u64 {
    #[cfg(target_pointer_width = "64")]
    {
        // We compute the full u64 x u64 -> u128 product, this is a single mul
        // instruction on x86-64, one mul plus one mulhi on ARM64.
        let full = (x as u128) * (y as u128);
        let lo = full as u64;
        let hi = (full >> 64) as u64;

        // The middle bits of the full product fluctuate the most with small
        // changes in the input. This is the top bits of lo and the bottom bits
        // of hi. We can thus make the entire output fluctuate with small
        // changes to the input by XOR'ing these two halves.
        lo ^ hi

        // Unfortunately both 2^64 + 1 and 2^64 - 1 have small prime factors,
        // otherwise combining with + or - could result in a really strong hash, as:
        //     x * y = 2^64 * hi + lo = (-1) * hi + lo = lo - hi,   (mod 2^64 + 1)
        //     x * y = 2^64 * hi + lo =    1 * hi + lo = lo + hi,   (mod 2^64 - 1)
        // Multiplicative hashing is universal in a field (like mod p).
    }

    #[cfg(target_pointer_width = "32")]
    {
        // u64 x u64 -> u128 product is prohibitively expensive on 32-bit.
        // Decompose into 32-bit parts.
        let lx = x as u32;
        let ly = y as u32;
        let hx = (x >> 32) as u32;
        let hy = (y >> 32) as u32;

        // u32 x u32 -> u64 the low bits of one with the high bits of the other.
        let afull = (lx as u64) * (hy as u64);
        let bfull = (hx as u64) * (ly as u64);

        // Combine, swapping low/high of one of them so the upper bits of the
        // product of one combine with the lower bits of the other.
        afull ^ bfull.rotate_right(32)
    }
}

/// A wyhash-inspired non-collision-resistant hash for strings/slices designed
/// by Orson Peters, with a focus on small strings and small codesize.
///
/// The 64-bit version of this hash passes the SMHasher3 test suite on the full
/// 64-bit output, that is, f(hash_bytes(b) ^ f(seed)) for some good avalanching
/// permutation f() passed all tests with zero failures. When using the 32-bit
/// version of multiply_mix this hash has a few non-catastrophic failures where
/// there are a handful more collisions than an optimal hash would give.
///
/// We don't bother avalanching here as we'll feed this hash into a
/// multiplication after which we take the high bits, which avalanches for us.
#[inline]
fn hash_bytes(bytes: &[u8]) -> u64 {
    let len = bytes.len();
    let mut s0 = SEED1;
    let mut s1 = SEED2;

    if len <= 16 {
        // XOR the input into s0, s1.
        if len >= 8 {
            s0 ^= u64::from_le_bytes(bytes[0..8].try_into().unwrap());
            s1 ^= u64::from_le_bytes(bytes[len - 8..].try_into().unwrap());
        } else if len >= 4 {
            s0 ^= u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as u64;
            s1 ^= u32::from_le_bytes(bytes[len - 4..].try_into().unwrap()) as u64;
        } else if len > 0 {
            let lo = bytes[0];
            let mid = bytes[len / 2];
            let hi = bytes[len - 1];
            s0 ^= lo as u64;
            s1 ^= ((hi as u64) << 8) | mid as u64;
        }
    } else {
        // Handle bulk (can partially overlap with suffix).
        let mut off = 0;
        while off < len - 16 {
            let x = u64::from_le_bytes(bytes[off..off + 8].try_into().unwrap());
            let y = u64::from_le_bytes(bytes[off + 8..off + 16].try_into().unwrap());

            // Replace s1 with a mix of s0, x, and y, and s0 with s1.
            // This ensures the compiler can unroll this loop into two
            // independent streams, one operating on s0, the other on s1.
            //
            // Since zeroes are a common input we prevent an immediate trivial
            // collapse of the hash function by XOR'ing a constant with y.
            let t = multiply_mix(s0 ^ x, PREVENT_TRIVIAL_ZERO_COLLAPSE ^ y);
            s0 = s1;
            s1 = t;
            off += 16;
        }

        let suffix = &bytes[len - 16..];
        s0 ^= u64::from_le_bytes(suffix[0..8].try_into().unwrap());
        s1 ^= u64::from_le_bytes(suffix[8..16].try_into().unwrap());
    }

    multiply_mix(s0, s1) ^ (len as u64)
}

/// An implementation of [`BuildHasher`] that produces [`FxHasher`]s.
///
/// ```
/// use std::hash::BuildHasher;
/// use rustc_hash::FxBuildHasher;
/// assert_ne!(FxBuildHasher.hash_one(1), FxBuildHasher.hash_one(2));
/// ```
#[derive(Copy, Clone, Default)]
pub struct FxBuildHasher;

impl BuildHasher for FxBuildHasher {
    type Hasher = FxHasher;
    fn build_hasher(&self) -> FxHasher {
        FxHasher::default()
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(any(target_pointer_width = "64", target_pointer_width = "32")))]
    compile_error!("The test suite only supports 64 bit and 32 bit usize");

    use crate::{FxBuildHasher, FxHasher};
    use core::hash::{BuildHasher, Hash, Hasher};

    macro_rules! test_hash {
        (
            $(
                hash($value:expr) == $result:expr,
            )*
        ) => {
            $(
                assert_eq!(FxBuildHasher.hash_one($value), $result);
            )*
        };
    }

    const B32: bool = cfg!(target_pointer_width = "32");

    #[test]
    fn unsigned() {
        test_hash! {
            hash(0_u8) == 0,
            hash(1_u8) == if B32 { 3001993707 } else { 12157901119326311915 },
            hash(100_u8) == if B32 { 3844759569 } else { 16751747135202103309 },
            hash(u8::MAX) == if B32 { 999399879 } else { 1211781028898739645 },

            hash(0_u16) == 0,
            hash(1_u16) == if B32 { 3001993707 } else { 12157901119326311915 },
            hash(100_u16) == if B32 { 3844759569 } else { 16751747135202103309 },
            hash(u16::MAX) == if B32 { 3440503042 } else { 16279819243059860173 },

            hash(0_u32) == 0,
            hash(1_u32) == if B32 { 3001993707 } else { 12157901119326311915 },
            hash(100_u32) == if B32 { 3844759569 } else { 16751747135202103309 },
            hash(u32::MAX) == if B32 { 1293006356 } else { 7729994835221066939 },

            hash(0_u64) == 0,
            hash(1_u64) == if B32 { 275023839 } else { 12157901119326311915 },
            hash(100_u64) == if B32 { 1732383522 } else { 16751747135202103309 },
            hash(u64::MAX) == if B32 { 1017982517 } else { 6288842954450348564 },

            hash(0_u128) == 0,
            hash(1_u128) == if B32 { 1860738631 } else { 13032756267696824044 },
            hash(100_u128) == if B32 { 1389515751 } else { 12003541609544029302 },
            hash(u128::MAX) == if B32 { 2156022013 } else { 11702830760530184999 },

            hash(0_usize) == 0,
            hash(1_usize) == if B32 { 3001993707 } else { 12157901119326311915 },
            hash(100_usize) == if B32 { 3844759569 } else { 16751747135202103309 },
            hash(usize::MAX) == if B32 { 1293006356 } else { 6288842954450348564 },
        }
    }

    #[test]
    fn signed() {
        test_hash! {
            hash(i8::MIN) == if B32 { 2000713177 } else { 6684841074112525780 },
            hash(0_i8) == 0,
            hash(1_i8) == if B32 { 3001993707 } else { 12157901119326311915 },
            hash(100_i8) == if B32 { 3844759569 } else { 16751747135202103309 },
            hash(i8::MAX) == if B32 { 3293686765 } else { 12973684028562874344 },

            hash(i16::MIN) == if B32 { 1073764727 } else { 14218860181193086044 },
            hash(0_i16) == 0,
            hash(1_i16) == if B32 { 3001993707 } else { 12157901119326311915 },
            hash(100_i16) == if B32 { 3844759569 } else { 16751747135202103309 },
            hash(i16::MAX) == if B32 { 2366738315 } else { 2060959061933882993 },

            hash(i32::MIN) == if B32 { 16384 } else { 9943947977240134995 },
            hash(0_i32) == 0,
            hash(1_i32) == if B32 { 3001993707 } else { 12157901119326311915 },
            hash(100_i32) == if B32 { 3844759569 } else { 16751747135202103309 },
            hash(i32::MAX) == if B32 { 1293022740 } else { 16232790931690483559 },

            hash(i64::MIN) == if B32 { 16384 } else { 33554432 },
            hash(0_i64) == 0,
            hash(1_i64) == if B32 { 275023839 } else { 12157901119326311915 },
            hash(100_i64) == if B32 { 1732383522 } else { 16751747135202103309 },
            hash(i64::MAX) == if B32 { 1017998901 } else { 6288842954483902996 },

            hash(i128::MIN) == if B32 { 16384 } else { 33554432 },
            hash(0_i128) == 0,
            hash(1_i128) == if B32 { 1860738631 } else { 13032756267696824044 },
            hash(100_i128) == if B32 { 1389515751 } else { 12003541609544029302 },
            hash(i128::MAX) == if B32 { 2156005629 } else { 11702830760496630567 },

            hash(isize::MIN) == if B32 { 16384 } else { 33554432 },
            hash(0_isize) == 0,
            hash(1_isize) == if B32 { 3001993707 } else { 12157901119326311915 },
            hash(100_isize) == if B32 { 3844759569 } else { 16751747135202103309 },
            hash(isize::MAX) == if B32 { 1293022740 } else { 6288842954483902996 },
        }
    }

    // Avoid relying on any `Hash` implementations in the standard library.
    struct HashBytes(&'static [u8]);
    impl Hash for HashBytes {
        fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
            state.write(self.0);
        }
    }

    #[test]
    fn bytes() {
        test_hash! {
            hash(HashBytes(&[])) == if B32 { 2673204745 } else { 17606491139363777937 },
            hash(HashBytes(&[0])) == if B32 { 2948228584 } else { 5448590020104574886 },
            hash(HashBytes(&[0, 0, 0, 0, 0, 0])) == if B32 { 3223252423 } else { 16766921560080789783 },
            hash(HashBytes(&[1])) == if B32 { 2943445104 } else { 5922447956811044110 },
            hash(HashBytes(&[2])) == if B32 { 1055423297 } else { 5229781508510959783 },
            hash(HashBytes(b"uwu")) == if B32 { 2699662140 } else { 7168164714682931527 },
            hash(HashBytes(b"These are some bytes for testing rustc_hash.")) == if B32 { 2303640537 } else { 2349210501944688211 },
        }
    }

    #[test]
    fn with_seed_actually_different() {
        let seeds = [
            [1, 2],
            [42, 17],
            [124436707, 99237],
            [usize::MIN, usize::MAX],
        ];

        for [a_seed, b_seed] in seeds {
            let a = || FxHasher::with_seed(a_seed);
            let b = || FxHasher::with_seed(b_seed);

            for x in u8::MIN..=u8::MAX {
                let mut a = a();
                let mut b = b();

                x.hash(&mut a);
                x.hash(&mut b);

                assert_ne!(a.finish(), b.finish())
            }
        }
    }
}
