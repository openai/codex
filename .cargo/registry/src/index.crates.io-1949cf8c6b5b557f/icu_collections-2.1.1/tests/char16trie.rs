// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use icu_collections::char16trie::{Char16Trie, TrieResult};
use zerovec::ZeroVec;

#[test]
fn empty() {
    let trie_data = toml::from_str::<TestFile>(include_str!("data/char16trie/empty.toml"))
        .unwrap()
        .ucharstrie
        .data;
    let trie = Char16Trie::new(ZeroVec::from_slice_or_alloc(trie_data.as_slice()));
    let res = trie.iter().next('h');
    assert_eq!(res, TrieResult::NoMatch);
}

#[test]
fn a() {
    let trie_data = toml::from_str::<TestFile>(include_str!("data/char16trie/test_a.toml"))
        .unwrap()
        .ucharstrie
        .data;
    let trie = Char16Trie::new(ZeroVec::from_slice_or_alloc(trie_data.as_slice()));

    let mut iter = trie.iter();
    let res = iter.next('h');
    assert_eq!(res, TrieResult::NoMatch);

    let mut iter = trie.iter();
    let res = iter.next('a');
    assert_eq!(res, TrieResult::FinalValue(1));
    let res = iter.next('a');
    assert_eq!(res, TrieResult::NoMatch);
}

#[test]
fn a_b() {
    let trie_data = toml::from_str::<TestFile>(include_str!("data/char16trie/test_a_ab.toml"))
        .unwrap()
        .ucharstrie
        .data;
    let trie = Char16Trie::new(ZeroVec::from_slice_or_alloc(trie_data.as_slice()));

    let mut iter = trie.iter();
    let res = iter.next('a');
    assert_eq!(res, TrieResult::Intermediate(1));
    let res = iter.next('a');
    assert_eq!(res, TrieResult::NoMatch);

    let mut iter = trie.iter();
    let res = iter.next('a');
    assert_eq!(res, TrieResult::Intermediate(1));
    let res = iter.next('b');
    assert_eq!(res, TrieResult::FinalValue(100));
    let res = iter.next('b');
    assert_eq!(res, TrieResult::NoMatch);
}

#[test]
fn shortest_branch() {
    let trie_data =
        toml::from_str::<TestFile>(include_str!("data/char16trie/test_shortest_branch.toml"))
            .unwrap()
            .ucharstrie
            .data;
    let trie = Char16Trie::new(ZeroVec::from_slice_or_alloc(trie_data.as_slice()));

    let mut iter = trie.iter();
    let res = iter.next('a');
    assert_eq!(res, TrieResult::FinalValue(1000));
    let res = iter.next('b');
    assert_eq!(res, TrieResult::NoMatch);

    let mut iter = trie.iter();
    let res = iter.next('b');
    assert_eq!(res, TrieResult::FinalValue(2000));
    let res = iter.next('a');
    assert_eq!(res, TrieResult::NoMatch);
}

#[test]
fn branches() {
    let trie_data = toml::from_str::<TestFile>(include_str!("data/char16trie/test_branches.toml"))
        .unwrap()
        .ucharstrie
        .data;
    let trie = Char16Trie::new(ZeroVec::from_slice_or_alloc(trie_data.as_slice()));

    for (query, expected) in [
        ("a", TrieResult::FinalValue(0x10)),
        ("cc", TrieResult::FinalValue(0x40)),
        ("e", TrieResult::FinalValue(0x100)),
        ("ggg", TrieResult::FinalValue(0x400)),
        ("i", TrieResult::FinalValue(0x1000)),
        ("kkkk", TrieResult::FinalValue(0x4000)),
        ("n", TrieResult::FinalValue(0x10000)),
        ("ppppp", TrieResult::FinalValue(0x40000)),
        ("r", TrieResult::FinalValue(0x100000)),
        ("sss", TrieResult::FinalValue(0x200000)),
        ("t", TrieResult::FinalValue(0x400000)),
        ("uu", TrieResult::FinalValue(0x800000)),
        ("vv", TrieResult::FinalValue(0x7fffffff)),
        ("zz", TrieResult::FinalValue(-2147483648)),
    ] {
        let mut iter = trie.iter();
        for (i, chr) in query.chars().enumerate() {
            let res = iter.next(chr);
            if i + 1 == query.len() {
                assert_eq!(res, expected);
            } else {
                assert_eq!(res, TrieResult::NoValue);
            }
        }
    }
}

#[test]
fn long_sequence() {
    let trie_data =
        toml::from_str::<TestFile>(include_str!("data/char16trie/test_long_sequence.toml"))
            .unwrap()
            .ucharstrie
            .data;
    let trie = Char16Trie::new(ZeroVec::from_slice_or_alloc(trie_data.as_slice()));

    for (query, expected) in [
        ("a", TrieResult::Intermediate(-1)),
        // sequence of linear-match nodes
        (
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
            TrieResult::Intermediate(-2),
        ),
        // more than 256 units
        (
            concat!(
                "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
                "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
                "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
                "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
                "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
                "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
            ),
            TrieResult::FinalValue(-3),
        ),
    ] {
        let mut iter = trie.iter();
        for (i, chr) in query.chars().enumerate() {
            let res = iter.next(chr);
            if i + 1 == query.len() {
                assert_eq!(res, expected);
            } else if i == 0 {
                assert_eq!(res, TrieResult::Intermediate(-1));
            } else if i == 51 {
                assert_eq!(res, TrieResult::Intermediate(-2));
            } else {
                assert_eq!(res, TrieResult::NoValue);
            }
        }
    }
}

