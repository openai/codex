#![feature(test)]

extern crate test;
#[macro_use]
extern crate lazy_static;

use fnv::FnvHasher;
use std::hash::BuildHasherDefault;
use std::hash::Hash;
type FnvBuilder = BuildHasherDefault<FnvHasher>;

use test::black_box;
use test::Bencher;

use indexmap::IndexMap;

use std::collections::HashMap;

use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

/// Use a consistently seeded Rng for benchmark stability
fn small_rng() -> SmallRng {
    let seed = u64::from_le_bytes(*b"indexmap");
    SmallRng::seed_from_u64(seed)
}

#[bench]
fn new_hashmap(b: &mut Bencher) {
    b.iter(|| HashMap::<String, String>::new());
}

#[bench]
fn new_indexmap(b: &mut Bencher) {
    b.iter(|| IndexMap::<String, String>::new());
}

#[bench]
fn with_capacity_10e5_hashmap(b: &mut Bencher) {
    b.iter(|| HashMap::<String, String>::with_capacity(10_000));
}

#[bench]
fn with_capacity_10e5_indexmap(b: &mut Bencher) {
    b.iter(|| IndexMap::<String, String>::with_capacity(10_000));
}

#[bench]
fn insert_hashmap_10_000(b: &mut Bencher) {
    let c = 10_000;
    b.iter(|| {
        let mut map = HashMap::with_capacity(c);
        for x in 0..c {
            map.insert(x, ());
        }
        map
    });
}

#[bench]
fn insert_indexmap_10_000(b: &mut Bencher) {
    let c = 10_000;
    b.iter(|| {
        let mut map = IndexMap::with_capacity(c);
        for x in 0..c {
            map.insert(x, ());
        }
        map
    });
}

#[bench]
fn insert_hashmap_string_10_000(b: &mut Bencher) {
    let c = 10_000;
    b.iter(|| {
        let mut map = HashMap::with_capacity(c);
        for x in 0..c {
            map.insert(x.to_string(), ());
        }
        map
    });
}

#[bench]
fn insert_indexmap_string_10_000(b: &mut Bencher) {
    let c = 10_000;
    b.iter(|| {
        let mut map = IndexMap::with_capacity(c);
        for x in 0..c {
            map.insert(x.to_string(), ());
        }
        map
    });
}

#[bench]
fn insert_hashmap_str_10_000(b: &mut Bencher) {
    let c = 10_000;
    let ss = Vec::from_iter((0..c).map(|x| x.to_string()));
    b.iter(|| {
        let mut map = HashMap::with_capacity(c);
        for key in &ss {
            map.insert(&key[..], ());
        }
        map
    });
}

#[bench]
fn insert_indexmap_str_10_000(b: &mut Bencher) {
    let c = 10_000;
    let ss = Vec::from_iter((0..c).map(|x| x.to_string()));
    b.iter(|| {
        let mut map = IndexMap::with_capacity(c);
        for key in &ss {
            map.insert(&key[..], ());
        }
        map
    });
}

#[bench]
fn insert_hashmap_int_bigvalue_10_000(b: &mut Bencher) {
    let c = 10_000;
    let value = [0u64; 10];
    b.iter(|| {
        let mut map = HashMap::with_capacity(c);
        for i in 0..c {
            map.insert(i, value);
        }
        map
    });
}

#[bench]
fn insert_indexmap_int_bigvalue_10_000(b: &mut Bencher) {
    let c = 10_000;
    let value = [0u64; 10];
    b.iter(|| {
        let mut map = IndexMap::with_capacity(c);
        for i in 0..c {
            map.insert(i, value);
        }
        map
    });
}

#[bench]
fn insert_hashmap_100_000(b: &mut Bencher) {
    let c = 100_000;
    b.iter(|| {
        let mut map = HashMap::with_capacity(c);
        for x in 0..c {
            map.insert(x, ());
        }
        map
    });
}

#[bench]
fn insert_indexmap_100_000(b: &mut Bencher) {
    let c = 100_000;
    b.iter(|| {
        let mut map = IndexMap::with_capacity(c);
        for x in 0..c {
            map.insert(x, ());
        }
        map
    });
}

