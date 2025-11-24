#![cfg(feature = "serde")]

use core::hash::BuildHasherDefault;
use fnv::FnvHasher;
use hashbrown::{HashMap, HashSet};
use serde_test::{assert_tokens, Token};

// We use FnvHash for this test because we rely on the ordering
type FnvHashMap<K, V> = HashMap<K, V, BuildHasherDefault<FnvHasher>>;
type FnvHashSet<T> = HashSet<T, BuildHasherDefault<FnvHasher>>;

#[test]
fn map_serde_tokens_empty() {
    let map = FnvHashMap::<char, u32>::default();

    assert_tokens(&map, &[Token::Map { len: Some(0) }, Token::MapEnd]);
}

#[test]
fn map_serde_tokens() {
    let mut map = FnvHashMap::default();
    map.insert('b', 20);
    map.insert('a', 10);
    map.insert('c', 30);

    assert_tokens(
        &map,
        &[
            Token::Map { len: Some(3) },
            Token::Char('a'),
            Token::I32(10),
            Token::Char('c'),
            Token::I32(30),
            Token::Char('b'),
            Token::I32(20),
            Token::MapEnd,
        ],
    );
}

#[test]
fn set_serde_tokens_empty() {
    let set = FnvHashSet::<u32>::default();

    assert_tokens(&set, &[Token::Seq { len: Some(0) }, Token::SeqEnd]);
}

#[test]
fn set_serde_tokens() {
    let mut set = FnvHashSet::default();
    set.insert(20);
    set.insert(10);
    set.insert(30);

    assert_tokens(
        &set,
        &[
            Token::Seq { len: Some(3) },
            Token::I32(30),
            Token::I32(20),
            Token::I32(10),
            Token::SeqEnd,
        ],
    );
}