#[test]
fn long_branch() {
    let trie_data =
        toml::from_str::<TestFile>(include_str!("data/char16trie/test_long_branch.toml"))
            .unwrap()
            .ucharstrie
            .data;
    let trie = Char16Trie::new(ZeroVec::from_slice_or_alloc(trie_data.as_slice()));

    for (query, expected) in [
        ("a", TrieResult::FinalValue(-2)),
        ("b", TrieResult::FinalValue(-1)),
        ("c", TrieResult::FinalValue(0)),
        ("d2", TrieResult::FinalValue(1)),
        ("f", TrieResult::FinalValue(0x3f)),
        ("g", TrieResult::FinalValue(0x40)),
        ("h", TrieResult::FinalValue(0x41)),
        ("j23", TrieResult::FinalValue(0x1900)),
        ("j24", TrieResult::FinalValue(0x19ff)),
        ("j25", TrieResult::FinalValue(0x1a00)),
        ("k2", TrieResult::FinalValue(0x1a80)),
        ("k3", TrieResult::FinalValue(0x1aff)),
        ("l234567890", TrieResult::Intermediate(0x1b00)),
        ("l234567890123", TrieResult::FinalValue(0x1b01)),
        (
            "nnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn",
            TrieResult::FinalValue(0x10ffff),
        ),
        (
            "oooooooooooooooooooooooooooooooooooooooooooooooooooooo",
            TrieResult::FinalValue(0x110000),
        ),
        (
            "pppppppppppppppppppppppppppppppppppppppppppppppppppppp",
            TrieResult::FinalValue(0x120000),
        ),
        ("r", TrieResult::FinalValue(0x333333)),
        ("s2345", TrieResult::FinalValue(0x4444444)),
        ("t234567890", TrieResult::FinalValue(0x77777777)),
        ("z", TrieResult::FinalValue(-2147483647)),
    ] {
        let mut iter = trie.iter();
        for (i, chr) in query.chars().enumerate() {
            let res = iter.next(chr);
            if i + 1 == query.len() {
                assert_eq!(res, expected);
            } else if query == "l234567890123" && i == 9 {
                assert_eq!(res, TrieResult::Intermediate(0x1b00));
            } else {
                assert_eq!(res, TrieResult::NoValue);
            }
        }
    }
}

#[test]
fn compact() {
    let trie_data = toml::from_str::<TestFile>(include_str!("data/char16trie/test_compact.toml"))
        .unwrap()
        .ucharstrie
        .data;
    let trie = Char16Trie::new(ZeroVec::from_slice_or_alloc(trie_data.as_slice()));

    for (query, expected) in [
        ("+", TrieResult::Intermediate(0)),
        ("+august", TrieResult::FinalValue(8)),
        ("+december", TrieResult::FinalValue(12)),
        ("+july", TrieResult::FinalValue(7)),
        ("+june", TrieResult::FinalValue(6)),
        ("+november", TrieResult::FinalValue(11)),
        ("+october", TrieResult::FinalValue(10)),
        ("+september", TrieResult::FinalValue(9)),
        ("-", TrieResult::Intermediate(0)),
        ("-august", TrieResult::FinalValue(8)),
        ("-december", TrieResult::FinalValue(12)),
        ("-july", TrieResult::FinalValue(7)),
        ("-june", TrieResult::FinalValue(6)),
        ("-november", TrieResult::FinalValue(11)),
        ("-october", TrieResult::FinalValue(10)),
        ("-september", TrieResult::FinalValue(9)),
        ("xjuly", TrieResult::FinalValue(7)),
        ("xjune", TrieResult::FinalValue(6)),
    ] {
        let mut iter = trie.iter();
        for (i, chr) in query.chars().enumerate() {
            let res = iter.next(chr);
            if i + 1 == query.len() {
                assert_eq!(res, expected);
            } else if chr == '-' || chr == '+' {
                assert_eq!(res, TrieResult::Intermediate(0));
            } else {
                assert_eq!(res, TrieResult::NoValue);
            }
        }
    }
}

#[test]
fn months() {
    let trie_data = toml::from_str::<TestFile>(include_str!("data/char16trie/months.toml"))
        .unwrap()
        .ucharstrie
        .data;
    let trie = Char16Trie::new(ZeroVec::from_slice_or_alloc(trie_data.as_slice()));

    let mut iter = trie.iter();
    for (chr, expected) in [
        ('j', TrieResult::NoValue),
        ('u', TrieResult::NoValue),
        ('n', TrieResult::Intermediate(6)),
        ('e', TrieResult::FinalValue(6)),
    ] {
        let res = iter.next(chr);
        assert_eq!(res, expected);
    }
    let res = iter.next('h');
    assert_eq!(res, TrieResult::NoMatch);

    let mut iter = trie.iter();
    for (chr, expected) in [
        ('j', TrieResult::NoValue),
        ('u', TrieResult::NoValue),
        ('l', TrieResult::NoValue),
        ('y', TrieResult::FinalValue(7)),
    ] {
        let res = iter.next(chr);
        assert_eq!(res, expected);
    }
    let res = iter.next('h');
    assert_eq!(res, TrieResult::NoMatch);
}

#[derive(serde::Deserialize)]
pub struct TestFile {
    ucharstrie: Char16TrieVec,
}

#[derive(serde::Deserialize)]
pub struct Char16TrieVec {
    data: Vec<u16>,
}
