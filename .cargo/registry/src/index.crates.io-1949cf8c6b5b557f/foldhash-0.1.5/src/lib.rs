//! This crate provides foldhash, a fast, non-cryptographic, minimally
//! DoS-resistant hashing algorithm designed for computational uses such as
//! hashmaps, bloom filters, count sketching, etc.
//!
//! When should you **not** use foldhash:
//!
//! - You are afraid of people studying your long-running program's behavior
//!   to reverse engineer its internal random state and using this knowledge to
//!   create many colliding inputs for computational complexity attacks.
//!
//! - You expect foldhash to have a consistent output across versions or
//!   platforms, such as for persistent file formats or communication protocols.
//!   
//! - You are relying on foldhash's properties for any kind of security.
//!   Foldhash is **not appropriate for any cryptographic purpose**.
//!
//! Foldhash has two variants, one optimized for speed which is ideal for data
//! structures such as hash maps and bloom filters, and one optimized for
//! statistical quality which is ideal for algorithms such as
//! [HyperLogLog](https://en.wikipedia.org/wiki/HyperLogLog) and
//! [MinHash](https://en.wikipedia.org/wiki/MinHash).
//!
//! Foldhash can be used in a `#![no_std]` environment by disabling its default
//! `"std"` feature.
//!
//! # Usage
//!
//! The easiest way to use this crate with the standard library [`HashMap`] or
//! [`HashSet`] is to import them from `foldhash` instead, along with the
//! extension traits to make [`HashMap::new`] and [`HashMap::with_capacity`]
//! work out-of-the-box:
//!
//! ```rust
//! use foldhash::{HashMap, HashMapExt};
//!
//! let mut hm = HashMap::new();
//! hm.insert(42, "hello");
//! ```
//!
//! You can also avoid the convenience types and do it manually by initializing
//! a [`RandomState`](fast::RandomState), for example if you are using a different hash map
//! implementation like [`hashbrown`](https://docs.rs/hashbrown/):
//!
//! ```rust
//! use hashbrown::HashMap;
//! use foldhash::fast::RandomState;
//!
//! let mut hm = HashMap::with_hasher(RandomState::default());
//! hm.insert("foo", "bar");
//! ```
//!
//! The above methods are the recommended way to use foldhash, which will
//! automatically generate a randomly generated hasher instance for you. If you
//! absolutely must have determinism you can use [`FixedState`](fast::FixedState)
//! instead, but note that this makes you trivially vulnerable to HashDoS
//! attacks and might lead to quadratic runtime when moving data from one
//! hashmap/set into another:
//!
//! ```rust
//! use std::collections::HashSet;
//! use foldhash::fast::FixedState;
//!
//! let mut hm = HashSet::with_hasher(FixedState::with_seed(42));
//! hm.insert([1, 10, 100]);
//! ```
//!
//! If you rely on statistical properties of the hash for the correctness of
//! your algorithm, such as in [HyperLogLog](https://en.wikipedia.org/wiki/HyperLogLog),
//! it is suggested to use the [`RandomState`](quality::RandomState)
//! or [`FixedState`](quality::FixedState) from the [`quality`] module instead
//! of the [`fast`] module. The latter is optimized purely for speed in hash
//! tables and has known statistical imperfections.
//!
//! Finally, you can also directly use the [`RandomState`](quality::RandomState)
//! or [`FixedState`](quality::FixedState) to manually hash items using the
//! [`BuildHasher`](std::hash::BuildHasher) trait:
//! ```rust
//! use std::hash::BuildHasher;
//! use foldhash::quality::RandomState;
//!
//! let random_state = RandomState::default();
//! let hash = random_state.hash_one("hello world");
//! ```
//!
//! ## Seeding
//!
//! Foldhash relies on a single 8-byte per-hasher seed which should be ideally
//! be different from each instance to instance, and also a larger
//! [`SharedSeed`] which may be shared by many different instances.
//!
//! To reduce overhead, this [`SharedSeed`] is typically initialized once and
//! stored. To prevent each hashmap unnecessarily containing a reference to this
//! value there are three kinds of [`BuildHasher`](core::hash::BuildHasher)s
//! foldhash provides (both for [`fast`] and [`quality`]):
//!
//! 1. [`RandomState`](fast::RandomState), which always generates a
//!    random per-hasher seed and implicitly stores a reference to [`SharedSeed::global_random`].
//! 2. [`FixedState`](fast::FixedState), which by default uses a fixed
//!    per-hasher seed and implicitly stores a reference to [`SharedSeed::global_fixed`].
//! 3. [`SeedableRandomState`](fast::SeedableRandomState), which works like
//!    [`RandomState`](fast::RandomState) by default but can be seeded in any manner.
//!    This state must include an explicit reference to a [`SharedSeed`], and thus
//!    this struct is 16 bytes as opposed to just 8 bytes for the previous two.

