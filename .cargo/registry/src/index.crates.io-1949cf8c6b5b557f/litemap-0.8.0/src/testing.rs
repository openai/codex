// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Test utilities, primarily targeted to custom LiteMap stores.

use crate::store::*;
use crate::LiteMap;
use alloc::vec::Vec;
use core::fmt::Debug;

// Test code
#[allow(clippy::expect_used)]
fn check_equivalence<'a, K, V, S0, S1>(mut a: S0, mut b: S1)
where
    K: Ord + Debug + PartialEq + 'a,
    V: Debug + PartialEq + 'a,
    S0: StoreMut<K, V> + StoreIterable<'a, K, V>,
    S1: StoreMut<K, V> + StoreIterable<'a, K, V>,
{
    let len = a.lm_len();
    assert_eq!(len, b.lm_len());
    if len == 0 {
        assert!(a.lm_is_empty());
        assert!(b.lm_is_empty());
    }
    for i in 0..len {
        let a_kv = a.lm_get(i);
        let b_kv = b.lm_get(i);
        assert!(a_kv.is_some());
        assert_eq!(a_kv, b_kv);
        let a_kv_mut = a.lm_get_mut(i);
        let b_kv_mut = b.lm_get_mut(i);
        assert!(a_kv_mut.is_some());
        assert_eq!(a_kv_mut, b_kv_mut);
    }
    for j in 0..len {
        let needle = a.lm_get(j).expect("j is in range").0;
        let a_binary = a.lm_binary_search_by(|k| k.cmp(needle));
        let b_binary = a.lm_binary_search_by(|k| k.cmp(needle));
        assert_eq!(Ok(j), a_binary);
        assert_eq!(Ok(j), b_binary);
    }
    assert!(a.lm_get(len).is_none());
    assert!(b.lm_get(len).is_none());
    assert_eq!(a.lm_last(), b.lm_last());
}

// Test code
#[allow(clippy::expect_used)]
fn check_into_iter_equivalence<K, V, S0, S1>(a: S0, b: S1)
where
    K: Ord + Debug + PartialEq,
    V: Debug + PartialEq,
    S0: StoreIntoIterator<K, V>,
    S1: StoreIntoIterator<K, V>,
{
    let a_vec = a.lm_into_iter().collect::<Vec<_>>();
    let b_vec = b.lm_into_iter().collect::<Vec<_>>();
    assert_eq!(a_vec, b_vec);
}

const SORTED_DATA: &[(u32, u64)] = &[
    (106, 4816),
    (147, 9864),
    (188, 8588),
    (252, 6031),
    (434, 2518),
    (574, 8500),
    (607, 3756),
    (619, 4965),
    (663, 2669),
    (724, 9211),
];

const RANDOM_DATA: &[(u32, u64)] = &[
    (546, 7490),
    (273, 4999),
    (167, 8078),
    (176, 2101),
    (373, 1304),
    (339, 9613),
    (561, 3620),
    (301, 1214),
    (483, 4453),
    (704, 5359),
];

// Test code
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
fn populate_litemap<S>(map: &mut LiteMap<u32, u64, S>)
where
    S: StoreMut<u32, u64> + Debug,
{
    assert_eq!(0, map.len());
    assert!(map.is_empty());
    for (k, v) in SORTED_DATA.iter() {
        #[allow(clippy::single_match)] // for clarity
        match map.try_append(*k, *v) {
            Some(_) => panic!("appending sorted data: {k:?} to {map:?}"),
            None => (), // OK
        };
    }
    assert_eq!(10, map.len());
    for (k, v) in RANDOM_DATA.iter() {
        #[allow(clippy::single_match)] // for clarity
        match map.try_append(*k, *v) {
            Some(_) => (), // OK
            None => panic!("cannot append random data: {k:?} to{map:?}"),
        };
    }
    assert_eq!(10, map.len());
    for (k, v) in RANDOM_DATA.iter() {
        map.insert(*k, *v);
    }
    assert_eq!(20, map.len());
}

/// Tests that a litemap that uses the given store as backend has behavior consistent with the
/// reference impl.
///
/// Call this function in a test with the store impl to test as a valid backend for LiteMap.
// Test code
#[allow(clippy::expect_used)]
pub fn check_store<'a, S>()
where
    S: StoreConstEmpty<u32, u64>
        + StoreMut<u32, u64>
        + StoreIterable<'a, u32, u64>
        + StoreFromIterator<u32, u64>
        + Clone
        + Debug
        + PartialEq
        + 'a,
{
    let mut litemap_test: LiteMap<u32, u64, S> = LiteMap::new();
    assert!(litemap_test.is_empty());
    let mut litemap_std = LiteMap::<u32, u64>::new();
    populate_litemap(&mut litemap_test);
    populate_litemap(&mut litemap_std);
    check_equivalence(litemap_test.clone().values, litemap_std.clone().values);

    litemap_test
        .remove(&175)
        .ok_or(())
        .expect_err("does not exist");
    litemap_test.remove(&147).ok_or(()).expect("exists");
    litemap_std
        .remove(&175)
        .ok_or(())
        .expect_err("does not exist");
    litemap_std.remove(&147).ok_or(()).expect("exists");

    assert_eq!(19, litemap_test.len());
    assert_eq!(19, litemap_std.len());
    check_equivalence(litemap_test.clone().values, litemap_std.clone().values);

    litemap_test.clear();
    litemap_std.clear();
    check_equivalence(litemap_test.values, litemap_std.values);
}

