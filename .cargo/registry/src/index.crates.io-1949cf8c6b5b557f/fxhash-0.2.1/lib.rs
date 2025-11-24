// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![deny(missing_docs)]

//! # Fx Hash
//!
//! This hashing algorithm was extracted from the Rustc compiler.  This is the same hashing
//! algoirthm used for some internal operations in FireFox.  The strength of this algorithm
//! is in hashing 8 bytes at a time on 64-bit platforms, where the FNV algorithm works on one
//! byte at a time.
//!
//! ## Disclaimer
//!
//! It is **not a cryptographically secure** hash, so it is strongly recommended that you do
//! not use this hash for cryptographic purproses.  Furthermore, this hashing algorithm was
//! not designed to prevent any attacks for determining collisions which could be used to
//! potentially cause quadratic behavior in `HashMap`s.  So it is not recommended to expose
//! this hash in places where collissions or DDOS attacks may be a concern.

use std::collections::{HashMap, HashSet};
use std::default::Default;
use std::hash::{Hasher, Hash, BuildHasherDefault};
use std::ops::BitXor;

extern crate byteorder;
use byteorder::{ByteOrder, NativeEndian};

/// A builder for default Fx hashers.
pub type FxBuildHasher = BuildHasherDefault<FxHasher>;

/// A `HashMap` using a default Fx hasher.
pub type FxHashMap<K, V> = HashMap<K, V, FxBuildHasher>;

/// A `HashSet` using a default Fx hasher.
pub type FxHashSet<V> = HashSet<V, FxBuildHasher>;

const ROTATE: u32 = 5;
const SEED64: u64 = 0x517cc1b727220a95;
const SEED32: u32 = (SEED64 & 0xFFFF_FFFF) as u32;

#[cfg(target_pointer_width = "32")]
const SEED: usize = SEED32 as usize;
#[cfg(target_pointer_width = "64")]
const SEED: usize = SEED64 as usize;

trait HashWord {
    fn hash_word(&mut self, Self);
}

macro_rules! impl_hash_word {
    ($($ty:ty = $key:ident),* $(,)*) => (
        $(
            impl HashWord for $ty {
                #[inline]
                fn hash_word(&mut self, word: Self) {
                    *self = self.rotate_left(ROTATE).bitxor(word).wrapping_mul($key);
                }
            }
        )*
    )
}

impl_hash_word!(usize = SEED, u32 = SEED32, u64 = SEED64);

#[inline]
fn write32(mut hash: u32, mut bytes: &[u8]) -> u32 {
    while bytes.len() >= 4 {
        let n = NativeEndian::read_u32(bytes);
        hash.hash_word(n);
        bytes = bytes.split_at(4).1;
    }

    for byte in bytes {
        hash.hash_word(*byte as u32);
    }
    hash
}

#[inline]
fn write64(mut hash: u64, mut bytes: &[u8]) -> u64 {
    while bytes.len() >= 8 {
        let n = NativeEndian::read_u64(bytes);
        hash.hash_word(n);
        bytes = bytes.split_at(8).1;
    }

    if bytes.len() >= 4 {
        let n = NativeEndian::read_u32(bytes);
        hash.hash_word(n as u64);
        bytes = bytes.split_at(4).1;
    }

    for byte in bytes {
        hash.hash_word(*byte as u64);
    }
    hash
}

#[inline]
#[cfg(target_pointer_width = "32")]
fn write(hash: usize, bytes: &[u8]) -> usize {
    write32(hash as u32, bytes) as usize
}

#[inline]
#[cfg(target_pointer_width = "64")]
fn write(hash: usize, bytes: &[u8]) -> usize {
    write64(hash as u64, bytes) as usize
}