#[bench]
fn insert_hashmap_150(b: &mut Bencher) {
    let c = 150;
    b.iter(|| {
        let mut map = HashMap::with_capacity(c);
        for x in 0..c {
            map.insert(x, ());
        }
        map
    });
}

#[bench]
fn insert_indexmap_150(b: &mut Bencher) {
    let c = 150;
    b.iter(|| {
        let mut map = IndexMap::with_capacity(c);
        for x in 0..c {
            map.insert(x, ());
        }
        map
    });
}

#[bench]
fn entry_hashmap_150(b: &mut Bencher) {
    let c = 150;
    b.iter(|| {
        let mut map = HashMap::with_capacity(c);
        for x in 0..c {
            map.entry(x).or_insert(());
        }
        map
    });
}

#[bench]
fn entry_indexmap_150(b: &mut Bencher) {
    let c = 150;
    b.iter(|| {
        let mut map = IndexMap::with_capacity(c);
        for x in 0..c {
            map.entry(x).or_insert(());
        }
        map
    });
}

#[bench]
fn iter_sum_hashmap_10_000(b: &mut Bencher) {
    let c = 10_000;
    let mut map = HashMap::with_capacity(c);
    let len = c - c / 10;
    for x in 0..len {
        map.insert(x, ());
    }
    assert_eq!(map.len(), len);
    b.iter(|| map.keys().sum::<usize>());
}

#[bench]
fn iter_sum_indexmap_10_000(b: &mut Bencher) {
    let c = 10_000;
    let mut map = IndexMap::with_capacity(c);
    let len = c - c / 10;
    for x in 0..len {
        map.insert(x, ());
    }
    assert_eq!(map.len(), len);
    b.iter(|| map.keys().sum::<usize>());
}

#[bench]
fn iter_black_box_hashmap_10_000(b: &mut Bencher) {
    let c = 10_000;
    let mut map = HashMap::with_capacity(c);
    let len = c - c / 10;
    for x in 0..len {
        map.insert(x, ());
    }
    assert_eq!(map.len(), len);
    b.iter(|| {
        for &key in map.keys() {
            black_box(key);
        }
    });
}

#[bench]
fn iter_black_box_indexmap_10_000(b: &mut Bencher) {
    let c = 10_000;
    let mut map = IndexMap::with_capacity(c);
    let len = c - c / 10;
    for x in 0..len {
        map.insert(x, ());
    }
    assert_eq!(map.len(), len);
    b.iter(|| {
        for &key in map.keys() {
            black_box(key);
        }
    });
}

fn shuffled_keys<I>(iter: I) -> Vec<I::Item>
where
    I: IntoIterator,
{
    let mut v = Vec::from_iter(iter);
    let mut rng = small_rng();
    v.shuffle(&mut rng);
    v
}

#[bench]
fn lookup_hashmap_10_000_exist(b: &mut Bencher) {
    let c = 10_000;
    let mut map = HashMap::with_capacity(c);
    let keys = shuffled_keys(0..c);
    for &key in &keys {
        map.insert(key, 1);
    }
    b.iter(|| {
        let mut found = 0;
        for key in 5000..c {
            found += map.get(&key).is_some() as i32;
        }
        found
    });
}

#[bench]
fn lookup_hashmap_10_000_noexist(b: &mut Bencher) {
    let c = 10_000;
    let mut map = HashMap::with_capacity(c);
    let keys = shuffled_keys(0..c);
    for &key in &keys {
        map.insert(key, 1);
    }
    b.iter(|| {
        let mut found = 0;
        for key in c..15000 {
            found += map.get(&key).is_some() as i32;
        }
        found
    });
}

#[bench]
fn lookup_indexmap_10_000_exist(b: &mut Bencher) {
    let c = 10_000;
    let mut map = IndexMap::with_capacity(c);
    let keys = shuffled_keys(0..c);
    for &key in &keys {
        map.insert(key, 1);
    }
    b.iter(|| {
        let mut found = 0;
        for key in 5000..c {
            found += map.get(&key).is_some() as i32;
        }
        found
    });
}

