// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use postcard::ser_flavors::{AllocVec, Flavor};
use serde::Serialize;
use zerotrie::ZeroTriePerfectHash;
use zerotrie::ZeroTrieSimpleAscii;
use zerovec::ZeroMap;

mod testdata {
    include!("data/data.rs");
}

#[test]
fn test_basic() {
    let bytes_ascii = testdata::basic::TRIE_ASCII;
    let data_ascii = testdata::basic::DATA_ASCII;
    let trie_ascii = ZeroTrieSimpleAscii::from_bytes(bytes_ascii);
    let trie_phf_ascii = ZeroTriePerfectHash::from_bytes(bytes_ascii);

    let bytes_unicode = testdata::basic::TRIE_UNICODE;
    let data_unicode = testdata::basic::DATA_UNICODE;
    let trie_phf_unicode = ZeroTriePerfectHash::from_bytes(bytes_unicode);

    let bytes_binary = testdata::basic::TRIE_BINARY;
    let data_binary = testdata::basic::DATA_BINARY;
    let trie_phf_binary = ZeroTriePerfectHash::from_bytes(bytes_binary);

    // Check that the getter works
    for (key, expected) in data_ascii {
        let actual = match trie_ascii.get(key) {
            Some(v) => v,
            None => panic!("value should be in trie: {:?} => {}", key, expected),
        };
        assert_eq!(*expected, actual);
        let actual = match trie_phf_ascii.get(key) {
            Some(v) => v,
            None => panic!("value should be in trie6: {:?} => {}", key, expected),
        };
        assert_eq!(*expected, actual);
    }

    for (key, expected) in data_unicode {
        let actual_unicode = match trie_phf_unicode.get(key) {
            Some(v) => v,
            None => panic!("value should be in trie6: {:?} => {}", key, expected),
        };
        assert_eq!(*expected, actual_unicode);
    }

    for (key, expected) in data_binary {
        let actual_bin6 = match trie_phf_binary.get(key) {
            Some(v) => v,
            None => panic!("value should be in trie6: {:?} => {}", key, expected),
        };
        assert_eq!(*expected, actual_bin6);
    }

    // Compare the size to a postcard ZeroMap
    let zm: ZeroMap<[u8], u32> = data_ascii.iter().map(|(a, b)| (*a, *b as u32)).collect();
    let mut serializer = postcard::Serializer {
        output: AllocVec::new(),
    };
    Serialize::serialize(&zm, &mut serializer).unwrap();
    let zeromap_bytes = serializer
        .output
        .finalize()
        .expect("Failed to finalize serializer output");

    assert_eq!(26, bytes_ascii.len());
    assert_eq!(77, zeromap_bytes.len());
}
