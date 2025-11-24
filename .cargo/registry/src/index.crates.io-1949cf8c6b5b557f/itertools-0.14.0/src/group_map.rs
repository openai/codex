#![cfg(feature = "use_std")]

use std::collections::HashMap;
use std::hash::Hash;
use std::iter::Iterator;

/// Return a `HashMap` of keys mapped to a list of their corresponding values.
///
/// See [`.into_group_map()`](crate::Itertools::into_group_map)
/// for more information.
pub fn into_group_map<I, K, V>(iter: I) -> HashMap<K, Vec<V>>
where
    I: Iterator<Item = (K, V)>,
    K: Hash + Eq,
{
    let mut lookup = HashMap::new();

    iter.for_each(|(key, val)| {
        lookup.entry(key).or_insert_with(Vec::new).push(val);
    });

    lookup
}

pub fn into_group_map_by<I, K, V, F>(iter: I, mut f: F) -> HashMap<K, Vec<V>>
where
    I: Iterator<Item = V>,
    K: Hash + Eq,
    F: FnMut(&V) -> K,
{
    into_group_map(iter.map(|v| (f(&v), v)))
}