#![cfg_attr(all(not(test), not(feature = "std")), no_std)]
#![warn(missing_docs)]

pub mod fast;
pub mod quality;
mod seed;
pub use seed::SharedSeed;

#[cfg(feature = "std")]
mod convenience;
#[cfg(feature = "std")]
pub use convenience::*;

// Arbitrary constants with high entropy. Hexadecimal digits of pi were used.
const ARBITRARY0: u64 = 0x243f6a8885a308d3;
const ARBITRARY1: u64 = 0x13198a2e03707344;
const ARBITRARY2: u64 = 0xa4093822299f31d0;
const ARBITRARY3: u64 = 0x082efa98ec4e6c89;
const ARBITRARY4: u64 = 0x452821e638d01377;
const ARBITRARY5: u64 = 0xbe5466cf34e90c6c;
const ARBITRARY6: u64 = 0xc0ac29b7c97c50dd;
const ARBITRARY7: u64 = 0x3f84d5b5b5470917;
const ARBITRARY8: u64 = 0x9216d5d98979fb1b;
const ARBITRARY9: u64 = 0xd1310ba698dfb5ac;

#[inline(always)]
const fn folded_multiply(x: u64, y: u64) -> u64 {
    // The following code path is only fast if 64-bit to 128-bit widening
    // multiplication is supported by the architecture. Most 64-bit
    // architectures except SPARC64 and Wasm64 support it. However, the target
    // pointer width doesn't always indicate that we are dealing with a 64-bit
    // architecture, as there are ABIs that reduce the pointer width, especially
    // on AArch64 and x86-64. WebAssembly (regardless of pointer width) supports
    // 64-bit to 128-bit widening multiplication with the `wide-arithmetic`
    // proposal.
    #[cfg(any(
        all(
            target_pointer_width = "64",
            not(any(target_arch = "sparc64", target_arch = "wasm64")),
        ),
        target_arch = "aarch64",
        target_arch = "x86_64",
        all(target_family = "wasm", target_feature = "wide-arithmetic"),
    ))]
    {
        // We compute the full u64 x u64 -> u128 product, this is a single mul
        // instruction on x86-64, one mul plus one mulhi on ARM64.
        let full = (x as u128).wrapping_mul(y as u128);
        let lo = full as u64;
        let hi = (full >> 64) as u64;

        // The middle bits of the full product fluctuate the most with small
        // changes in the input. This is the top bits of lo and the bottom bits
        // of hi. We can thus make the entire output fluctuate with small
        // changes to the input by XOR'ing these two halves.
        lo ^ hi
    }

    #[cfg(not(any(
        all(
            target_pointer_width = "64",
            not(any(target_arch = "sparc64", target_arch = "wasm64")),
        ),
        target_arch = "aarch64",
        target_arch = "x86_64",
        all(target_family = "wasm", target_feature = "wide-arithmetic"),
    )))]
    {
        // u64 x u64 -> u128 product is quite expensive on 32-bit.
        // We approximate it by expanding the multiplication and eliminating
        // carries by replacing additions with XORs:
        //    (2^32 hx + lx)*(2^32 hy + ly) =
        //    2^64 hx*hy + 2^32 (hx*ly + lx*hy) + lx*ly ~=
        //    2^64 hx*hy ^ 2^32 (hx*ly ^ lx*hy) ^ lx*ly
        // Which when folded becomes:
        //    (hx*hy ^ lx*ly) ^ (hx*ly ^ lx*hy).rotate_right(32)

        let lx = x as u32;
        let ly = y as u32;
        let hx = (x >> 32) as u32;
        let hy = (y >> 32) as u32;

        let ll = (lx as u64).wrapping_mul(ly as u64);
        let lh = (lx as u64).wrapping_mul(hy as u64);
        let hl = (hx as u64).wrapping_mul(ly as u64);
        let hh = (hx as u64).wrapping_mul(hy as u64);

        (hh ^ ll) ^ (hl ^ lh).rotate_right(32)
    }
}

