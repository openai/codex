// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::map::{MutableZeroVecLike, ZeroMapKV, ZeroVecLike};
use crate::ZeroVec;
use alloc::vec::Vec;
use core::borrow::Borrow;
use core::hash::Hash;

pub mod algorithms;
use algorithms::*;

#[cfg(feature = "serde")]
mod serde;

/// A perfect zerohashmap optimized for lookups over immutable keys.
///
/// # Examples
/// ```
/// use zerovec::ZeroHashMap;
///
/// let hashmap =
///     ZeroHashMap::<i32, str>::from_iter([(0, "a"), (1, "b"), (2, "c")]);
/// assert_eq!(hashmap.get(&0), Some("a"));
/// assert_eq!(hashmap.get(&2), Some("c"));
/// assert_eq!(hashmap.get(&4), None);
/// ```
#[derive(Debug)]
pub struct ZeroHashMap<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
{
    /// Array of (d0, d1) which splits the keys with same first level hash into distinct
    /// slots.
    /// The ith index of the array splits the keys with first level hash i.
    /// If no key with first level hash is found in the original keys, (0, 0) is used as an empty
    /// placeholder.
    displacements: ZeroVec<'a, (u32, u32)>,
    keys: K::Container,
    values: V::Container,
}

impl<'a, K, V> ZeroHashMap<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
{
    /// The number of elements in the [`ZeroHashMap`].
    pub fn len(&self) -> usize {
        self.values.zvl_len()
    }

    /// Whether the [`ZeroHashMap`] is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<'a, K, V> ZeroHashMap<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized + Hash + Eq,
    V: ZeroMapKV<'a> + ?Sized,
{
    /// Given a `key` return the index for the key or [`None`] if the key is absent.
    fn index<A>(&self, key: &A) -> Option<usize>
    where
        A: Borrow<K> + ?Sized,
    {
        let hash = compute_hash(key.borrow());
        let (g, f0, f1) = split_hash64(hash, self.len());

        #[expect(clippy::unwrap_used)] // g is in-range
        let (d0, d1) = self.displacements.get(g).unwrap();
        let index = compute_index((f0, f1), (d0, d1), self.displacements.len() as u32)?;

        #[expect(clippy::unwrap_used)] // index is in 0..self.keys.len()
        let found = self.keys.zvl_get(index).unwrap();
        if K::Container::zvl_get_as_t(found, |found| found == key.borrow()) {
            Some(index)
        } else {
            None
        }
    }

    /// Get the value corresponding to `key`.
    /// If absent [`None`] is returned.
    ///
    /// # Example
    /// ```
    /// use zerovec::ZeroHashMap;
    ///
    /// let hashmap = ZeroHashMap::<str, str>::from_iter([("a", "A"), ("z", "Z")]);
    ///
    /// assert_eq!(hashmap.get("a"), Some("A"));
    /// assert_eq!(hashmap.get("z"), Some("Z"));
    /// assert_eq!(hashmap.get("0"), None);
    /// ```
    pub fn get<'b, A>(&'b self, key: &A) -> Option<&'b V::GetType>
    where
        A: Borrow<K> + ?Sized + 'b,
    {
        self.index(key).and_then(|i| self.values.zvl_get(i))
    }

    /// Returns whether `key` is contained in this hashmap
    ///
    /// # Example
    /// ```rust
    /// use zerovec::ZeroHashMap;
    ///
    /// let hashmap = ZeroHashMap::<str, str>::from_iter([("a", "A"), ("z", "Z")]);
    ///
    /// assert!(hashmap.contains_key("a"));
    /// assert!(!hashmap.contains_key("p"));
    /// ```
    pub fn contains_key(&self, key: &K) -> bool {
        self.index(key).is_some()
    }
}

