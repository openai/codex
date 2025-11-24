#![allow(dead_code)]

use std::borrow::Borrow;

/// Flat (Vec) backed map
///
/// This preserves insertion order
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FlatMap<K, V> {
    keys: Vec<K>,
    values: Vec<V>,
}

impl<K: PartialEq + Eq, V> FlatMap<K, V> {
    pub(crate) fn new() -> Self {
        Default::default()
    }

    pub(crate) fn insert(&mut self, key: K, mut value: V) -> Option<V> {
        for (index, existing) in self.keys.iter().enumerate() {
            if *existing == key {
                std::mem::swap(&mut self.values[index], &mut value);
                return Some(value);
            }
        }

        self.insert_unchecked(key, value);
        None
    }

    pub(crate) fn insert_unchecked(&mut self, key: K, value: V) {
        self.keys.push(key);
        self.values.push(value);
    }

    pub(crate) fn extend_unchecked(&mut self, iter: impl IntoIterator<Item = (K, V)>) {
        for (key, value) in iter {
            self.insert_unchecked(key, value);
        }
    }

    pub(crate) fn contains_key<Q: ?Sized>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Eq,
    {
        for existing in &self.keys {
            if existing.borrow() == key {
                return true;
            }
        }
        false
    }

    pub(crate) fn remove<Q: ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: std::hash::Hash + Eq,
    {
        self.remove_entry(key).map(|(_, v)| v)
    }

    pub(crate) fn remove_entry<Q: ?Sized>(&mut self, key: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q>,
        Q: std::hash::Hash + Eq,
    {
        let index = some!(self
            .keys
            .iter()
            .enumerate()
            .find_map(|(i, k)| (k.borrow() == key).then_some(i)));
        let key = self.keys.remove(index);
        let value = self.values.remove(index);
        Some((key, value))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub(crate) fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        for (index, existing) in self.keys.iter().enumerate() {
            if *existing == key {
                return Entry::Occupied(OccupiedEntry { v: self, index });
            }
        }
        Entry::Vacant(VacantEntry { v: self, key })
    }

    pub(crate) fn get<Q: ?Sized>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Eq,
    {
        for (index, existing) in self.keys.iter().enumerate() {
            if existing.borrow() == k {
                return Some(&self.values[index]);
            }
        }
        None
    }

    pub(crate) fn get_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Eq,
    {
        for (index, existing) in self.keys.iter().enumerate() {
            if existing.borrow() == k {
                return Some(&mut self.values[index]);
            }
        }
        None
    }

    pub(crate) fn keys(&self) -> std::slice::Iter<'_, K> {
        self.keys.iter()
    }

    pub(crate) fn values(&self) -> std::slice::Iter<'_, V> {
        self.values.iter()
    }

    pub(crate) fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            keys: self.keys.iter(),
            values: self.values.iter(),
        }
    }

    pub(crate) fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        IterMut {
            keys: self.keys.iter_mut(),
            values: self.values.iter_mut(),
        }
    }
}

impl<K: PartialEq + Eq, V> Default for FlatMap<K, V> {
    fn default() -> Self {
        Self {
            keys: Default::default(),
            values: Default::default(),
        }
    }
}

pub(crate) enum Entry<'a, K, V> {
    Vacant(VacantEntry<'a, K, V>),
    Occupied(OccupiedEntry<'a, K, V>),
}

impl<'a, K: 'a, V: 'a> Entry<'a, K, V> {
    pub(crate) fn or_insert(self, default: V) -> &'a mut V {
        match self {
            Entry::Occupied(entry) => &mut entry.v.values[entry.index],
            Entry::Vacant(entry) => {
                entry.v.keys.push(entry.key);
                entry.v.values.push(default);
                entry.v.values.last_mut().unwrap()
            }
        }
    }

    pub(crate) fn or_insert_with<F: FnOnce() -> V>(self, default: F) -> &'a mut V {
        match self {
            Entry::Occupied(entry) => &mut entry.v.values[entry.index],
            Entry::Vacant(entry) => {
                entry.v.keys.push(entry.key);
                entry.v.values.push(default());
                entry.v.values.last_mut().unwrap()
            }
        }
    }
}

pub(crate) struct VacantEntry<'a, K, V> {
    v: &'a mut FlatMap<K, V>,
    key: K,
}

pub(crate) struct OccupiedEntry<'a, K, V> {
    v: &'a mut FlatMap<K, V>,
    index: usize,
}

pub(crate) struct Iter<'a, K, V> {
    keys: std::slice::Iter<'a, K>,
    values: std::slice::Iter<'a, V>,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<(&'a K, &'a V)> {
        match self.keys.next() {
            Some(k) => {
                let v = self.values.next().unwrap();
                Some((k, v))
            }
            None => None,
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.keys.size_hint()
    }
}

impl<'a, K, V> DoubleEndedIterator for Iter<'a, K, V> {
    fn next_back(&mut self) -> Option<(&'a K, &'a V)> {
        match self.keys.next_back() {
            Some(k) => {
                let v = self.values.next_back().unwrap();
                Some((k, v))
            }
            None => None,
        }
    }
}

impl<K, V> ExactSizeIterator for Iter<'_, K, V> {}

pub(crate) struct IterMut<'a, K, V> {
    keys: std::slice::IterMut<'a, K>,
    values: std::slice::IterMut<'a, V>,
}

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    fn next(&mut self) -> Option<(&'a K, &'a mut V)> {
        match self.keys.next() {
            Some(k) => {
                let v = self.values.next().unwrap();
                Some((k, v))
            }
            None => None,
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.keys.size_hint()
    }
}

impl<'a, K, V> DoubleEndedIterator for IterMut<'a, K, V> {
    fn next_back(&mut self) -> Option<(&'a K, &'a mut V)> {
        match self.keys.next_back() {
            Some(k) => {
                let v = self.values.next_back().unwrap();
                Some((k, v))
            }
            None => None,
        }
    }
}

impl<K, V> ExactSizeIterator for IterMut<'_, K, V> {}
