use alloc::collections::VecDeque;
use core::borrow::Borrow;
use core::hash::Hash;

use crate::hash_map::{Entry, HashMap};

/// A HashMap-alike, which never gets larger than a specified
/// capacity, and evicts the oldest insertion to maintain this.
///
/// The requested capacity may be rounded up by the underlying
/// collections.  This implementation uses all the allocated
/// storage.
///
/// This is inefficient: it stores keys twice.
pub(crate) struct LimitedCache<K: Clone + Hash + Eq, V> {
    map: HashMap<K, V>,

    // first item is the oldest key
    oldest: VecDeque<K>,
}

impl<K, V> LimitedCache<K, V>
where
    K: Eq + Hash + Clone + core::fmt::Debug,
    V: Default,
{
    pub(crate) fn get_or_insert_default_and_edit(&mut self, k: K, edit: impl FnOnce(&mut V)) {
        let inserted_new_item = match self.map.entry(k) {
            Entry::Occupied(value) => {
                edit(value.into_mut());
                false
            }
            entry @ Entry::Vacant(_) => {
                self.oldest
                    .push_back(entry.key().clone());
                edit(entry.or_insert_with(V::default));
                true
            }
        };

        // ensure next insertion does not require a realloc
        if inserted_new_item && self.oldest.capacity() == self.oldest.len() {
            if let Some(oldest_key) = self.oldest.pop_front() {
                self.map.remove(&oldest_key);
            }
        }
    }

    pub(crate) fn get_mut<Q: Hash + Eq + ?Sized>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
    {
        self.map.get_mut(k)
    }
}

impl<K, V> LimitedCache<K, V>
where
    K: Eq + Hash + Clone + core::fmt::Debug,
    V: Default,
{
    /// Create a new LimitedCache with the given rough capacity.
    pub(crate) fn new(capacity_order_of_magnitude: usize) -> Self {
        Self {
            map: HashMap::with_capacity(capacity_order_of_magnitude),
            oldest: VecDeque::with_capacity(capacity_order_of_magnitude),
        }
    }

    pub(crate) fn insert(&mut self, k: K, v: V) {
        let inserted_new_item = match self.map.entry(k) {
            Entry::Occupied(mut old) => {
                // Note: does not freshen entry in `oldest`
                old.insert(v);
                false
            }

            entry @ Entry::Vacant(_) => {
                self.oldest
                    .push_back(entry.key().clone());
                entry.or_insert(v);
                true
            }
        };

        // ensure next insertion does not require a realloc
        if inserted_new_item && self.oldest.capacity() == self.oldest.len() {
            if let Some(oldest_key) = self.oldest.pop_front() {
                self.map.remove(&oldest_key);
            }
        }
    }

    pub(crate) fn get<Q: Hash + Eq + ?Sized>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        self.map.get(k)
    }

    pub(crate) fn remove<Q: Hash + Eq + ?Sized>(&mut self, k: &Q) -> Option<V>
    where
        K: Borrow<Q>,
    {
        let value = self.map.remove(k)?;

        // O(N) search, followed by O(N) removal
        if let Some(index) = self
            .oldest
            .iter()
            .position(|item| item.borrow() == k)
        {
            self.oldest.remove(index);
        }

        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use std::prelude::v1::*;

    type Test = super::LimitedCache<String, usize>;

    #[test]
    fn test_updates_existing_item() {
        let mut t = Test::new(3);
        t.insert("abc".into(), 1);
        t.insert("abc".into(), 2);
        assert_eq!(t.get("abc"), Some(&2));
    }

    #[test]
    fn test_evicts_oldest_item() {
        let mut t = Test::new(3);
        t.insert("abc".into(), 1);
        t.insert("def".into(), 2);
        t.insert("ghi".into(), 3);

        assert_eq!(t.get("abc"), None);
        assert_eq!(t.get("def"), Some(&2));
        assert_eq!(t.get("ghi"), Some(&3));
    }

    #[test]
    fn test_evicts_second_oldest_item_if_first_removed() {
        let mut t = Test::new(3);
        t.insert("abc".into(), 1);
        t.insert("def".into(), 2);

        assert_eq!(t.remove("abc"), Some(1));

        t.insert("ghi".into(), 3);
        t.insert("jkl".into(), 4);

        assert_eq!(t.get("abc"), None);
        assert_eq!(t.get("def"), None);
        assert_eq!(t.get("ghi"), Some(&3));
        assert_eq!(t.get("jkl"), Some(&4));
    }

    #[test]
    fn test_evicts_after_second_oldest_item_removed() {
        let mut t = Test::new(3);
        t.insert("abc".into(), 1);
        t.insert("def".into(), 2);

        assert_eq!(t.remove("def"), Some(2));
        assert_eq!(t.get("abc"), Some(&1));

        t.insert("ghi".into(), 3);
        t.insert("jkl".into(), 4);

        assert_eq!(t.get("abc"), None);
        assert_eq!(t.get("def"), None);
        assert_eq!(t.get("ghi"), Some(&3));
        assert_eq!(t.get("jkl"), Some(&4));
    }

    #[test]
    fn test_removes_all_items() {
        let mut t = Test::new(3);
        t.insert("abc".into(), 1);
        t.insert("def".into(), 2);

        assert_eq!(t.remove("def"), Some(2));
        assert_eq!(t.remove("abc"), Some(1));

        t.insert("ghi".into(), 3);
        t.insert("jkl".into(), 4);
        t.insert("mno".into(), 5);

        assert_eq!(t.get("abc"), None);
        assert_eq!(t.get("def"), None);
        assert_eq!(t.get("ghi"), None);
        assert_eq!(t.get("jkl"), Some(&4));
        assert_eq!(t.get("mno"), Some(&5));
    }

    #[test]
    fn test_inserts_many_items() {
        let mut t = Test::new(3);

        for _ in 0..10000 {
            t.insert("abc".into(), 1);
            t.insert("def".into(), 2);
            t.insert("ghi".into(), 3);
        }
    }

    #[test]
    fn test_get_or_insert_default_and_edit_evicts_old_items_to_meet_capacity() {
        let mut t = Test::new(3);

        t.get_or_insert_default_and_edit("abc".into(), |v| *v += 1);
        t.get_or_insert_default_and_edit("def".into(), |v| *v += 2);

        // evicts "abc"
        t.get_or_insert_default_and_edit("ghi".into(), |v| *v += 3);
        assert_eq!(t.get("abc"), None);

        // evicts "def"
        t.get_or_insert_default_and_edit("jkl".into(), |v| *v += 4);
        assert_eq!(t.get("def"), None);

        // evicts "ghi"
        t.get_or_insert_default_and_edit("abc".into(), |v| *v += 5);
        assert_eq!(t.get("ghi"), None);

        // evicts "jkl"
        t.get_or_insert_default_and_edit("def".into(), |v| *v += 6);

        assert_eq!(t.get("abc"), Some(&5));
        assert_eq!(t.get("def"), Some(&6));
        assert_eq!(t.get("ghi"), None);
        assert_eq!(t.get("jkl"), None);
    }

    #[test]
    fn test_get_or_insert_default_and_edit_edits_existing_item() {
        let mut t = Test::new(3);

        t.get_or_insert_default_and_edit("abc".into(), |v| *v += 1);
        t.get_or_insert_default_and_edit("abc".into(), |v| *v += 2);
        t.get_or_insert_default_and_edit("abc".into(), |v| *v += 3);

        assert_eq!(t.get("abc"), Some(&6));
    }
}
