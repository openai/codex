/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! Ordered set.

mod iter;

use std::fmt;
use std::fmt::Debug;
use std::hash::Hash;
use std::hash::Hasher;
use std::marker::PhantomData;

use allocative::Allocative;
use equivalent::Equivalent;
use serde::Deserialize;
use serde::Serialize;

use crate::hashed::Hashed;
use crate::small_map::SmallMap;
pub use crate::small_set::iter::IntoIter;
pub use crate::small_set::iter::IntoIterHashed;
pub use crate::small_set::iter::Iter;
pub use crate::small_set::iter::IterHashed;
pub use crate::small_set::iter::IterMutUnchecked;

/// An memory-efficient set with deterministic order, based on [`SmallMap`].
#[derive(Clone, Allocative)]
pub struct SmallSet<T>(SmallMap<T, ()>);

impl<T> Default for SmallSet<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Debug> Debug for SmallSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

impl<T> Eq for SmallSet<T> where T: Eq {}

impl<T> PartialEq for SmallSet<T>
where
    T: Eq,
{
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl<T> FromIterator<T> for SmallSet<T>
where
    T: Hash + Eq,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let mut smallset = Self::with_capacity(iter.size_hint().0);
        for t in iter {
            smallset.insert(t);
        }
        smallset
    }
}

impl<T> SmallSet<T> {
    /// Creates an empty `SmallSet`.
    #[inline]
    pub const fn new() -> Self {
        SmallSet(SmallMap::new())
    }

    /// Empty small set with preallocated capacity.
    #[inline]
    pub fn with_capacity(n: usize) -> Self {
        Self(SmallMap::with_capacity(n))
    }

    /// Current capacity of the set.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// Iterate the element references.
    #[inline]
    pub fn iter(&self) -> Iter<T> {
        self.into_iter()
    }

    /// Iterate the hashed element references.
    #[inline]
    pub fn iter_hashed(&self) -> IterHashed<T> {
        IterHashed {
            iter: self.0.iter_hashed(),
        }
    }

    /// Iterate the mutable element references.
    ///
    /// This operation is memory safe, but otherwise no guarantees
    /// if keys are mutated inconsistently (hash or equality changes).
    #[inline]
    pub fn iter_mut_unchecked(&mut self) -> IterMutUnchecked<T> {
        IterMutUnchecked {
            iter: self.0.iter_mut_unchecked(),
        }
    }

    /// Into hashed entries.
    #[inline]
    pub fn into_iter_hashed(self) -> IntoIterHashed<T> {
        IntoIterHashed {
            iter: self.0.into_iter_hashed(),
        }
    }

    /// Insert the element into the set.
    ///
    /// Return `true` iff the element was inserted.
    #[inline]
    pub fn insert(&mut self, key: T) -> bool
    where
        T: Hash + Eq,
    {
        self.0.insert(key, ()).is_none()
    }

    /// Insert the element into the set without checking for a duplicate entry.
    #[inline]
    pub fn insert_unique_unchecked(&mut self, key: T)
    where
        T: Hash,
    {
        self.0.insert_unique_unchecked(key, ());
    }

    /// Insert the element into the set.
    ///
    /// Return `true` iff the element was inserted.
    #[inline]
    pub fn insert_hashed(&mut self, key: Hashed<T>) -> bool
    where
        T: Eq,
    {
        self.0.insert_hashed(key, ()).is_none()
    }

    /// Insert an entry into the set without checking for a duplicate key.
    #[inline]
    pub fn insert_hashed_unique_unchecked(&mut self, key: Hashed<T>) {
        self.0.insert_hashed_unique_unchecked(key, ());
    }

    /// Return a reference to the value stored in the set, if it is present,
    /// else `None`.
    ///
    /// Computes in **O(1)** time (average).
    #[inline]
    pub fn get<Q>(&self, value: &Q) -> Option<&T>
    where
        Q: Hash + Equivalent<T> + ?Sized,
        T: Eq,
    {
        self.0.get_full(value).map(|(_, t, _)| t)
    }

