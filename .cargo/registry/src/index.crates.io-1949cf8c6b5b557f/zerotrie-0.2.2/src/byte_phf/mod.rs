// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

#![allow(rustdoc::private_intra_doc_links)] // doc(hidden) module

//! # Byte Perfect Hash Function Internals
//!
//! This module contains a perfect hash function (PHF) designed for a fast, compact perfect
//! hash over 1 to 256 nodes (bytes).
//!
//! The PHF uses the following variables:
//!
//! 1. A single parameter `p`, which is 0 in about 98% of cases.
//! 2. A list of `N` parameters `q_t`, one per _bucket_
//! 3. The `N` keys in an arbitrary order determined by the PHF
//!
//! Reading a `key` from the PHF uses the following algorithm:
//!
//! 1. Let `t`, the bucket index, be `f1(key, p)`.
//! 2. Let `i`, the key index, be `f2(key, q_t)`.
//! 3. If `key == k_i`, return `Some(i)`; else return `None`.
//!
//! The functions [`f1`] and [`f2`] are internal to the PHF but should remain stable across
//! serialization versions of `ZeroTrie`. They are very fast, constant-time operations as long
//! as `p` <= [`P_FAST_MAX`] and `q` <= [`Q_FAST_MAX`]. In practice, nearly 100% of parameter
//! values are in the fast range.
//!
//! ```
//! use zerotrie::_internal::PerfectByteHashMap;
//!
//! let phf_example_bytes = [
//!     // `p` parameter
//!     1, // `q` parameters, one for each of the N buckets
//!     0, 0, 1, 1, // Exact keys to be compared with the input
//!     b'e', b'a', b'c', b'g',
//! ];
//!
//! let phf = PerfectByteHashMap::from_bytes(&phf_example_bytes);
//!
//! // The PHF returns the index of the key or `None` if not found.
//! assert_eq!(phf.get(b'a'), Some(1));
//! assert_eq!(phf.get(b'b'), None);
//! assert_eq!(phf.get(b'c'), Some(2));
//! assert_eq!(phf.get(b'd'), None);
//! assert_eq!(phf.get(b'e'), Some(0));
//! assert_eq!(phf.get(b'f'), None);
//! assert_eq!(phf.get(b'g'), Some(3));
//! ```

use crate::helpers::*;

#[cfg(feature = "alloc")]
mod builder;
#[cfg(feature = "alloc")]
mod cached_owned;

#[cfg(feature = "alloc")]
pub use cached_owned::PerfectByteHashMapCacheOwned;

/// The cutoff for the fast version of [`f1`].
#[cfg(feature = "alloc")] // used in the builder code
const P_FAST_MAX: u8 = 95;

/// The cutoff for the fast version of [`f2`].
const Q_FAST_MAX: u8 = 95;

/// The maximum allowable value of `p`. This could be raised if found to be necessary.
/// Values exceeding P_FAST_MAX could use a different `p` algorithm by modifying [`f1`].
#[cfg(feature = "alloc")] // used in the builder code
const P_REAL_MAX: u8 = P_FAST_MAX;

/// The maximum allowable value of `q`. This could be raised if found to be necessary.
#[cfg(feature = "alloc")] // used in the builder code
const Q_REAL_MAX: u8 = 127;

/// Calculates the function `f1` for the PHF. For the exact formula, please read the code.
///
/// When `p == 0`, the operation is a simple modulus.
///
/// The argument `n` is used only for taking the modulus so that the return value is
/// in the range `[0, n)`.
///
/// # Examples
///
/// ```
/// use zerotrie::_internal::f1;
/// const N: u8 = 10;
///
/// // With p = 0:
/// assert_eq!(0, f1(0, 0, N));
/// assert_eq!(1, f1(1, 0, N));
/// assert_eq!(2, f1(2, 0, N));
/// assert_eq!(9, f1(9, 0, N));
/// assert_eq!(0, f1(10, 0, N));
/// assert_eq!(1, f1(11, 0, N));
/// assert_eq!(2, f1(12, 0, N));
/// assert_eq!(9, f1(19, 0, N));
///
/// // With p = 1:
/// assert_eq!(1, f1(0, 1, N));
/// assert_eq!(0, f1(1, 1, N));
/// assert_eq!(2, f1(2, 1, N));
/// assert_eq!(2, f1(9, 1, N));
/// assert_eq!(4, f1(10, 1, N));
/// assert_eq!(5, f1(11, 1, N));
/// assert_eq!(1, f1(12, 1, N));
/// assert_eq!(7, f1(19, 1, N));
/// ```
#[inline]
pub fn f1(byte: u8, p: u8, n: u8) -> u8 {
    if n == 0 {
        byte
    } else if p == 0 {
        byte % n
    } else {
        // `p` always uses the below constant-time operation. If needed, we
        // could add some other operation here with `p > P_FAST_MAX` to solve
        // difficult cases if the need arises.
        let result = byte ^ p ^ byte.wrapping_shr(p as u32);
        result % n
    }
}

