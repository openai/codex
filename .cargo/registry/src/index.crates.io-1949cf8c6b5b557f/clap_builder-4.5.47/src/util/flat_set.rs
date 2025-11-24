#![allow(dead_code)]

use std::borrow::Borrow;

/// Flat (Vec) backed set
///
/// This preserves insertion order
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FlatSet<T> {
    inner: Vec<T>,
}

impl<T: PartialEq + Eq> FlatSet<T> {
    pub(crate) fn new() -> Self {
        Default::default()
    }

    pub(crate) fn insert(&mut self, value: T) -> bool {
        for existing in &self.inner {
            if *existing == value {
                return false;
            }
        }
        self.inner.push(value);
        true
    }

    pub(crate) fn contains<Q: ?Sized>(&self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Eq,
    {
        for existing in &self.inner {
            if existing.borrow() == value {
                return true;
            }
        }
        false
    }

    pub(crate) fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&T) -> bool,
    {
        self.inner.retain(f);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub(crate) fn iter(&self) -> std::slice::Iter<'_, T> {
        self.inner.iter()
    }

    pub(crate) fn sort_by_key<K, F>(&mut self, f: F)
    where
        F: FnMut(&T) -> K,
        K: Ord,
    {
        self.inner.sort_by_key(f);
    }
}

impl<T: PartialEq + Eq> Default for FlatSet<T> {
    fn default() -> Self {
        Self {
            inner: Default::default(),
        }
    }
}

impl<T: PartialEq + Eq> IntoIterator for FlatSet<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

impl<'s, T: PartialEq + Eq> IntoIterator for &'s FlatSet<T> {
    type Item = &'s T;
    type IntoIter = std::slice::Iter<'s, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}

impl<T: PartialEq + Eq> Extend<T> for FlatSet<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for value in iter {
            self.insert(value);
        }
    }
}

impl<T: PartialEq + Eq> FromIterator<T> for FlatSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut set = Self::new();
        for value in iter {
            set.insert(value);
        }
        set
    }
}