impl<'a, K, V> ZeroHashMap<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
{
    // Produce an iterator over (key, value) pairs.
    pub fn iter<'b>(
        &'b self,
    ) -> impl ExactSizeIterator<
        Item = (
            &'b <K as ZeroMapKV<'a>>::GetType,
            &'b <V as ZeroMapKV<'a>>::GetType,
        ),
    > {
        (0..self.len()).map(|index| {
            (
                #[expect(clippy::unwrap_used)] // index is in range
                self.keys.zvl_get(index).unwrap(),
                #[expect(clippy::unwrap_used)] // index is in range
                self.values.zvl_get(index).unwrap(),
            )
        })
    }

    // Produce an iterator over keys.
    pub fn iter_keys<'b>(
        &'b self,
    ) -> impl ExactSizeIterator<Item = &'b <K as ZeroMapKV<'a>>::GetType> {
        #[expect(clippy::unwrap_used)] // index is in range
        (0..self.len()).map(|index| self.keys.zvl_get(index).unwrap())
    }

    // Produce an iterator over values.
    pub fn iter_values<'b>(
        &'b self,
    ) -> impl ExactSizeIterator<Item = &'b <V as ZeroMapKV<'a>>::GetType> {
        #[expect(clippy::unwrap_used)] // index is in range
        (0..self.len()).map(|index| self.values.zvl_get(index).unwrap())
    }
}

impl<'a, K, V, A, B> FromIterator<(A, B)> for ZeroHashMap<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized + Hash + Eq,
    V: ZeroMapKV<'a> + ?Sized,
    B: Borrow<V>,
    A: Borrow<K>,
{
    /// Build a [`ZeroHashMap`] from an iterator returning (K, V) tuples.
    ///
    /// # Example
    /// ```
    /// use zerovec::ZeroHashMap;
    ///
    /// let hashmap = ZeroHashMap::<i32, str>::from_iter([
    ///     (1, "a"),
    ///     (2, "b"),
    ///     (3, "c"),
    ///     (4, "d"),
    /// ]);
    /// assert_eq!(hashmap.get(&1), Some("a"));
    /// assert_eq!(hashmap.get(&2), Some("b"));
    /// assert_eq!(hashmap.get(&3), Some("c"));
    /// assert_eq!(hashmap.get(&4), Some("d"));
    /// ```
    fn from_iter<T: IntoIterator<Item = (A, B)>>(iter: T) -> Self {
        let iter = iter.into_iter();
        let size_hint = match iter.size_hint() {
            (_, Some(upper)) => upper,
            (lower, None) => lower,
        };

        let mut key_hashes = Vec::with_capacity(size_hint);
        let mut keys = K::Container::zvl_with_capacity(size_hint);
        let mut values = V::Container::zvl_with_capacity(size_hint);
        for (k, v) in iter {
            keys.zvl_push(k.borrow());
            key_hashes.push(compute_hash(k.borrow()));
            values.zvl_push(v.borrow());
        }

        let (displacements, mut reverse_mapping) = compute_displacements(key_hashes.into_iter());

        keys.zvl_permute(&mut reverse_mapping.clone());
        values.zvl_permute(&mut reverse_mapping);

        Self {
            displacements: ZeroVec::alloc_from_slice(&displacements),
            values,
            keys,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ule::AsULE;
    use rand::{distr::StandardUniform, Rng, SeedableRng};
    use rand_pcg::Lcg64Xsh32;

    #[test]
    fn test_zhms_u64k_u64v() {
        const N: usize = 65530;
        let seed = u64::from_le_bytes(*b"testseed");
        let rng = Lcg64Xsh32::seed_from_u64(seed);
        let kv: Vec<(u64, u64)> = rng.sample_iter(&StandardUniform).take(N).collect();
        let hashmap: ZeroHashMap<u64, u64> =
            ZeroHashMap::from_iter(kv.iter().map(|e| (&e.0, &e.1)));
        for (k, v) in kv {
            assert_eq!(
                hashmap.get(&k).copied().map(<u64 as AsULE>::from_unaligned),
                Some(v),
            );
        }
    }
}