/// Similar to [`check_store`] function, but also checks the validitiy of [`StoreIterableMut`]
/// and [`StoreBulkMut`] traits.
// Test code
#[allow(clippy::expect_used)]
pub fn check_store_full<'a, S>()
where
    S: StoreConstEmpty<u32, u64>
        + StoreIterableMut<'a, u32, u64>
        + StoreIntoIterator<u32, u64>
        + StoreFromIterator<u32, u64>
        + StoreBulkMut<u32, u64>
        + Clone
        + Debug
        + PartialEq
        + 'a,
{
    let mut litemap_test: LiteMap<u32, u64, S> = LiteMap::new();
    assert!(litemap_test.is_empty());
    let mut litemap_std = LiteMap::<u32, u64>::new();
    populate_litemap(&mut litemap_test);
    populate_litemap(&mut litemap_std);
    check_equivalence(litemap_test.clone().values, litemap_std.clone().values);
    check_into_iter_equivalence(litemap_test.clone().values, litemap_std.clone().values);

    let extras_test = litemap_test.clone();
    let extras_test = litemap_test
        .extend_from_litemap(extras_test)
        .expect("duplicates");
    assert_eq!(extras_test, litemap_test);
    let extras_std = litemap_std.clone();
    check_equivalence(litemap_test.clone().values, litemap_std.clone().values);
    check_into_iter_equivalence(litemap_test.clone().values, litemap_std.clone().values);

    assert_eq!(20, litemap_test.len());
    litemap_test.retain(|_, v| v % 2 == 0);
    litemap_std.retain(|_, v| v % 2 == 0);
    assert_eq!(11, litemap_test.len());
    assert_eq!(11, litemap_std.len());
    check_equivalence(litemap_test.clone().values, litemap_std.clone().values);
    check_into_iter_equivalence(litemap_test.clone().values, litemap_std.clone().values);

    let extras_test = litemap_test
        .extend_from_litemap(extras_test)
        .expect("duplicates");
    let extras_std = litemap_std
        .extend_from_litemap(extras_std)
        .expect("duplicates");
    assert_eq!(11, extras_test.len());
    assert_eq!(11, extras_std.len());
    assert_eq!(20, litemap_test.len());
    assert_eq!(20, litemap_std.len());
    check_equivalence(litemap_test.clone().values, litemap_std.clone().values);

    assert_eq!(20, litemap_test.len());
    litemap_test.retain(|_, v| v % 2 == 0);
    litemap_std.retain(|_, v| v % 2 == 0);
    assert_eq!(11, litemap_test.len());
    assert_eq!(11, litemap_std.len());
    let mut extras = LiteMap::<u32, u64>::new();
    populate_litemap(&mut extras);
    litemap_test.extend(extras.clone());
    litemap_std.extend(extras);
    assert_eq!(20, litemap_test.len());
    assert_eq!(20, litemap_std.len());

    check_into_iter_equivalence(litemap_test.clone().values, litemap_std.clone().values);
    litemap_test
        .remove(&175)
        .ok_or(())
        .expect_err("does not exist");
    litemap_test.remove(&176).ok_or(()).expect("exists");
    litemap_std
        .remove(&175)
        .ok_or(())
        .expect_err("does not exist");
    litemap_std.remove(&176).ok_or(()).expect("exists");
    assert_eq!(19, litemap_test.len());
    assert_eq!(19, litemap_std.len());
    check_equivalence(litemap_test.clone().values, litemap_std.clone().values);
    check_into_iter_equivalence(litemap_test.clone().values, litemap_std.clone().values);

    litemap_test.clear();
    litemap_std.clear();
    check_equivalence(litemap_test.clone().values, litemap_std.clone().values);
    check_into_iter_equivalence(litemap_test.values, litemap_std.values);

    test_extend::<S>();
}

fn test_extend<'a, S>()
where
    S: StoreConstEmpty<u32, u64>
        + StoreIterableMut<'a, u32, u64>
        + StoreIntoIterator<u32, u64>
        + StoreFromIterator<u32, u64>
        + StoreBulkMut<u32, u64>
        + Clone
        + Debug
        + PartialEq
        + 'a,
{
    // Extend an empty BTreeMap with initial entries.
    let mut map: LiteMap<u32, u64, S> = LiteMap::new();
    let initial_entries = [(1, 1), (2, 2), (3, 3)];
    map.extend(initial_entries);
    assert_eq!(map.len(), 3);
    assert_eq!(map.get(&1), Some(&1));
    assert_eq!(map.get(&2), Some(&2));
    assert_eq!(map.get(&3), Some(&3));

    // Extend with entries that contain keys already present.
    // For repeated keys, the last value should remain.
    let overlapping_entries = [(2, 22), (4, 44), (1, 11)];
    map.extend(overlapping_entries);
    assert_eq!(map.len(), 4);
    assert_eq!(map.get(&1), Some(&11));
    assert_eq!(map.get(&2), Some(&22));
    assert_eq!(map.get(&3), Some(&3));
    assert_eq!(map.get(&4), Some(&44));

    // Extend with an iterator that includes duplicate key entries.
    // The very last occurrence for a key should be the final value.
    let duplicate_entries = [(3, 333), (3, 3333), (5, 5)];
    map.extend(duplicate_entries);
    assert_eq!(map.len(), 5);
    assert_eq!(map.get(&3), Some(&3333));
    assert_eq!(map.get(&5), Some(&5));

    // Extend with an empty iterator: the map should remain unchanged.
    let empty_entries: Vec<(u32, u64)> = Vec::new();
    let map_clone = map.clone();
    map.extend(empty_entries);
    check_equivalence(map.values.clone(), map_clone.values.clone());

    // Extend with the same values: the map should remain unchanged.
    map.extend(map_clone.clone());
    check_equivalence(map.values.clone(), map_clone.values);
}