#[bench]
fn lookup_indexmap_10_000_noexist(b: &mut Bencher) {
    let c = 10_000;
    let mut map = IndexMap::with_capacity(c);
    let keys = shuffled_keys(0..c);
    for &key in &keys {
        map.insert(key, 1);
    }
    b.iter(|| {
        let mut found = 0;
        for key in c..15000 {
            found += map.get(&key).is_some() as i32;
        }
        found
    });
}

// number of items to look up
const LOOKUP_MAP_SIZE: u32 = 100_000_u32;
const LOOKUP_SAMPLE_SIZE: u32 = 5000;
const SORT_MAP_SIZE: usize = 10_000;

// use lazy_static so that comparison benchmarks use the exact same inputs
lazy_static! {
    static ref KEYS: Vec<u32> = shuffled_keys(0..LOOKUP_MAP_SIZE);
}

lazy_static! {
    static ref HMAP_100K: HashMap<u32, u32> = {
        let c = LOOKUP_MAP_SIZE;
        let mut map = HashMap::with_capacity(c as usize);
        let keys = &*KEYS;
        for &key in keys {
            map.insert(key, key);
        }
        map
    };
}

lazy_static! {
    static ref IMAP_100K: IndexMap<u32, u32> = {
        let c = LOOKUP_MAP_SIZE;
        let mut map = IndexMap::with_capacity(c as usize);
        let keys = &*KEYS;
        for &key in keys {
            map.insert(key, key);
        }
        map
    };
}

lazy_static! {
    static ref IMAP_SORT_U32: IndexMap<u32, u32> = {
        let mut map = IndexMap::with_capacity(SORT_MAP_SIZE);
        for &key in &KEYS[..SORT_MAP_SIZE] {
            map.insert(key, key);
        }
        map
    };
}
lazy_static! {
    static ref IMAP_SORT_S: IndexMap<String, String> = {
        let mut map = IndexMap::with_capacity(SORT_MAP_SIZE);
        for &key in &KEYS[..SORT_MAP_SIZE] {
            map.insert(format!("{:^16x}", &key), String::new());
        }
        map
    };
}

#[bench]
fn lookup_hashmap_100_000_multi(b: &mut Bencher) {
    let map = &*HMAP_100K;
    b.iter(|| {
        let mut found = 0;
        for key in 0..LOOKUP_SAMPLE_SIZE {
            found += map.get(&key).is_some() as u32;
        }
        found
    });
}

#[bench]
fn lookup_indexmap_100_000_multi(b: &mut Bencher) {
    let map = &*IMAP_100K;
    b.iter(|| {
        let mut found = 0;
        for key in 0..LOOKUP_SAMPLE_SIZE {
            found += map.get(&key).is_some() as u32;
        }
        found
    });
}

// inorder: Test looking up keys in the same order as they were inserted
#[bench]
fn lookup_hashmap_100_000_inorder_multi(b: &mut Bencher) {
    let map = &*HMAP_100K;
    let keys = &*KEYS;
    b.iter(|| {
        let mut found = 0;
        for key in &keys[0..LOOKUP_SAMPLE_SIZE as usize] {
            found += map.get(key).is_some() as u32;
        }
        found
    });
}

#[bench]
fn lookup_indexmap_100_000_inorder_multi(b: &mut Bencher) {
    let map = &*IMAP_100K;
    let keys = &*KEYS;
    b.iter(|| {
        let mut found = 0;
        for key in &keys[0..LOOKUP_SAMPLE_SIZE as usize] {
            found += map.get(key).is_some() as u32;
        }
        found
    });
}

#[bench]
fn lookup_hashmap_100_000_single(b: &mut Bencher) {
    let map = &*HMAP_100K;
    let mut iter = (0..LOOKUP_MAP_SIZE + LOOKUP_SAMPLE_SIZE).cycle();
    b.iter(|| {
        let key = iter.next().unwrap();
        map.get(&key).is_some()
    });
}

#[bench]
fn lookup_indexmap_100_000_single(b: &mut Bencher) {
    let map = &*IMAP_100K;
    let mut iter = (0..LOOKUP_MAP_SIZE + LOOKUP_SAMPLE_SIZE).cycle();
    b.iter(|| {
        let key = iter.next().unwrap();
        map.get(&key).is_some()
    });
}