/// This hashing algorithm was extracted from the Rustc compiler.
/// This is the same hashing algoirthm used for some internal operations in FireFox.
/// The strength of this algorithm is in hashing 8 bytes at a time on 64-bit platforms,
/// where the FNV algorithm works on one byte at a time.
///
/// This hashing algorithm should not be used for cryptographic, or in scenarios where
/// DOS attacks are a concern.
#[derive(Debug, Clone)]
pub struct FxHasher {
    hash: usize,
}

impl Default for FxHasher {
    #[inline]
    fn default() -> FxHasher {
        FxHasher { hash: 0 }
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.hash = write(self.hash, bytes);
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.hash.hash_word(i as usize);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.hash.hash_word(i as usize);
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.hash.hash_word(i as usize);
    }

    #[inline]
    #[cfg(target_pointer_width = "32")]
    fn write_u64(&mut self, i: u64) {
        self.hash.hash_word(i as usize);
        self.hash.hash_word((i >> 32) as usize);
    }

    #[inline]
    #[cfg(target_pointer_width = "64")]
    fn write_u64(&mut self, i: u64) {
        self.hash.hash_word(i as usize);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.hash.hash_word(i);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash as u64
    }
}

/// This hashing algorithm was extracted from the Rustc compiler.
/// This is the same hashing algoirthm used for some internal operations in FireFox.
/// The strength of this algorithm is in hashing 8 bytes at a time on any platform,
/// where the FNV algorithm works on one byte at a time.
///
/// This hashing algorithm should not be used for cryptographic, or in scenarios where
/// DOS attacks are a concern.
#[derive(Debug, Clone)]
pub struct FxHasher64 {
    hash: u64,
}

impl Default for FxHasher64 {
    #[inline]
    fn default() -> FxHasher64 {
        FxHasher64 { hash: 0 }
    }
}

impl Hasher for FxHasher64 {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.hash = write64(self.hash, bytes);
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.hash.hash_word(i as u64);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.hash.hash_word(i as u64);
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.hash.hash_word(i as u64);
    }

    fn write_u64(&mut self, i: u64) {
        self.hash.hash_word(i);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.hash.hash_word(i as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

/// This hashing algorithm was extracted from the Rustc compiler.
/// This is the same hashing algoirthm used for some internal operations in FireFox.
/// The strength of this algorithm is in hashing 4 bytes at a time on any platform,
/// where the FNV algorithm works on one byte at a time.
///
/// This hashing algorithm should not be used for cryptographic, or in scenarios where
/// DOS attacks are a concern.
#[derive(Debug, Clone)]
pub struct FxHasher32 {
    hash: u32,
}

impl Default for FxHasher32 {
    #[inline]
    fn default() -> FxHasher32 {
        FxHasher32 { hash: 0 }
    }
}

impl Hasher for FxHasher32 {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.hash = write32(self.hash, bytes);
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.hash.hash_word(i as u32);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.hash.hash_word(i as u32);
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.hash.hash_word(i);
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.hash.hash_word(i as u32);
        self.hash.hash_word((i >> 32) as u32);
    }

    #[inline]
    #[cfg(target_pointer_width = "32")]
    fn write_usize(&mut self, i: usize) {
        self.write_u32(i as u32);
    }

    #[inline]
    #[cfg(target_pointer_width = "64")]
    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash as u64
    }
}

/// A convenience function for when you need a quick 64-bit hash.
#[inline]
pub fn hash64<T: Hash + ?Sized>(v: &T) -> u64 {
    let mut state = FxHasher64::default();
    v.hash(&mut state);
    state.finish()
}

/// A convenience function for when you need a quick 32-bit hash.
#[inline]
pub fn hash32<T: Hash + ?Sized>(v: &T) -> u32 {
    let mut state = FxHasher32::default();
    v.hash(&mut state);
    state.finish() as u32
}

/// A convenience function for when you need a quick usize hash.
#[inline]
pub fn hash<T: Hash + ?Sized>(v: &T) -> usize {
    let mut state = FxHasher::default();
    v.hash(&mut state);
    state.finish() as usize
}