    /// Query the set by a prehashed value.
    #[inline]
    pub fn get_hashed<Q>(&self, value: Hashed<&Q>) -> Option<&T>
    where
        Q: Equivalent<T> + ?Sized,
        T: Eq,
    {
        self.0.get_full_hashed(value).map(|(_, t, _)| t)
    }

    /// Find an entry by an index.
    #[inline]
    pub fn get_index(&self, index: usize) -> Option<&T> {
        self.0.get_index(index).map(|(k, _)| k)
    }

    /// Return item index, if it exists in the set
    #[inline]
    pub fn get_index_of<Q>(&self, value: &Q) -> Option<usize>
    where
        Q: Hash + Equivalent<T> + ?Sized,
        T: Eq,
    {
        self.0.get_index_of(value)
    }

    /// Find the index of the given hashed value.
    #[inline]
    pub fn get_index_of_hashed<Q>(&self, value: Hashed<&Q>) -> Option<usize>
    where
        Q: Equivalent<T> + ?Sized,
        T: Eq,
    {
        self.0.get_index_of_hashed(value)
    }

    /// Find the index of the given hashed value.
    ///
    /// This operations is similar to [`get_index_of_hashed`](Self::get_index_of_hashed),
    /// but it takes the key by value, instead of by reference
    /// which sometimes generates better code.
    #[inline]
    pub fn get_index_of_hashed_by_value<Q>(&self, value: Hashed<Q>) -> Option<usize>
    where
        Q: Equivalent<T>,
        T: Eq,
    {
        self.0.get_index_of_hashed_by_value(value)
    }

    /// Remove the element from the set if it is present.
    ///
    /// Time complexity of this operation is *O(N)* where *N* is the number of entries in the set.
    #[inline]
    pub fn shift_remove<Q>(&mut self, key: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<T>,
        T: Eq,
    {
        self.0.shift_remove(key).is_some()
    }

    /// Remove the element by index. This is *O(N)* operation.
    #[inline]
    pub fn shift_remove_index_hashed(&mut self, i: usize) -> Option<Hashed<T>> {
        Some(self.0.shift_remove_index_hashed(i)?.0)
    }

    /// Remove the element by index. This is *O(N)* operation.
    #[inline]
    pub fn shift_remove_index(&mut self, i: usize) -> Option<T> {
        Some(self.shift_remove_index_hashed(i)?.into_key())
    }

    /// Remove the entry for the key.
    ///
    /// Time complexity of this operation is *O(N)* where *N* is the number of entries in the set.
    pub fn shift_remove_hashed<Q>(&mut self, key: Hashed<&Q>) -> bool
    where
        Q: ?Sized + Equivalent<T>,
    {
        self.0.shift_remove_hashed(key).is_some()
    }

    /// Insert entry if it doesn't exist.
    ///
    /// Return the resulting entry in the map.
    #[inline]
    pub fn get_or_insert(&mut self, value: T) -> &T
    where
        T: Hash + Eq,
    {
        let value = Hashed::new(value);
        match self
            .0
            .get_index_of_hashed_raw(value.hash(), |v| value.key().equivalent(v))
        {
            Some(index) => self.0.get_index(index).unwrap().0,
            None => self.0.insert_hashed_unique_unchecked(value, ()).0,
        }
    }

    /// Insert entry if it doesn't exist.
    ///
    /// Return the resulting entry in the map.
    #[inline]
    pub fn get_or_insert_owned<Q>(&mut self, value: &Q) -> &T
    where
        Q: Hash + Equivalent<T> + ToOwned<Owned = T> + ?Sized,
        T: Eq,
    {
        let value = Hashed::new(value);
        match self.0.get_index_of_hashed(value) {
            Some(index) => self.0.get_index(index).unwrap().0,
            None => self.0.insert_hashed_unique_unchecked(value.owned(), ()).0,
        }
    }

    /// Remove the element from the set if it is present,
    ///
    /// and return the removed element.
    #[inline]
    pub fn take<Q>(&mut self, key: &Q) -> Option<T>
    where
        Q: ?Sized + Hash + Equivalent<T>,
        T: Eq,
    {
        self.0.shift_remove_entry(key).map(|(k, _)| k)
    }

    /// Remove the last element from the set.
    #[inline]
    pub fn pop(&mut self) -> Option<T>
    where
        T: Eq,
    {
        self.0.pop().map(|(k, ())| k)
    }

    /// Is the set empty?
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Number of elements in the set.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if the set contains an element.
    #[inline]
    pub fn contains<Q>(&self, key: &Q) -> bool
    where
        Q: Hash + Equivalent<T> + ?Sized,
        T: Eq,
    {
        self.0.contains_key(key)
    }

    /// Check if the set contains an element.
    #[inline]
    pub fn contains_hashed<Q>(&self, key: Hashed<&Q>) -> bool
    where
        Q: Equivalent<T> + ?Sized,
        T: Eq,
    {
        self.0.contains_key_hashed(key)
    }

    /// Remove all elements from the set.
    ///
    /// Retain the capacity.
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear()
    }