/// Calculates the function `f2` for the PHF. For the exact formula, please read the code.
///
/// When `q == 0`, the operation is a simple modulus.
///
/// The argument `n` is used only for taking the modulus so that the return value is
/// in the range `[0, n)`.
///
/// # Examples
///
/// ```
/// use zerotrie::_internal::f2;
/// const N: u8 = 10;
///
/// // With q = 0:
/// assert_eq!(0, f2(0, 0, N));
/// assert_eq!(1, f2(1, 0, N));
/// assert_eq!(2, f2(2, 0, N));
/// assert_eq!(9, f2(9, 0, N));
/// assert_eq!(0, f2(10, 0, N));
/// assert_eq!(1, f2(11, 0, N));
/// assert_eq!(2, f2(12, 0, N));
/// assert_eq!(9, f2(19, 0, N));
///
/// // With q = 1:
/// assert_eq!(1, f2(0, 1, N));
/// assert_eq!(0, f2(1, 1, N));
/// assert_eq!(3, f2(2, 1, N));
/// assert_eq!(8, f2(9, 1, N));
/// assert_eq!(1, f2(10, 1, N));
/// assert_eq!(0, f2(11, 1, N));
/// assert_eq!(3, f2(12, 1, N));
/// assert_eq!(8, f2(19, 1, N));
/// ```
#[inline]
pub fn f2(byte: u8, q: u8, n: u8) -> u8 {
    if n == 0 {
        return byte;
    }
    let mut result = byte ^ q;
    // In almost all cases, the PHF works with the above constant-time operation.
    // However, to crack a few difficult cases, we fall back to the linear-time
    // operation shown below.
    for _ in Q_FAST_MAX..q {
        result = result ^ (result << 1) ^ (result >> 1);
    }
    result % n
}

/// A constant-time map from bytes to unique indices.
///
/// Uses a perfect hash function (see module-level documentation). Does not support mutation.
///
/// Standard layout: P, N bytes of Q, N bytes of expected keys
#[derive(Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct PerfectByteHashMap<Store: ?Sized>(Store);

impl<Store> PerfectByteHashMap<Store> {
    /// Creates an instance from a pre-existing store. See [`Self::as_bytes`].
    #[inline]
    pub fn from_store(store: Store) -> Self {
        Self(store)
    }
}

impl<Store> PerfectByteHashMap<Store>
where
    Store: AsRef<[u8]> + ?Sized,
{
    /// Gets the usize for the given byte, or `None` if it is not in the map.
    pub fn get(&self, key: u8) -> Option<usize> {
        let (p, buffer) = self.0.as_ref().split_first()?;
        // Note: there are N buckets followed by N keys
        let n_usize = buffer.len() / 2;
        if n_usize == 0 {
            return None;
        }
        let n = n_usize as u8;
        let (qq, eks) = buffer.debug_split_at(n_usize);
        debug_assert_eq!(qq.len(), eks.len());
        let l1 = f1(key, *p, n) as usize;
        let q = debug_unwrap!(qq.get(l1), return None);
        let l2 = f2(key, *q, n) as usize;
        let ek = debug_unwrap!(eks.get(l2), return None);
        if *ek == key {
            Some(l2)
        } else {
            None
        }
    }
    /// This is called `num_items` because `len` is ambiguous: it could refer
    /// to the number of items or the number of bytes.
    pub fn num_items(&self) -> usize {
        self.0.as_ref().len() / 2
    }
    /// Get an iterator over the keys in the order in which they are stored in the map.
    pub fn keys(&self) -> &[u8] {
        let n = self.num_items();
        self.0.as_ref().debug_split_at(1 + n).1
    }
    /// Diagnostic function that returns `p` and the maximum value of `q`
    #[cfg(test)]
    pub fn p_qmax(&self) -> Option<(u8, u8)> {
        let (p, buffer) = self.0.as_ref().split_first()?;
        let n = buffer.len() / 2;
        if n == 0 {
            return None;
        }
        let (qq, _) = buffer.debug_split_at(n);
        Some((*p, *qq.iter().max().unwrap()))
    }
    /// Returns the map as bytes. The map can be recovered with [`Self::from_store`]
    /// or [`Self::from_bytes`].
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }

    #[cfg(all(feature = "alloc", test))]
    pub(crate) fn check(&self) -> Result<(), (&'static str, u8)> {
        use alloc::vec;
        let len = self.num_items();
        let mut seen = vec![false; len];
        for b in 0..=255u8 {
            let get_result = self.get(b);
            if self.keys().contains(&b) {
                let i = get_result.ok_or(("expected to find", b))?;
                if seen[i] {
                    return Err(("seen", b));
                }
                seen[i] = true;
            } else if get_result.is_some() {
                return Err(("did not expect to find", b));
            }
        }
        Ok(())
    }
}