const GROW_SIZE: usize = 100_000;
type GrowKey = u32;

// Test grow/resize without preallocation
#[bench]
fn grow_fnv_hashmap_100_000(b: &mut Bencher) {
    b.iter(|| {
        let mut map: HashMap<_, _, FnvBuilder> = HashMap::default();
        for x in 0..GROW_SIZE {
            map.insert(x as GrowKey, x as GrowKey);
        }
        map
    });
}

#[bench]
fn grow_fnv_indexmap_100_000(b: &mut Bencher) {
    b.iter(|| {
        let mut map: IndexMap<_, _, FnvBuilder> = IndexMap::default();
        for x in 0..GROW_SIZE {
            map.insert(x as GrowKey, x as GrowKey);
        }
        map
    });
}

const MERGE: u64 = 10_000;
#[bench]
fn hashmap_merge_simple(b: &mut Bencher) {
    let first_map: HashMap<u64, _> = (0..MERGE).map(|i| (i, ())).collect();
    let second_map: HashMap<u64, _> = (MERGE..MERGE * 2).map(|i| (i, ())).collect();
    b.iter(|| {
        let mut merged = first_map.clone();
        merged.extend(second_map.iter().map(|(&k, &v)| (k, v)));
        merged
    });
}

#[bench]
fn hashmap_merge_shuffle(b: &mut Bencher) {
    let first_map: HashMap<u64, _> = (0..MERGE).map(|i| (i, ())).collect();
    let second_map: HashMap<u64, _> = (MERGE..MERGE * 2).map(|i| (i, ())).collect();
    let mut v = Vec::new();
    let mut rng = small_rng();
    b.iter(|| {
        let mut merged = first_map.clone();
        v.extend(second_map.iter().map(|(&k, &v)| (k, v)));
        v.shuffle(&mut rng);
        merged.extend(v.drain(..));

        merged
    });
}

#[bench]
fn indexmap_merge_simple(b: &mut Bencher) {
    let first_map: IndexMap<u64, _> = (0..MERGE).map(|i| (i, ())).collect();
    let second_map: IndexMap<u64, _> = (MERGE..MERGE * 2).map(|i| (i, ())).collect();
    b.iter(|| {
        let mut merged = first_map.clone();
        merged.extend(second_map.iter().map(|(&k, &v)| (k, v)));
        merged
    });
}

#[bench]
fn indexmap_merge_shuffle(b: &mut Bencher) {
    let first_map: IndexMap<u64, _> = (0..MERGE).map(|i| (i, ())).collect();
    let second_map: IndexMap<u64, _> = (MERGE..MERGE * 2).map(|i| (i, ())).collect();
    let mut v = Vec::new();
    let mut rng = small_rng();
    b.iter(|| {
        let mut merged = first_map.clone();
        v.extend(second_map.iter().map(|(&k, &v)| (k, v)));
        v.shuffle(&mut rng);
        merged.extend(v.drain(..));

        merged
    });
}

#[bench]
fn swap_remove_indexmap_100_000(b: &mut Bencher) {
    let map = IMAP_100K.clone();
    let mut keys = Vec::from_iter(map.keys().copied());
    let mut rng = small_rng();
    keys.shuffle(&mut rng);

    b.iter(|| {
        let mut map = map.clone();
        for key in &keys {
            map.swap_remove(key);
        }
        assert_eq!(map.len(), 0);
        map
    });
}

#[bench]
fn shift_remove_indexmap_100_000_few(b: &mut Bencher) {
    let map = IMAP_100K.clone();
    let mut keys = Vec::from_iter(map.keys().copied());
    let mut rng = small_rng();
    keys.shuffle(&mut rng);
    keys.truncate(50);

    b.iter(|| {
        let mut map = map.clone();
        for key in &keys {
            map.shift_remove(key);
        }
        assert_eq!(map.len(), IMAP_100K.len() - keys.len());
        map
    });
}

