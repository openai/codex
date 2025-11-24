//! Proper testing, with QuickCheck.

use crate::{Trie, TrieCommon, TrieKey};
use quickcheck::{quickcheck, Arbitrary, Gen};
use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key(Vec<u8>);

#[derive(Clone, Debug)]
struct RandomKeys(HashSet<Key>);

const MAX_KEYS: usize = 512;
const KEY_RUN_LEN: usize = 8;
const KEY_MAX_VAL: u8 = 4;

impl Arbitrary for Key {
    fn arbitrary<G: Gen>(g: &mut G) -> Key {
        let len = g.gen::<usize>() % KEY_RUN_LEN;
        let mut key = Vec::with_capacity(len);
        for _ in 0..len {
            key.push(g.gen::<u8>() % KEY_MAX_VAL);
        }
        Key(key)
    }
}

impl Key {
    fn extend_random<G: Gen>(&self, g: &mut G) -> Key {
        self.extend(Key::arbitrary(g))
    }

    fn extend(&self, other: Key) -> Key {
        let mut key = self.clone();
        key.0.extend(other.0);
        key
    }

    fn len(&self) -> usize {
        self.0.len()
    }
}

impl TrieKey for Key {
    fn encode_bytes(&self) -> Vec<u8> {
        self.0.clone()
    }
}

impl Arbitrary for RandomKeys {
    fn arbitrary<G: Gen>(g: &mut G) -> RandomKeys {
        let num_keys = g.gen::<usize>() % MAX_KEYS;
        let mut keys = Vec::with_capacity(num_keys);
        keys.push(Key::arbitrary(g));

        for _ in 0..num_keys {
            match g.gen::<u8>() % 10 {
                // Generate a new random key.
                1 => keys.push(Key::arbitrary(g)),
                // Extend an existing key.
                _ => {
                    let i = g.gen::<usize>() % keys.len();
                    let key = keys[i].extend_random(g);
                    keys.push(key);
                }
            }
        }

        RandomKeys(HashSet::from_iter(keys))
    }
}

#[test]
fn insert_all_remove_all() {
    fn prop(RandomKeys(keys): RandomKeys) -> bool {
        let mut trie = Trie::new();
        let mut length = 0;

        for k in &keys {
            if trie.insert(k.clone(), k.len()).is_some() {
                return false;
            }
            length += 1;
            if trie.len() != length {
                return false;
            }
        }

        if !trie.check_integrity() {
            return false;
        }

        for k in &keys {
            if trie.get(&k) != Some(&k.len()) {
                return false;
            }
            if trie.remove(&k) != Some(k.len()) {
                return false;
            }
            length -= 1;
            if trie.len() != length {
                return false;
            }
            if trie.get(&k).is_some() {
                return false;
            }
        }
        trie.check_integrity()
    }

    quickcheck(prop as fn(RandomKeys) -> bool);
}

#[test]
fn subtrie() {
    fn prop(RandomKeys(keys): RandomKeys) -> bool {
        let half = keys.len() / 2;
        let first_half = keys.iter().take(half).map(|k| (k.clone(), k.len()));
        let trie = Trie::from_iter(first_half);

        // Check node existence for inserted keys.
        for k in keys.iter().take(half) {
            match trie.subtrie(&k) {
                Some(node) => {
                    if node.value() != Some(&k.len()) {
                        return false;
                    }
                }
                None => return false,
            }
        }

        // Check that nodes for non-inserted keys don't have values.
        for k in keys.iter().skip(half) {
            if let Some(node) = trie.subtrie(&k) {
                if node.value().is_some() {
                    return false;
                }
            }
        }

        trie.check_integrity()
    }

    quickcheck(prop as fn(RandomKeys) -> bool);
}

// trie.subtrie(k1).get(k2) should be the same as trie.get(k2) if k1 is a prefix of k2.
#[test]
fn subtrie_get() {
    fn prop(trie_keys: RandomKeys, k1: Key, k2: Key) -> bool {
        let mut trie = length_trie(trie_keys.0);
        trie.insert(k1.clone(), k1.len());

        let subtrie = trie.subtrie(&k1).unwrap();

        if k2.0.starts_with(&k1.0) {
            subtrie.get(&k2).unwrap() == trie.get(&k2)
        } else {
            subtrie.get(&k2).is_err()
        }
    }

    quickcheck(prop as fn(RandomKeys, Key, Key) -> bool);
}

#[test]
fn subtrie_mut_get() {
    fn prop(trie_keys: RandomKeys, k1: Key, k2: Key) -> bool {
        let mut trie = length_trie(trie_keys.0);
        trie.insert(k1.clone(), k1.len());

        let subtrie = trie.subtrie_mut(&k1).unwrap();

        let ok = if k2.0.starts_with(&k1.0) {
            subtrie.get(&k2).is_ok()
        } else {
            subtrie.get(&k2).is_err()
        };
        ok && trie.check_integrity()
    }

    quickcheck(prop as fn(RandomKeys, Key, Key) -> bool);
}

#[test]
fn subtrie_insert() {
    fn prop(trie_keys: RandomKeys, key_suffixes: RandomKeys, k1: Key) -> bool {
        let mut trie = length_trie(trie_keys.0);
        trie.insert(k1.clone(), k1.len());

        {
            let mut subtrie = trie.subtrie_mut(&k1).unwrap();

            let insert_keys = key_suffixes
                .0
                .into_iter()
                .map(|x| k1.extend(x))
                .collect::<HashSet<_>>();

            for k in insert_keys.iter() {
                assert!(subtrie.insert(k.clone(), k.len()).is_ok());
            }

            for k in insert_keys.iter() {
                match subtrie.get(k) {
                    Ok(Some(_)) => (),
                    _ => return false,
                }
            }
        }

        trie.check_integrity()
    }

    quickcheck(prop as fn(RandomKeys, RandomKeys, Key) -> bool);
}

// Construct a trie from a set of keys, with each key mapped to its length.
fn length_trie(keys: HashSet<Key>) -> Trie<Key, usize> {
    let mut t = Trie::new();
    for k in keys {
        let len = k.len();
        t.insert(k, len);
    }
    t
}

#[test]
fn remove_non_existent() {
    fn prop(RandomKeys(insert_keys): RandomKeys, RandomKeys(remove_keys): RandomKeys) -> bool {
        let mut trie = length_trie(insert_keys.clone());

        for k in remove_keys {
            if !insert_keys.contains(&k) && trie.remove(&k).is_some() {
                return false;
            }
        }
        trie.check_integrity()
    }
    quickcheck(prop as fn(RandomKeys, RandomKeys) -> bool);
}

#[test]
fn keys_iter() {
    fn prop(RandomKeys(keys): RandomKeys) -> bool {
        let trie = length_trie(keys.clone());
        let trie_keys: HashSet<Key> = trie.keys().cloned().collect();
        trie_keys == keys
    }
    quickcheck(prop as fn(RandomKeys) -> bool);
}

#[test]
fn values_iter() {
    // Create a map of values to frequencies.
    fn frequency_map<I: Iterator<Item = usize>>(values: I) -> HashMap<usize, u64> {
        let mut map = HashMap::new();
        for v in values {
            let current_val = map.entry(v).or_insert(0);
            *current_val += 1;
        }
        map
    }

    fn prop(RandomKeys(keys): RandomKeys) -> bool {
        let trie = length_trie(keys.clone());
        let trie_values: HashMap<usize, u64> = frequency_map(trie.values().cloned());
        let key_values = frequency_map(keys.into_iter().map(|k| k.len()));
        trie_values == key_values
    }
    quickcheck(prop as fn(RandomKeys) -> bool);
}