impl PerfectByteHashMap<[u8]> {
    /// Creates an instance from pre-existing bytes. See [`Self::as_bytes`].
    #[inline]
    pub fn from_bytes(bytes: &[u8]) -> &Self {
        // Safety: Self is repr(transparent) over [u8]
        unsafe { core::mem::transmute(bytes) }
    }
}

impl<Store> PerfectByteHashMap<Store>
where
    Store: AsRef<[u8]> + ?Sized,
{
    /// Converts from `PerfectByteHashMap<AsRef<[u8]>>` to `&PerfectByteHashMap<[u8]>`
    #[inline]
    pub fn as_borrowed(&self) -> &PerfectByteHashMap<[u8]> {
        PerfectByteHashMap::from_bytes(self.0.as_ref())
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    extern crate std;

    fn random_alphanums(seed: u64, len: usize) -> Vec<u8> {
        use rand::seq::SliceRandom;
        use rand::SeedableRng;

        let mut bytes: Vec<u8> =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789".into();
        let mut rng = rand_pcg::Lcg64Xsh32::seed_from_u64(seed);
        bytes.partial_shuffle(&mut rng, len).0.into()
    }

    #[test]
    fn test_smaller() {
        let mut count_by_p = [0; 256];
        let mut count_by_qmax = [0; 256];
        for len in 1..16 {
            for seed in 0..150 {
                let keys = random_alphanums(seed, len);
                let keys_str = core::str::from_utf8(&keys).unwrap();
                let computed = PerfectByteHashMap::try_new(&keys).expect(keys_str);
                computed
                    .check()
                    .unwrap_or_else(|_| panic!("{}", std::str::from_utf8(&keys).expect(keys_str)));
                let (p, qmax) = computed.p_qmax().unwrap();
                count_by_p[p as usize] += 1;
                count_by_qmax[qmax as usize] += 1;
            }
        }
        std::println!("count_by_p (smaller): {count_by_p:?}");
        std::println!("count_by_qmax (smaller): {count_by_qmax:?}");
        let count_fastq = count_by_qmax[0..=Q_FAST_MAX as usize].iter().sum::<usize>();
        let count_slowq = count_by_qmax[Q_FAST_MAX as usize + 1..]
            .iter()
            .sum::<usize>();
        std::println!("fastq/slowq: {count_fastq}/{count_slowq}");
        // Assert that 99% of cases resolve to the fast hash
        assert!(count_fastq >= count_slowq * 100);
    }

    #[test]
    fn test_larger() {
        let mut count_by_p = [0; 256];
        let mut count_by_qmax = [0; 256];
        for len in 16..60 {
            for seed in 0..75 {
                let keys = random_alphanums(seed, len);
                let keys_str = core::str::from_utf8(&keys).unwrap();
                let computed = PerfectByteHashMap::try_new(&keys).expect(keys_str);
                computed
                    .check()
                    .unwrap_or_else(|_| panic!("{}", std::str::from_utf8(&keys).expect(keys_str)));
                let (p, qmax) = computed.p_qmax().unwrap();
                count_by_p[p as usize] += 1;
                count_by_qmax[qmax as usize] += 1;
            }
        }
        std::println!("count_by_p (larger): {count_by_p:?}");
        std::println!("count_by_qmax (larger): {count_by_qmax:?}");
        let count_fastq = count_by_qmax[0..=Q_FAST_MAX as usize].iter().sum::<usize>();
        let count_slowq = count_by_qmax[Q_FAST_MAX as usize + 1..]
            .iter()
            .sum::<usize>();
        std::println!("fastq/slowq: {count_fastq}/{count_slowq}");
        // Assert that 99% of cases resolve to the fast hash
        assert!(count_fastq >= count_slowq * 100);
    }

    #[test]
    fn test_hard_cases() {
        let keys = [
            0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
            46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67,
            68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89,
            90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108,
            109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125,
            126, 195, 196,
        ];

        let computed = PerfectByteHashMap::try_new(&keys).unwrap();
        let (p, qmax) = computed.p_qmax().unwrap();
        assert_eq!(p, 69);
        assert_eq!(qmax, 67);
    }

    #[test]
    fn test_build_read_small() {
        #[derive(Debug)]
        struct TestCase<'a> {
            keys: &'a str,
            expected: &'a [u8],
            reordered_keys: &'a str,
        }
        let cases = [
            TestCase {
                keys: "ab",
                expected: &[0, 0, 0, b'b', b'a'],
                reordered_keys: "ba",
            },
            TestCase {
                keys: "abc",
                expected: &[0, 0, 0, 0, b'c', b'a', b'b'],
                reordered_keys: "cab",
            },
            TestCase {
                // Note: splitting "a" and "c" into different buckets requires the heavier hash
                // function because the difference between "a" and "c" is the period (2).
                keys: "ac",
                expected: &[1, 0, 1, b'c', b'a'],
                reordered_keys: "ca",
            },
            TestCase {
                keys: "aceg",
                expected: &[1, 0, 0, 1, 1, b'e', b'a', b'c', b'g'],
                reordered_keys: "eacg",
            },
            TestCase {
                keys: "abd",
                expected: &[0, 0, 1, 3, b'a', b'b', b'd'],
                reordered_keys: "abd",
            },
            TestCase {
                keys: "def",
                expected: &[0, 0, 0, 0, b'f', b'd', b'e'],
                reordered_keys: "fde",
            },
            TestCase {
                keys: "fi",
                expected: &[0, 0, 0, b'f', b'i'],
                reordered_keys: "fi",
            },
            TestCase {
                keys: "gh",
                expected: &[0, 0, 0, b'h', b'g'],
                reordered_keys: "hg",
            },
            TestCase {
                keys: "lm",
                expected: &[0, 0, 0, b'l', b'm'],
                reordered_keys: "lm",
            },
            TestCase {
                // Note: "a" and "q" (0x61 and 0x71) are very hard to split; only a handful of
                // hash function crates can get them into separate buckets.
                keys: "aq",
                expected: &[4, 0, 1, b'a', b'q'],
                reordered_keys: "aq",
            },
            TestCase {
                keys: "xy",
                expected: &[0, 0, 0, b'x', b'y'],
                reordered_keys: "xy",
            },
            TestCase {
                keys: "xyz",
                expected: &[0, 0, 0, 0, b'x', b'y', b'z'],
                reordered_keys: "xyz",
            },
            TestCase {
                keys: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz",
                expected: &[
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 10, 12, 16, 4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 16,
                    16, 16, 16, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
                    2, 0, 7, 104, 105, 106, 107, 108, 109, 110, 111, 112, 117, 118, 119, 68, 69,
                    70, 113, 114, 65, 66, 67, 120, 121, 122, 115, 72, 73, 74, 71, 80, 81, 82, 83,
                    84, 85, 86, 87, 88, 89, 90, 75, 76, 77, 78, 79, 103, 97, 98, 99, 116, 100, 102,
                    101,
                ],
                reordered_keys: "hijklmnopuvwDEFqrABCxyzsHIJGPQRSTUVWXYZKLMNOgabctdfe",
            },
            TestCase {
                keys: "abcdefghij",
                expected: &[
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100, 101, 102, 103, 104, 105, 106, 97, 98, 99,
                ],
                reordered_keys: "defghijabc",
            },
            TestCase {
                // This is a small case that resolves to the slow hasher
                keys: "Jbej",
                expected: &[2, 0, 0, 102, 0, b'j', b'e', b'b', b'J'],
                reordered_keys: "jebJ",
            },
            TestCase {
                // This is another small case that resolves to the slow hasher
                keys: "JFNv",
                expected: &[1, 98, 0, 2, 0, b'J', b'F', b'N', b'v'],
                reordered_keys: "JFNv",
            },
        ];
        for cas in cases {
            let computed = PerfectByteHashMap::try_new(cas.keys.as_bytes()).expect(cas.keys);
            assert_eq!(computed.as_bytes(), cas.expected, "{:?}", cas);
            assert_eq!(computed.keys(), cas.reordered_keys.as_bytes(), "{:?}", cas);
            computed.check().expect(cas.keys);
        }
    }
}