    /// Returns a reference to the first item.
    #[inline]
    pub fn first(&self) -> Option<&T> {
        self.0.first().map(|(k, ())| k)
    }

    /// Returns a reference to the last item.
    #[inline]
    pub fn last(&self) -> Option<&T> {
        self.0.last().map(|(k, ())| k)
    }

    /// Iterator over elements of this set which are not in the other set.
    pub fn difference<'a>(&'a self, other: &'a Self) -> Difference<'a, T>
    where
        T: Eq + Hash,
    {
        Difference {
            iter: self.iter(),
            other,
        }
    }

    /// Iterator over union of two sets.
    ///
    /// Iteration order is: elements of this set followed by elements in the other set
    /// not present in this set.
    pub fn union<'a>(&'a self, other: &'a Self) -> Union<'a, T>
    where
        T: Eq + Hash,
    {
        Union {
            iter: self.iter().chain(other.difference(self)),
        }
    }

    /// Sort entries.
    pub fn sort(&mut self)
    where
        T: Ord,
    {
        self.0.sort_keys();
    }

    /// Equal if entries are equal in iteration order.
    pub fn eq_ordered(&self, other: &Self) -> bool
    where
        T: PartialEq,
    {
        self.0.eq_ordered(&other.0)
    }

    /// Hash entries in iteration order.
    ///
    /// Note, entries are not hashed, but previously computed hashes are hashed instead.
    pub fn hash_ordered<H: Hasher>(&self, state: &mut H)
    where
        T: Hash,
    {
        self.0.hash_ordered(state)
    }

    /// Reverse the iteration order of the set.
    pub fn reverse(&mut self) {
        self.0.reverse();
    }

    /// Retains only the elements specified by the predicate.
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&T) -> bool,
    {
        self.0.retain(|k, _| f(k))
    }
}

impl<'a, T> IntoIterator for &'a SmallSet<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        Iter {
            iter: self.0.iter(),
        }
    }
}

impl<T> IntoIterator for SmallSet<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            iter: self.0.into_iter(),
        }
    }
}

impl<T> Extend<T> for SmallSet<T>
where
    T: Eq + Hash,
{
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.0.extend(iter.into_iter().map(|v| (v, ())));
    }
}

/// Iterator over the difference of two sets.
pub struct Difference<'a, T: 'a> {
    iter: Iter<'a, T>,
    other: &'a SmallSet<T>,
}

impl<'a, T: 'a> Iterator for Difference<'a, T>
where
    T: Hash + Eq,
{
    type Item = &'a T;

    #[allow(clippy::while_let_on_iterator)]
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(item) = self.iter.next() {
            if !self.other.contains(item) {
                return Some(item);
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            self.iter.len().saturating_sub(self.other.len()),
            Some(self.iter.len()),
        )
    }
}

/// Iterator over a union of two sets.
pub struct Union<'a, T: 'a> {
    iter: std::iter::Chain<Iter<'a, T>, Difference<'a, T>>,
}

