// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use icu_locale_core::extensions::private::Private;
use icu_locale_core::Locale;
use litemap::LiteMap;
use std::collections::BTreeSet;
use writeable::Writeable;
use zerotrie::ZeroTriePerfectHash;
use zerotrie::ZeroTrieSimpleAscii;
use zerovec::VarZeroVec;

mod testdata {
    include!("data/data.rs");
}

use testdata::locales_with_aux::{NUM_UNIQUE_BLOBS, STRINGS};
use testdata::strings_to_litemap;

#[test]
fn test_combined() {
    let litemap = strings_to_litemap(STRINGS);

    let vzv: VarZeroVec<str> = STRINGS.into();

    // Lookup table size:
    assert_eq!(vzv.as_bytes().len(), 10219);

    // Size including pointer array:
    assert_eq!(
        vzv.as_bytes().len() + STRINGS.len() * core::mem::size_of::<usize>(),
        18635
    );

    let trie = ZeroTrieSimpleAscii::try_from(&litemap).unwrap();

    // Lookup table size:
    assert_eq!(trie.byte_len(), 5104);

    // Size including pointer array:
    assert_eq!(
        trie.byte_len() + NUM_UNIQUE_BLOBS * core::mem::size_of::<usize>(),
        8392
    );

    let trie = ZeroTriePerfectHash::try_from(&litemap).unwrap();

    // Lookup table size:
    assert_eq!(trie.byte_len(), 5157);

    // Size including pointer array:
    assert_eq!(
        trie.byte_len() + NUM_UNIQUE_BLOBS * core::mem::size_of::<usize>(),
        8445
    );

    let total_str_len = litemap.keys().map(|k| k.len()).sum::<usize>();
    assert_eq!(total_str_len, 8115);

    // Lookup table size:
    assert_eq!(
        total_str_len + STRINGS.len() * core::mem::size_of::<usize>(),
        16531
    );

    // Size including pointer array: (2x for the lookup array and value array)
    assert_eq!(
        total_str_len + 2 * STRINGS.len() * core::mem::size_of::<usize>(),
        24947
    );

    // Size including u16 pointer array:
    assert_eq!(
        total_str_len
            + STRINGS.len() * core::mem::size_of::<usize>()
            + STRINGS.len() * core::mem::size_of::<u16>()
            + NUM_UNIQUE_BLOBS * core::mem::size_of::<usize>(),
        21923
    );
}

#[test]
fn test_aux_split() {
    let locales: Vec<Locale> = STRINGS.iter().map(|s| s.parse().unwrap()).collect();

    let aux_keys: BTreeSet<&Private> = locales.iter().map(|l| &l.extensions.private).collect();
    assert_eq!(aux_keys.len(), 6);

    let mut cumulative_index = 0;
    let mut total_simpleascii_len = 0;
    let mut total_perfecthash_len = 0;
    let mut total_vzv_len = 0;
    let mut unique_locales = BTreeSet::new();
    for private in aux_keys.iter() {
        let current_locales: Vec<Locale> = locales
            .iter()
            .filter(|l| l.extensions.private == **private)
            .map(|l| {
                let mut l = l.clone();
                l.extensions.private = Private::default();
                l
            })
            .collect();
        let litemap: LiteMap<Vec<u8>, usize> = current_locales
            .iter()
            .map(|l| {
                (l.write_to_string().into_owned().into_bytes(), {
                    cumulative_index += 1;
                    cumulative_index - 1
                })
            })
            .collect();

        let trie = ZeroTrieSimpleAscii::try_from(&litemap).unwrap();
        total_simpleascii_len += trie.byte_len();

        let trie = ZeroTriePerfectHash::try_from(&litemap).unwrap();
        total_perfecthash_len += trie.byte_len();

        for k in litemap.keys() {
            unique_locales.insert(k.clone());
        }

        let strs: Vec<String> = current_locales
            .iter()
            .map(|l| l.write_to_string().into_owned())
            .collect();
        let vzv: VarZeroVec<str> = strs.as_slice().into();
        total_vzv_len += vzv.as_bytes().len();
    }
    assert_eq!(cumulative_index, locales.len());

    assert_eq!(total_simpleascii_len, 5098);
    assert_eq!(total_perfecthash_len, 5302);
    assert_eq!(total_vzv_len, 5486);

    let total_unique_locale_str_len = unique_locales.iter().map(|v| v.len()).sum::<usize>();
    assert_eq!(total_unique_locale_str_len, 945);

    // Size including pointer array:
    assert_eq!(
        total_simpleascii_len + NUM_UNIQUE_BLOBS * core::mem::size_of::<usize>(),
        8386
    );
    assert_eq!(
        total_perfecthash_len + NUM_UNIQUE_BLOBS * core::mem::size_of::<usize>(),
        8590
    );
    assert_eq!(
        total_vzv_len + STRINGS.len() * core::mem::size_of::<usize>(),
        13902
    );
    // 2x for the lookup arrays and value arrays
    assert_eq!(
        total_unique_locale_str_len + 2 * STRINGS.len() * core::mem::size_of::<usize>(),
        17777
    );

    // Size including u16 pointer array:
    assert_eq!(
        total_unique_locale_str_len
            + STRINGS.len() * core::mem::size_of::<usize>()
            + STRINGS.len() * core::mem::size_of::<u16>()
            + NUM_UNIQUE_BLOBS * core::mem::size_of::<usize>(),
        14753
    );
}