#[bench]
fn shift_remove_indexmap_2_000_full(b: &mut Bencher) {
    let mut keys = KEYS[..2_000].to_vec();
    let mut map = IndexMap::with_capacity(keys.len());
    for &key in &keys {
        map.insert(key, key);
    }
    let mut rng = small_rng();
    keys.shuffle(&mut rng);

    b.iter(|| {
        let mut map = map.clone();
        for key in &keys {
            map.shift_remove(key);
        }
        assert_eq!(map.len(), 0);
        map
    });
}

#[bench]
fn pop_indexmap_100_000(b: &mut Bencher) {
    let map = IMAP_100K.clone();

    b.iter(|| {
        let mut map = map.clone();
        while !map.is_empty() {
            map.pop();
        }
        assert_eq!(map.len(), 0);
        map
    });
}

#[bench]
fn few_retain_indexmap_100_000(b: &mut Bencher) {
    let map = IMAP_100K.clone();

    b.iter(|| {
        let mut map = map.clone();
        map.retain(|k, _| *k % 7 == 0);
        map
    });
}

#[bench]
fn few_retain_hashmap_100_000(b: &mut Bencher) {
    let map = HMAP_100K.clone();

    b.iter(|| {
        let mut map = map.clone();
        map.retain(|k, _| *k % 7 == 0);
        map
    });
}

#[bench]
fn half_retain_indexmap_100_000(b: &mut Bencher) {
    let map = IMAP_100K.clone();

    b.iter(|| {
        let mut map = map.clone();
        map.retain(|k, _| *k % 2 == 0);
        map
    });
}

#[bench]
fn half_retain_hashmap_100_000(b: &mut Bencher) {
    let map = HMAP_100K.clone();

    b.iter(|| {
        let mut map = map.clone();
        map.retain(|k, _| *k % 2 == 0);
        map
    });
}

#[bench]
fn many_retain_indexmap_100_000(b: &mut Bencher) {
    let map = IMAP_100K.clone();

    b.iter(|| {
        let mut map = map.clone();
        map.retain(|k, _| *k % 100 != 0);
        map
    });
}

#[bench]
fn many_retain_hashmap_100_000(b: &mut Bencher) {
    let map = HMAP_100K.clone();

    b.iter(|| {
        let mut map = map.clone();
        map.retain(|k, _| *k % 100 != 0);
        map
    });
}

// simple sort impl for comparison
pub fn simple_sort<K: Ord + Hash, V>(m: &mut IndexMap<K, V>) {
    let mut ordered: Vec<_> = m.drain(..).collect();
    ordered.sort_by(|left, right| left.0.cmp(&right.0));
    m.extend(ordered);
}

#[bench]
fn indexmap_sort_s(b: &mut Bencher) {
    let map = IMAP_SORT_S.clone();

    // there's a map clone there, but it's still useful to profile this
    b.iter(|| {
        let mut map = map.clone();
        map.sort_keys();
        map
    });
}

#[bench]
fn indexmap_simple_sort_s(b: &mut Bencher) {
    let map = IMAP_SORT_S.clone();

    // there's a map clone there, but it's still useful to profile this
    b.iter(|| {
        let mut map = map.clone();
        simple_sort(&mut map);
        map
    });
}

#[bench]
fn indexmap_sort_u32(b: &mut Bencher) {
    let map = IMAP_SORT_U32.clone();

    // there's a map clone there, but it's still useful to profile this
    b.iter(|| {
        let mut map = map.clone();
        map.sort_keys();
        map
    });
}

#[bench]
fn indexmap_simple_sort_u32(b: &mut Bencher) {
    let map = IMAP_SORT_U32.clone();

    // there's a map clone there, but it's still useful to profile this
    b.iter(|| {
        let mut map = map.clone();
        simple_sort(&mut map);
        map
    });
}

// measure the fixed overhead of cloning in sort benchmarks
#[bench]
fn indexmap_clone_for_sort_s(b: &mut Bencher) {
    let map = IMAP_SORT_S.clone();

    b.iter(|| map.clone());
}

#[bench]
fn indexmap_clone_for_sort_u32(b: &mut Bencher) {
    let map = IMAP_SORT_U32.clone();

    b.iter(|| map.clone());
}