impl<'a, T: 'a> Iterator for Union<'a, T>
where
    T: Hash + Eq,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

/// Create a [`SmallSet`](SmallSet) from a list of values.
///
/// ## Example
///
/// ```
/// use starlark_map::smallset;
///
/// let set = smallset! {"a", "b"};
/// assert_eq!(set.contains("a"), true);
/// assert_eq!(set.len(), 2);
/// assert_eq!(set.contains("c"), false);
/// ```
#[macro_export]
macro_rules! smallset {
    (@single $($x:tt)*) => (());
    (@count $($rest:expr),*) => (<[()]>::len(&[$(smallset!(@single $rest)),*]));

    ($($key:expr,)+) => { smallset!($($key),+) };
    ($($key:expr),*) => {
        {
            let cap = smallset!(@count $($key),*);
            let mut set = $crate::small_set::SmallSet::with_capacity(cap);
            $(
                set.insert($key);
            )*
            set
        }
    };
}

impl<T: Serialize> Serialize for SmallSet<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_seq(self.iter())
    }
}

impl<'de, T> Deserialize<'de> for SmallSet<T>
where
    T: Deserialize<'de> + Hash + Eq,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SeqVisitor<T> {
            marker: PhantomData<SmallSet<T>>,
        }

        impl<'de, T> serde::de::Visitor<'de> for SeqVisitor<T>
        where
            T: serde::de::Deserialize<'de> + Hash + Eq,
        {
            type Value = SmallSet<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a sequence")
            }

            #[inline]
            fn visit_seq<A>(self, mut set: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut values = SmallSet::with_capacity(set.size_hint().unwrap_or(0));
                while let Some(value) = set.next_element()? {
                    values.insert(value);
                }
                Ok(values)
            }
        }

        let visitor = SeqVisitor {
            marker: PhantomData,
        };
        deserializer.deserialize_seq(visitor)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::rc::Rc;

    use dupe::Dupe;
    use dupe::IterDupedExt;

    use super::*;

    #[test]
    fn empty_set() {
        let m = SmallSet::<i8>::new();
        assert_eq!(m.is_empty(), true);
        assert_eq!(m.len(), 0);
        assert_eq!(m.iter().next(), None);
    }

    #[test]
    fn few_entries() {
        let entries1 = [(0), (1)];
        let m1 = entries1.iter().duped().collect::<SmallSet<_>>();

        let entries2 = [(1), (0)];
        let m2 = entries2.iter().duped().collect::<SmallSet<_>>();
        assert_eq!(m1.is_empty(), false);
        assert_eq!(m1.len(), 2);
        assert_eq!(m2.is_empty(), false);
        assert_eq!(m2.len(), 2);

        assert_eq!(m1.iter().eq(entries1.iter()), true);
        assert_eq!(m2.iter().eq(entries2.iter()), true);
        assert_eq!(m1.iter().eq(m2.iter()), false);
        assert_eq!(m1.eq(&m1), true);
        assert_eq!(m2.eq(&m2), true);
        assert_eq!(m1, m2);

        assert_ne!(m1, smallset![1])
    }

    #[test]
    fn many_entries() {
        let letters = 'a'..='z';

        let entries1 = letters;
        let m1 = entries1.clone().collect::<SmallSet<_>>();

        assert_eq!(m1.get(&'b'), Some(&'b'));
        assert_eq!(m1.get_index_of(&'b'), Some(1));

        assert_eq!(m1.get(&'!'), None);
        assert_eq!(m1.get_index_of(&'!'), None);

        let letters = ('a'..='z').rev();
        let entries2 = letters;
        let m2 = entries2.clone().collect::<SmallSet<_>>();
        assert_eq!(m1.is_empty(), false);
        assert_eq!(m1.len(), 26);
        assert_eq!(m2.is_empty(), false);
        assert_eq!(m2.len(), 26);

        assert_eq!(m1.clone().into_iter().eq(entries1), true);
        assert_eq!(m2.clone().into_iter().eq(entries2), true);
        assert_eq!(m1.iter().eq(m2.iter()), false);
        assert_eq!(m1.eq(&m1), true);
        assert_eq!(m2.eq(&m2), true);
        assert_eq!(m1, m2);

        let not_m1 = {
            let mut s = m1.clone();
            s.shift_remove(&'a');
            s
        };
        assert_ne!(m1, not_m1);
    }

    #[test]
    fn small_set_macros() {
        let s = smallset![1, 4, 2];
        let mut i = s.into_iter();
        assert_eq!(i.next(), Some(1));
        assert_eq!(i.next(), Some(4));
        assert_eq!(i.next(), Some(2));
        assert_eq!(i.next(), None);
    }

    #[test]
    fn small_set_inserts() {
        let mut s = SmallSet::new();
        assert_eq!(s.insert(2), true);
        assert_eq!(s.insert(5), true);

        assert_eq!(s.insert(5), false);
    }

    #[test]
    fn get_or_insert() {
        let mut set = SmallSet::new();
        let x = set.get_or_insert(Rc::new(1)).dupe();
        let x1 = set.get_or_insert(Rc::new(1));
        assert!(Rc::ptr_eq(&x, x1));
    }

    #[test]
    fn get_or_insert_owned() {
        let mut set = SmallSet::new();
        let x = set.get_or_insert_owned(&Rc::new(1)).dupe();
        let x1 = set.get_or_insert_owned(&Rc::new(1));
        assert!(Rc::ptr_eq(&x, x1));
    }

    #[test]
    fn test_first() {
        let mut s = SmallSet::new();
        s.insert(1);
        assert_eq!(s.first(), Some(&1));
        s.insert(2);
        assert_eq!(s.first(), Some(&1));
        s.shift_remove(&1);
        assert_eq!(s.first(), Some(&2));
    }

    #[test]
    fn test_last() {
        let mut s = SmallSet::new();
        s.insert(1);
        assert_eq!(s.last(), Some(&1));
        s.insert(2);
        assert_eq!(s.last(), Some(&2));
    }

    #[test]
    fn test_shift_remove() {
        let mut h: HashSet<u32> = HashSet::from_iter([17]);
        let mut s: SmallSet<u32> = SmallSet::from_iter([17]);
        assert!(h.remove(&17));
        assert!(s.shift_remove(&17));
        assert!(!h.remove(&17));
        assert!(!s.shift_remove(&17));
    }

    #[test]
    fn test_difference() {
        let a = SmallSet::from_iter([1, 2, 3]);
        let b = SmallSet::from_iter([2, 4, 1]);
        let d = Vec::from_iter(a.difference(&b).copied());
        assert_eq!(vec![3], d);
    }

    #[test]
    fn test_union() {
        let a = SmallSet::from_iter([1, 2, 3]);
        let b = SmallSet::from_iter([2, 4, 1]);
        let d = Vec::from_iter(a.union(&b).copied());
        assert_eq!(vec![1, 2, 3, 4], d);
    }

    #[test]
    fn test_sort() {
        let mut a = SmallSet::from_iter([1, 3, 2]);
        a.sort();
        assert_eq!(vec![1, 2, 3], Vec::from_iter(a));
    }

    #[test]
    fn test_difference_size_hint() {
        let a = SmallSet::from_iter([1, 2, 3]);
        let b = SmallSet::from_iter([2]);
        let mut iter = a.difference(&b);
        assert_eq!((2, Some(3)), iter.size_hint());
        assert_eq!(Some(&1), iter.next());
        assert_eq!((1, Some(2)), iter.size_hint());
        assert_eq!(Some(&3), iter.next());
        assert_eq!((0, Some(0)), iter.size_hint());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn test_json() {
        let mp = smallset! {"a".to_owned() , "b".to_owned() };
        let expected = serde_json::json!(["a", "b"]);
        assert_eq!(serde_json::to_value(&mp).unwrap(), expected);
        assert_eq!(
            serde_json::from_value::<SmallSet<String>>(expected).unwrap(),
            mp
        );
    }
}
