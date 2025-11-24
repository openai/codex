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

//! `SmallSet` which considers order important for equality and hash.

use std::cmp::Ordering;
use std::hash::Hash;

use allocative::Allocative;

use crate::small_set;
use crate::small_set::SmallSet;
use crate::Equivalent;
use crate::Hashed;

/// `SmallSet` wrapper, but equality and hash of self depends on iteration order.
#[derive(Debug, Clone, Allocative)]
pub struct OrderedSet<T>(SmallSet<T>);

/// Error returned by `try_insert`.
#[derive(Debug)]
pub struct OccupiedError<'a, T> {
    /// The value that was not inserted.
    pub value: T,
    /// The value that was already in the set.
    pub occupied: &'a T,
}

impl<T> OrderedSet<T> {
    /// Create a new empty set.
    #[inline]
    pub const fn new() -> OrderedSet<T> {
        OrderedSet(SmallSet::new())
    }

    /// Create a new empty set with the specified capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> OrderedSet<T> {
        OrderedSet(SmallSet::with_capacity(capacity))
    }

    /// Get the number of elements in the set.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if the set is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get an element from the set.
    #[inline]
    pub fn get<Q>(&self, value: &Q) -> Option<&T>
    where
        Q: Hash + Equivalent<T> + ?Sized,
        T: Eq,
    {
        self.0.get(value)
    }

    /// Check if the set contains an element.
    #[inline]
    pub fn contains<Q>(&self, value: &Q) -> bool
    where
        Q: Hash + Equivalent<T> + ?Sized,
        T: Eq,
    {
        self.0.contains(value)
    }

    /// Get an element by index.
    #[inline]
    pub fn get_index(&self, index: usize) -> Option<&T> {
        self.0.get_index(index)
    }

    /// Get the index of an element in the set.
    #[inline]
    pub fn get_index_of<Q>(&self, value: &Q) -> Option<usize>
    where
        Q: Hash + Equivalent<T> + ?Sized,
        T: Eq,
    {
        self.0.get_index_of(value)
    }

    /// Remove an element from the set.
    #[inline]
    pub fn take<Q>(&mut self, value: &Q) -> Option<T>
    where
        Q: Hash + Equivalent<T> + ?Sized,
        T: Eq,
    {
        self.0.take(value)
    }

    /// Iterate over the elements.
    #[inline]
    pub fn iter(&self) -> Iter<T> {
        self.0.iter()
    }

    /// Get the first element.
    #[inline]
    pub fn first(&self) -> Option<&T> {
        self.0.first()
    }

    /// Get the last element.
    #[inline]
    pub fn last(&self) -> Option<&T> {
        self.0.last()
    }

    /// Insert an element into the set.
    #[inline]
    pub fn insert(&mut self, value: T) -> bool
    where
        T: Hash + Eq,
    {
        self.0.insert(value)
    }

    /// Insert an element into the set assuming it is not already present.
    #[inline]
    pub fn insert_unique_unchecked(&mut self, value: T)
    where
        T: Hash,
    {
        self.0.insert_unique_unchecked(value)
    }

    /// Insert an element if element is not present in the set,
    /// otherwise return the element.
    #[inline]
    pub fn try_insert<'a>(&'a mut self, value: T) -> Result<(), OccupiedError<'a, T>>
    where
        T: Hash + Eq,
    {
        let value = Hashed::new(value);

        // SAFETY: Extending the lifetime of the `self` reference here is valid because:
        //  1. The `self` reference already has lifetime `'a`.
        //  2. Within this block, we only use `this`, not `self`.
        //  3. Unless we return from this block, we do not store any references
        //     derived from `this` which might later conflict with `self`.
        // Writing the equivalent safe code does not work,
        // due to a well-known limitation of the borrow-checker:
        // https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=703ece3ad1d40f3c0d6ba1cd8b18194e
        // `-Zpolonius` fixes the issue.
        let this: &'a mut Self = unsafe { &mut *(self as *mut Self) };
        if let Some(occupied) = this.0.get_hashed(value.as_ref()) {
            return Err(OccupiedError {
                value: value.into_key(),
                occupied,
            });
        }

        self.0.insert_hashed_unique_unchecked(value);
        Ok(())
    }

    /// Clear the set.
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear()
    }

    /// Sort the set.
    #[inline]
    pub fn sort(&mut self)
    where
        T: Ord,
    {
        self.0.sort()
    }

    /// Iterate over the union of two sets.
    #[inline]
    pub fn union<'a>(&'a self, other: &'a Self) -> impl Iterator<Item = &'a T>
    where
        T: Eq + Hash,
    {
        self.0.union(&other.0)
    }

    /// Reverse the iteration order of the set.
    pub fn reverse(&mut self) {
        self.0.reverse();
    }
}

