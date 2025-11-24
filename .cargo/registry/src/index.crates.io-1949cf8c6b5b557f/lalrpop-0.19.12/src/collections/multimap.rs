use std::collections::btree_map;
use std::default::Default;
use std::iter::FromIterator;

use super::map::{map, Map};
use super::set::Set;

pub struct Multimap<K, C: Collection> {
    map: Map<K, C>,
}

pub trait Collection: Default {
    type Item;

    /// Push `item` into the collection and return `true` if
    /// collection changed.
    fn push(&mut self, item: Self::Item) -> bool;
}

impl<K: Ord, C: Collection> Multimap<K, C> {
    pub fn new() -> Multimap<K, C> {
        Multimap { map: map() }
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Push `value` to the collection associated with `key`. Returns
    /// true if the collection was changed from the default.
    pub fn push(&mut self, key: K, value: C::Item) -> bool {
        let mut inserted = false;
        let pushed = self
            .map
            .entry(key)
            .or_insert_with(|| {
                inserted = true;
                C::default()
            })
            .push(value);
        inserted || pushed
    }

    pub fn get(&self, key: &K) -> Option<&C> {
        self.map.get(key)
    }

    pub fn iter(&self) -> btree_map::Iter<K, C> {
        self.map.iter()
    }

    pub fn into_iter(self) -> btree_map::IntoIter<K, C> {
        self.map.into_iter()
    }
}

impl<K: Ord, C: Collection> IntoIterator for Multimap<K, C> {
    type Item = (K, C);
    type IntoIter = btree_map::IntoIter<K, C>;
    fn into_iter(self) -> btree_map::IntoIter<K, C> {
        self.into_iter()
    }
}

impl<'iter, K: Ord, C: Collection> IntoIterator for &'iter Multimap<K, C> {
    type Item = (&'iter K, &'iter C);
    type IntoIter = btree_map::Iter<'iter, K, C>;
    fn into_iter(self) -> btree_map::Iter<'iter, K, C> {
        self.iter()
    }
}

impl<K: Ord, C: Collection> FromIterator<(K, C::Item)> for Multimap<K, C> {
    fn from_iter<T>(iterator: T) -> Self
    where
        T: IntoIterator<Item = (K, C::Item)>,
    {
        let mut map = Multimap::new();
        for (key, value) in iterator {
            map.push(key, value);
        }
        map
    }
}

impl Collection for () {
    type Item = ();
    fn push(&mut self, _item: ()) -> bool {
        false
    }
}

impl<T> Collection for Vec<T> {
    type Item = T;

    fn push(&mut self, item: T) -> bool {
        self.push(item);
        true // always changes
    }
}

impl<T: Ord> Collection for Set<T> {
    type Item = T;

    fn push(&mut self, item: T) -> bool {
        self.insert(item)
    }
}

impl<K: Ord, C: Collection> Default for Multimap<K, C> {
    fn default() -> Self {
        Multimap::new()
    }
}

impl<K: Ord, C: Collection<Item = I>, I> Collection for Multimap<K, C> {
    type Item = (K, I);

    fn push(&mut self, item: (K, I)) -> bool {
        let (key, value) = item;
        self.push(key, value)
    }
}

#[test]
fn push() {
    let mut m: Multimap<u32, Set<char>> = Multimap::new();
    assert!(m.push(0, 'a'));
    assert!(m.push(0, 'b'));
    assert!(!m.push(0, 'b'));
    assert!(m.push(1, 'a'));
}

#[test]
fn push_nil() {
    let mut m: Multimap<u32, ()> = Multimap::new();
    assert!(m.push(0, ()));
    assert!(!m.push(0, ()));
    assert!(m.push(1, ()));
    assert!(!m.push(0, ()));
}