#[inline(always)]
const fn rotate_right(x: u64, r: u32) -> u64 {
    #[cfg(any(
        target_pointer_width = "64",
        target_arch = "aarch64",
        target_arch = "x86_64",
        target_family = "wasm",
    ))]
    {
        x.rotate_right(r)
    }

    #[cfg(not(any(
        target_pointer_width = "64",
        target_arch = "aarch64",
        target_arch = "x86_64",
        target_family = "wasm",
    )))]
    {
        // On platforms without 64-bit arithmetic rotation can be slow, rotate
        // each 32-bit half independently.
        let lo = (x as u32).rotate_right(r);
        let hi = ((x >> 32) as u32).rotate_right(r);
        ((hi as u64) << 32) | lo as u64
    }
}

/// Hashes strings >= 16 bytes, has unspecified behavior when bytes.len() < 16.
fn hash_bytes_medium(bytes: &[u8], mut s0: u64, mut s1: u64, fold_seed: u64) -> u64 {
    // Process 32 bytes per iteration, 16 bytes from the start, 16 bytes from
    // the end. On the last iteration these two chunks can overlap, but that is
    // perfectly fine.
    let left_to_right = bytes.chunks_exact(16);
    let mut right_to_left = bytes.rchunks_exact(16);
    for lo in left_to_right {
        let hi = right_to_left.next().unwrap();
        let unconsumed_start = lo.as_ptr();
        let unconsumed_end = hi.as_ptr_range().end;
        if unconsumed_start >= unconsumed_end {
            break;
        }

        let a = u64::from_ne_bytes(lo[0..8].try_into().unwrap());
        let b = u64::from_ne_bytes(lo[8..16].try_into().unwrap());
        let c = u64::from_ne_bytes(hi[0..8].try_into().unwrap());
        let d = u64::from_ne_bytes(hi[8..16].try_into().unwrap());
        s0 = folded_multiply(a ^ s0, c ^ fold_seed);
        s1 = folded_multiply(b ^ s1, d ^ fold_seed);
    }

    s0 ^ s1
}

/// Hashes strings >= 16 bytes, has unspecified behavior when bytes.len() < 16.
#[cold]
#[inline(never)]
fn hash_bytes_long(
    bytes: &[u8],
    mut s0: u64,
    mut s1: u64,
    mut s2: u64,
    mut s3: u64,
    fold_seed: u64,
) -> u64 {
    let chunks = bytes.chunks_exact(64);
    let remainder = chunks.remainder().len();
    for chunk in chunks {
        let a = u64::from_ne_bytes(chunk[0..8].try_into().unwrap());
        let b = u64::from_ne_bytes(chunk[8..16].try_into().unwrap());
        let c = u64::from_ne_bytes(chunk[16..24].try_into().unwrap());
        let d = u64::from_ne_bytes(chunk[24..32].try_into().unwrap());
        let e = u64::from_ne_bytes(chunk[32..40].try_into().unwrap());
        let f = u64::from_ne_bytes(chunk[40..48].try_into().unwrap());
        let g = u64::from_ne_bytes(chunk[48..56].try_into().unwrap());
        let h = u64::from_ne_bytes(chunk[56..64].try_into().unwrap());
        s0 = folded_multiply(a ^ s0, e ^ fold_seed);
        s1 = folded_multiply(b ^ s1, f ^ fold_seed);
        s2 = folded_multiply(c ^ s2, g ^ fold_seed);
        s3 = folded_multiply(d ^ s3, h ^ fold_seed);
    }
    s0 ^= s2;
    s1 ^= s3;

    if remainder > 0 {
        hash_bytes_medium(&bytes[bytes.len() - remainder.max(16)..], s0, s1, fold_seed)
    } else {
        s0 ^ s1
    }
}