impl<T> Default for OrderedSet<T> {
    #[inline]
    fn default() -> OrderedSet<T> {
        OrderedSet::new()
    }
}

impl<T: Eq> PartialEq for OrderedSet<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_ordered(&other.0)
    }
}

impl<T: Eq> Eq for OrderedSet<T> {}

impl<T: Eq + PartialOrd> PartialOrd for OrderedSet<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.iter().partial_cmp(other.0.iter())
    }
}

impl<T: Eq + Ord> Ord for OrderedSet<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.iter().cmp(other.0.iter())
    }
}

impl<T: Hash> Hash for OrderedSet<T> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash_ordered(state)
    }
}

/// Iterator returned by `iter`.
pub type Iter<'a, T> = small_set::Iter<'a, T>;
/// Iterator returned by `into_iter`.
pub type IntoIter<T> = small_set::IntoIter<T>;

impl<T> IntoIterator for OrderedSet<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a OrderedSet<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<T> From<SmallSet<T>> for OrderedSet<T> {
    #[inline]
    fn from(set: SmallSet<T>) -> OrderedSet<T> {
        OrderedSet(set)
    }
}

impl<T> FromIterator<T> for OrderedSet<T>
where
    T: Eq + Hash,
{
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        OrderedSet(SmallSet::from_iter(iter))
    }
}

impl<T> Extend<T> for OrderedSet<T>
where
    T: Eq + Hash,
{
    #[inline]
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.0.extend(iter)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::sync::Arc;

    use dupe::Dupe;

    use crate::ordered_set::OrderedSet;

    #[test]
    fn test_keys_are_not_hashed_when_map_is_hashed() {
        struct Tester {
            /// Number of times `hash` was called.
            hash_count: Cell<u32>,
        }

        impl PartialEq for Tester {
            fn eq(&self, _other: &Self) -> bool {
                true
            }
        }

        impl Eq for Tester {}

        impl Hash for Tester {
            fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {
                self.hash_count.set(self.hash_count.get() + 1);
            }
        }

        let set = OrderedSet::from_iter([Tester {
            hash_count: Cell::new(0),
        }]);
        assert_eq!(1, set.iter().next().unwrap().hash_count.get());

        let mut hasher = DefaultHasher::new();

        set.hash(&mut hasher);
        assert_eq!(1, set.iter().next().unwrap().hash_count.get());

        set.hash(&mut hasher);
        assert_eq!(1, set.iter().next().unwrap().hash_count.get());

        set.hash(&mut hasher);
        assert_eq!(1, set.iter().next().unwrap().hash_count.get());
    }

    #[test]
    fn test_insert_unique() {
        let mut set: OrderedSet<Arc<u32>> = OrderedSet::new();
        let inserted = set.try_insert(Arc::new(1));
        assert!(inserted.is_ok());

        let one = Arc::new(1);
        let inserted = set.try_insert(one.dupe());
        assert!(Arc::ptr_eq(&inserted.unwrap_err().value, &one));
    }
}
