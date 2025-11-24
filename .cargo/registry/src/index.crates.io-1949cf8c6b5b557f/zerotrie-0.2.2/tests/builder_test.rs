// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use litemap::LiteMap;
use zerotrie::ZeroTriePerfectHash;
use zerotrie::ZeroTrieSimpleAscii;

mod testdata {
    include!("data/data.rs");
}

use testdata::strings_to_litemap;

const NON_EXISTENT_STRINGS: &[&str] = &[
    "a9PS", "ahsY", "ahBO", "a8IN", "xk8o", "xv1l", "xI2S", "618y", "d6My", "uszy",
];

macro_rules! assert_bytes_eq {
    ($len:literal, $a:expr, $b:expr) => {
        assert_eq!($len, $a.len());
        assert_eq!($a, $b);
    };
}

fn check_simple_ascii_trie<S>(items: &LiteMap<&[u8], usize>, trie: &ZeroTrieSimpleAscii<S>)
where
    S: AsRef<[u8]> + ?Sized,
{
    // Check that each item is in the trie
    for (k, v) in items.iter() {
        assert_eq!(trie.get(k), Some(*v));
    }
    // Check that some items are not in the trie
    for s in NON_EXISTENT_STRINGS.iter() {
        assert_eq!(trie.get(s.as_bytes()), None);
    }
    // Check that the iterator returns items in the same order as the LiteMap
    assert!(items
        .iter()
        .map(|(s, v)| (String::from_utf8(s.to_vec()).unwrap(), *v))
        .eq(trie.iter()));
    // Check that the const builder works
    let const_trie = ZeroTrieSimpleAscii::try_from_litemap_with_const_builder(items).unwrap();
    assert_eq!(trie.as_bytes(), const_trie.as_bytes());
}

fn check_phf_ascii_trie<S>(items: &LiteMap<&[u8], usize>, trie: &ZeroTriePerfectHash<S>)
where
    S: AsRef<[u8]> + ?Sized,
{
    // Check that each item is in the trie
    for (k, v) in items.iter() {
        assert_eq!(trie.get(k), Some(*v));
    }
    // Check that some items are not in the trie
    for s in NON_EXISTENT_STRINGS.iter() {
        assert_eq!(trie.get(s.as_bytes()), None);
    }
    // Check that the iterator returns the contents of the LiteMap
    // Note: Since the items might not be in order, we collect them into a new LiteMap
    let recovered_items: LiteMap<_, _> = trie.iter().collect();
    assert_eq!(
        items.to_borrowed_keys_values::<[u8], usize, Vec<_>>(),
        recovered_items.to_borrowed_keys_values()
    );
}

fn check_phf_bytes_trie<S>(items: &LiteMap<&[u8], usize>, trie: &ZeroTriePerfectHash<S>)
where
    S: AsRef<[u8]> + ?Sized,
{
    // Check that each item is in the trie
    for (k, v) in items.iter() {
        assert_eq!(trie.get(k), Some(*v), "{k:?}");
    }
    // Check that some items are not in the trie
    for s in NON_EXISTENT_STRINGS.iter() {
        assert_eq!(trie.get(s.as_bytes()), None, "{s:?}");
    }
    // Check that the iterator returns the contents of the LiteMap
    // Note: Since the items might not be in order, we collect them into a new LiteMap
    let recovered_items: LiteMap<_, _> = trie.iter().collect();
    assert_eq!(
        items.to_borrowed_keys_values::<[u8], usize, Vec<_>>(),
        recovered_items.to_borrowed_keys_values()
    );
}

#[test]
fn test_basic() {
    let lm1a: LiteMap<&[u8], usize> = testdata::basic::DATA_ASCII.iter().copied().collect();
    let lm1b: LiteMap<&[u8], usize> = lm1a.to_borrowed_keys();
    let lm2: LiteMap<&[u8], usize> = testdata::basic::DATA_UNICODE.iter().copied().collect();
    let lm3: LiteMap<&[u8], usize> = testdata::basic::DATA_BINARY.iter().copied().collect();

    let expected_bytes = testdata::basic::TRIE_ASCII;
    let trie = ZeroTrieSimpleAscii::try_from(&lm1a).unwrap();
    assert_bytes_eq!(26, trie.as_bytes(), expected_bytes);
    check_simple_ascii_trie(&lm1a, &trie);

    let trie = ZeroTriePerfectHash::try_from(&lm1b).unwrap();
    assert_bytes_eq!(26, trie.as_bytes(), expected_bytes);
    check_phf_ascii_trie(&lm1a, &trie);

    let expected_bytes = testdata::basic::TRIE_UNICODE;
    let trie = ZeroTriePerfectHash::try_from(&lm2).unwrap();
    assert_bytes_eq!(39, trie.as_bytes(), expected_bytes);
    check_phf_bytes_trie(&lm2, &trie);

    let expected_bytes = testdata::basic::TRIE_BINARY;
    let trie = ZeroTriePerfectHash::try_from(&lm3).unwrap();
    assert_bytes_eq!(26, trie.as_bytes(), expected_bytes);
    check_phf_bytes_trie(&lm3, &trie);
}

#[test]
fn test_empty() {
    let trie = ZeroTrieSimpleAscii::try_from(&LiteMap::<&[u8], usize>::new_vec()).unwrap();
    assert_eq!(trie.byte_len(), 0);
    assert!(trie.is_empty());
    assert_eq!(trie.get(b""), None);
    assert_eq!(trie.as_bytes(), &[]);
}

#[test]
fn test_single_empty_value() {
    let litemap: LiteMap<&[u8], usize> = [
        (&b""[..], 10), //
    ]
    .into_iter()
    .collect();
    let trie = ZeroTrieSimpleAscii::try_from(&litemap.as_sliced()).unwrap();
    assert_eq!(trie.get(b""), Some(10));
    assert_eq!(trie.get(b"x"), None);
    let expected_bytes = &[0b10001010];
    assert_eq!(trie.as_bytes(), expected_bytes);

    let litemap_bytes = litemap.to_borrowed_keys::<[u8], Vec<_>>();
    let trie_phf = ZeroTriePerfectHash::try_from(&litemap_bytes).unwrap();
    assert_bytes_eq!(1, trie_phf.as_bytes(), expected_bytes);
    check_phf_ascii_trie(&litemap, &trie_phf);
}

#[test]
fn test_single_byte_string() {
    let litemap: LiteMap<&[u8], usize> = [
        (&b"x"[..], 10), //
    ]
    .into_iter()
    .collect();
    let trie = ZeroTrieSimpleAscii::try_from(&litemap.as_sliced()).unwrap();
    assert_eq!(trie.get(b""), None);
    assert_eq!(trie.get(b"xy"), None);
    check_simple_ascii_trie(&litemap, &trie);
    let expected_bytes = &[b'x', 0b10001010];
    assert_bytes_eq!(2, trie.as_bytes(), expected_bytes);

    let litemap_bytes = litemap.to_borrowed_keys::<[u8], Vec<_>>();
    let trie_phf = ZeroTriePerfectHash::try_from(&litemap_bytes).unwrap();
    assert_bytes_eq!(2, trie_phf.as_bytes(), expected_bytes);
    check_phf_ascii_trie(&litemap, &trie_phf);
}

#[test]
fn test_single_string() {
    let litemap: LiteMap<&[u8], usize> = [
        (&b"xyz"[..], 10), //
    ]
    .into_iter()
    .collect();
    let trie = ZeroTrieSimpleAscii::try_from(&litemap.as_sliced()).unwrap();
    assert_eq!(trie.get(b""), None);
    assert_eq!(trie.get(b"x"), None);
    assert_eq!(trie.get(b"xy"), None);
    assert_eq!(trie.get(b"xyzz"), None);
    check_simple_ascii_trie(&litemap, &trie);
    let expected_bytes = &[b'x', b'y', b'z', 0b10001010];
    assert_bytes_eq!(4, trie.as_bytes(), expected_bytes);

    let litemap_bytes = litemap.to_borrowed_keys::<[u8], Vec<_>>();
    let trie_phf = ZeroTriePerfectHash::try_from(&litemap_bytes).unwrap();
    assert_bytes_eq!(4, trie_phf.as_bytes(), expected_bytes);
    check_phf_ascii_trie(&litemap, &trie_phf);
}

#[test]
fn test_prefix_strings() {
    let litemap: LiteMap<&[u8], usize> = [(&b"x"[..], 0), (b"xy", 1)].into_iter().collect();
    let trie = ZeroTrieSimpleAscii::try_from(&litemap.as_sliced()).unwrap();
    assert_eq!(trie.get(b""), None);
    assert_eq!(trie.get(b"xyz"), None);
    check_simple_ascii_trie(&litemap, &trie);
    let expected_bytes = &[b'x', 0b10000000, b'y', 0b10000001];
    assert_bytes_eq!(4, trie.as_bytes(), expected_bytes);

    let litemap_bytes = litemap.to_borrowed_keys::<[u8], Vec<_>>();
    let trie_phf = ZeroTriePerfectHash::try_from(&litemap_bytes).unwrap();
    assert_bytes_eq!(4, trie_phf.as_bytes(), expected_bytes);
    check_phf_ascii_trie(&litemap, &trie_phf);
}

#[test]
fn test_single_byte_branch() {
    let litemap: LiteMap<&[u8], usize> = [(&b"x"[..], 0), (b"y", 1)].into_iter().collect();
    let trie = ZeroTrieSimpleAscii::try_from(&litemap.as_sliced()).unwrap();
    assert_eq!(trie.get(b""), None);
    assert_eq!(trie.get(b"xy"), None);
    check_simple_ascii_trie(&litemap, &trie);
    let expected_bytes = &[0b11000010, b'x', b'y', 1, 0b10000000, 0b10000001];
    assert_bytes_eq!(6, trie.as_bytes(), expected_bytes);

    let litemap_bytes = litemap.to_borrowed_keys::<[u8], Vec<_>>();
    let trie_phf = ZeroTriePerfectHash::try_from(&litemap_bytes).unwrap();
    assert_bytes_eq!(6, trie_phf.as_bytes(), expected_bytes);
    check_phf_ascii_trie(&litemap, &trie_phf);
}

#[test]
fn test_multi_byte_branch() {
    let litemap: LiteMap<&[u8], usize> = [(&b"axb"[..], 0), (b"ayc", 1)].into_iter().collect();
    let trie = ZeroTrieSimpleAscii::try_from(&litemap.as_sliced()).unwrap();
    assert_eq!(trie.get(b""), None);
    assert_eq!(trie.get(b"a"), None);
    assert_eq!(trie.get(b"ax"), None);
    assert_eq!(trie.get(b"ay"), None);
    check_simple_ascii_trie(&litemap, &trie);
    let expected_bytes = &[
        b'a', 0b11000010, b'x', b'y', 2, b'b', 0b10000000, b'c', 0b10000001,
    ];
    assert_bytes_eq!(9, trie.as_bytes(), expected_bytes);

    let litemap_bytes = litemap.to_borrowed_keys::<[u8], Vec<_>>();
    let trie_phf = ZeroTriePerfectHash::try_from(&litemap_bytes).unwrap();
    assert_bytes_eq!(9, trie_phf.as_bytes(), expected_bytes);
    check_phf_ascii_trie(&litemap, &trie_phf);
}

#[test]
fn test_linear_varint_values() {
    let litemap: LiteMap<&[u8], usize> = [(&b""[..], 100), (b"x", 500), (b"xyz", 5000)]
        .into_iter()
        .collect();
    let trie = ZeroTrieSimpleAscii::try_from(&litemap.as_sliced()).unwrap();
    assert_eq!(trie.get(b"xy"), None);
    assert_eq!(trie.get(b"xz"), None);
    assert_eq!(trie.get(b"xyzz"), None);
    check_simple_ascii_trie(&litemap, &trie);
    let expected_bytes = &[0x90, 0x54, b'x', 0x93, 0x64, b'y', b'z', 0x90, 0x96, 0x78];
    assert_bytes_eq!(10, trie.as_bytes(), expected_bytes);

    let litemap_bytes = litemap.to_borrowed_keys::<[u8], Vec<_>>();
    let trie_phf = ZeroTriePerfectHash::try_from(&litemap_bytes).unwrap();
    assert_bytes_eq!(10, trie_phf.as_bytes(), expected_bytes);
    check_phf_ascii_trie(&litemap, &trie_phf);
}

#[test]
fn test_bug() {
    let litemap: LiteMap<&[u8], usize> = [(&b"abc"[..], 100), (b"abcd", 500), (b"abcde", 5000)]
        .into_iter()
        .collect();
    let trie = ZeroTrieSimpleAscii::try_from(&litemap.as_sliced()).unwrap();
    assert_eq!(trie.get(b"ab"), None);
    assert_eq!(trie.get(b"abd"), None);
    assert_eq!(trie.get(b"abCD"), None);
    check_simple_ascii_trie(&litemap, &trie);

    let litemap_bytes = litemap.to_borrowed_keys::<[u8], Vec<_>>();
    let trie_phf = ZeroTriePerfectHash::try_from(&litemap_bytes).unwrap();
    check_phf_ascii_trie(&litemap, &trie_phf);
}

#[test]
fn test_varint_branch() {
    let chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let litemap: LiteMap<&[u8], usize> = (0..chars.len())
        .map(|i| (chars.get(i..i + 1).unwrap().as_bytes(), i))
        .collect();
    let trie = ZeroTrieSimpleAscii::try_from(&litemap.as_sliced()).unwrap();
    assert_eq!(trie.get(b""), None);
    assert_eq!(trie.get(b"ax"), None);
    assert_eq!(trie.get(b"ay"), None);
    check_simple_ascii_trie(&litemap, &trie);
    #[rustfmt::skip]
    let expected_bytes = &[
        0b11100000, // branch varint lead
        0x14,       // branch varint trail
        // search array:
        b'A', b'B', b'C', b'D', b'E', b'F', b'G', b'H', b'I', b'J',
        b'K', b'L', b'M', b'N', b'O', b'P', b'Q', b'R', b'S', b'T',
        b'U', b'V', b'W', b'X', b'Y', b'Z',
        b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j',
        b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't',
        b'u', b'v', b'w', b'x', b'y', b'z',
        // offset array:
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 18, 20,
        22, 24, 26, 28, 30, 32, 34, 36, 38, 40, 42, 44, 46, 48, 50, 52,
        54, 56, 58, 60, 62, 64, 66, 68, 70, 72, 74, 76, 78, 80, 82, 84,
        86,
        // single-byte values:
        0x80, (0x80 | 1), (0x80 | 2), (0x80 | 3), (0x80 | 4),
        (0x80 | 5), (0x80 | 6), (0x80 | 7), (0x80 | 8), (0x80 | 9),
        (0x80 | 10), (0x80 | 11), (0x80 | 12), (0x80 | 13), (0x80 | 14),
        (0x80 | 15),
        // multi-byte values:
        0x90, 0, 0x90, 1, 0x90, 2, 0x90, 3, 0x90, 4, 0x90, 5,
        0x90, 6, 0x90, 7, 0x90, 8, 0x90, 9, 0x90, 10, 0x90, 11,
        0x90, 12, 0x90, 13, 0x90, 14, 0x90, 15, 0x90, 16, 0x90, 17,
        0x90, 18, 0x90, 19, 0x90, 20, 0x90, 21, 0x90, 22, 0x90, 23,
        0x90, 24, 0x90, 25, 0x90, 26, 0x90, 27, 0x90, 28, 0x90, 29,
        0x90, 30, 0x90, 31, 0x90, 32, 0x90, 33, 0x90, 34, 0x90, 35,
    ];
    assert_bytes_eq!(193, trie.as_bytes(), expected_bytes);

    #[rustfmt::skip]
    let expected_bytes = &[
        0b11100000, // branch varint lead
        0x14,       // branch varint trail
        // PHF metadata:
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 10, 12, 16, 4, 4, 4, 4, 4, 4, 8,
        4, 4, 4, 16, 16, 16, 16, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 0, 7,
        // search array:
        b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o',
        b'p', b'u', b'v', b'w', b'D', b'E', b'F', b'q',
        b'r', b'A', b'B', b'C', b'x', b'y', b'z', b's',
        b'H', b'I', b'J', b'G', b'P', b'Q', b'R', b'S',
        b'T', b'U', b'V', b'W', b'X', b'Y', b'Z', b'K',
        b'L', b'M', b'N', b'O', b'g', b'a', b'b', b'c',
        b't', b'd', b'f', b'e',
        // offset array:
        2, 4, 6, 8, 10, 12, 14,
        16, 18, 20, 22, 24, 25, 26, 27,
        29, 31, 32, 33, 34, 36, 38, 40,
        42, 43, 44, 45, 46, 47, 49, 51,
        53, 55, 57, 59, 61, 63, 65, 67,
        68, 69, 70, 71, 72, 74, 76, 78,
        80, 82, 84, 86,
        // values:
        0x90, 17, 0x90, 18, 0x90, 19, 0x90, 20, 0x90, 21, 0x90, 22, 0x90, 23,
        0x90, 24, 0x90, 25, 0x90, 30, 0x90, 31, 0x90, 32, 0x80 | 3, 0x80 | 4,
        0x80 | 5, 0x90, 26, 0x90, 27, 0x80, 0x80 | 1, 0x80 | 2, 0x90, 33,
        0x90, 34, 0x90, 35, 0x90, 28, 0x80 | 7, 0x80 | 8, 0x80 | 9, 0x80 | 6,
        0x80 | 15, 0x90, 0, 0x90, 1, 0x90, 2, 0x90, 3, 0x90, 4, 0x90, 5,
        0x90, 6, 0x90, 7, 0x90, 8, 0x90, 9, 0x80 | 10, 0x80 | 11, 0x80 | 12,
        0x80 | 13, 0x80 | 14, 0x90, 16, 0x90, 10, 0x90, 11, 0x90, 12, 0x90, 29,
        0x90, 13, 0x90, 15, 0x90, 14,
    ];
    let litemap_bytes = litemap.to_borrowed_keys::<[u8], Vec<_>>();
    let trie_phf = ZeroTriePerfectHash::try_from(&litemap_bytes).unwrap();
    assert_bytes_eq!(246, trie_phf.as_bytes(), expected_bytes);
    check_phf_ascii_trie(&litemap, &trie_phf);
}

#[test]
fn test_below_wide() {
    let litemap: LiteMap<&[u8], usize> = [
        (&b"abcdefghijklmnopqrstuvwxyz"[..], 1),
        (b"bcdefghijklmnopqrstuvwxyza", 2),
        (b"cdefghijklmnopqrstuvwxyzab", 3),
        (b"defghijklmnopqrstuvwxyzabc", 4),
        (b"efghijklmnopqrstuvwxyzabcd", 5),
        (b"fghijklmnopqrstuvwxyzabcde", 6),
        (b"ghijklmnopqrstuvwxyzabcdef", 7),
        (b"hijklmnopqrstuvwxyzabcdefg", 8),
        (b"ijklmnopqrstuvwxyzabcdefgh", 9),
        (b"jklmnopqrstuvwxyzabcd", 10),
    ]
    .into_iter()
    .collect();
    let trie = ZeroTrieSimpleAscii::try_from(&litemap.as_sliced()).unwrap();
    assert_eq!(trie.get(b""), None);
    assert_eq!(trie.get(b"abc"), None);
    check_simple_ascii_trie(&litemap, &trie);
    #[rustfmt::skip]
    let expected_bytes = &[
        0b11001010, // branch
        // search array:
        b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j',
        // offset array:
        26, 52, 78, 104, 130, 156, 182, 208, 234,
        // offset data:
        b'b', b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n',
        b'o', b'p', b'q', b'r', b's', b't', b'u', b'v', b'w', b'x', b'y', b'z',
        0x81,
        b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o',
        b'p', b'q', b'r', b's', b't', b'u', b'v', b'w', b'x', b'y', b'z', b'a',
        0x82,
        b'd', b'e', b'f', b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p',
        b'q', b'r', b's', b't', b'u', b'v', b'w', b'x', b'y', b'z', b'a', b'b',
        0x83,
        b'e', b'f', b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q',
        b'r', b's', b't', b'u', b'v', b'w', b'x', b'y', b'z', b'a', b'b', b'c',
        0x84,
        b'f', b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r',
        b's', b't', b'u', b'v', b'w', b'x', b'y', b'z', b'a', b'b', b'c', b'd',
        0x85,
        b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's',
        b't', b'u', b'v', b'w', b'x', b'y', b'z', b'a', b'b', b'c', b'd', b'e',
        0x86,
        b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't',
        b'u', b'v', b'w', b'x', b'y', b'z', b'a', b'b', b'c', b'd', b'e', b'f',
        0x87,
        b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't', b'u',
        b'v', b'w', b'x', b'y', b'z', b'a', b'b', b'c', b'd', b'e', b'f', b'g',
        0x88,
        b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't', b'u', b'v',
        b'w', b'x', b'y', b'z', b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h',
        0x89,
        b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't', b'u', b'v', b'w',
        b'x', b'y', b'z', b'a', b'b', b'c', b'd',
        0x8A,
    ];
    assert_bytes_eq!(275, trie.as_bytes(), expected_bytes);
}

#[test]
fn test_at_wide() {
    let litemap: LiteMap<&[u8], usize> = [
        (&b"abcdefghijklmnopqrstuvwxyz"[..], 1),
        (b"bcdefghijklmnopqrstuvwxyza", 2),
        (b"cdefghijklmnopqrstuvwxyzab", 3),
        (b"defghijklmnopqrstuvwxyzabc", 4),
        (b"efghijklmnopqrstuvwxyzabcd", 5),
        (b"fghijklmnopqrstuvwxyzabcde", 6),
        (b"ghijklmnopqrstuvwxyzabcdef", 7),
        (b"hijklmnopqrstuvwxyzabcdefg", 8),
        (b"ijklmnopqrstuvwxyzabcdefgh", 9),
        (b"jklmnopqrstuvwxyzabcde", 10),
    ]
    .into_iter()
    .collect();
    let trie = ZeroTrieSimpleAscii::try_from(&litemap.as_sliced()).unwrap();
    assert_eq!(trie.get(b""), None);
    assert_eq!(trie.get(b"abc"), None);
    check_simple_ascii_trie(&litemap, &trie);
    #[rustfmt::skip]
    let expected_bytes = &[
        0b11100001, // branch lead
        0x6A, // branch trail
        // search array:
        b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j',
        // offset array (wide):
        0, 0, 0, 0, 0, 0, 0, 0, 0,
        26, 52, 78, 104, 130, 156, 182, 208, 234,
        // offset data:
        b'b', b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n',
        b'o', b'p', b'q', b'r', b's', b't', b'u', b'v', b'w', b'x', b'y', b'z',
        0x81,
        b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o',
        b'p', b'q', b'r', b's', b't', b'u', b'v', b'w', b'x', b'y', b'z', b'a',
        0x82,
        b'd', b'e', b'f', b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p',
        b'q', b'r', b's', b't', b'u', b'v', b'w', b'x', b'y', b'z', b'a', b'b',
        0x83,
        b'e', b'f', b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q',
        b'r', b's', b't', b'u', b'v', b'w', b'x', b'y', b'z', b'a', b'b', b'c',
        0x84,
        b'f', b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r',
        b's', b't', b'u', b'v', b'w', b'x', b'y', b'z', b'a', b'b', b'c', b'd',
        0x85,
        b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's',
        b't', b'u', b'v', b'w', b'x', b'y', b'z', b'a', b'b', b'c', b'd', b'e',
        0x86,
        b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't',
        b'u', b'v', b'w', b'x', b'y', b'z', b'a', b'b', b'c', b'd', b'e', b'f',
        0x87,
        b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't', b'u',
        b'v', b'w', b'x', b'y', b'z', b'a', b'b', b'c', b'd', b'e', b'f', b'g',
        0x88,
        b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't', b'u', b'v',
        b'w', b'x', b'y', b'z', b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h',
        0x89,
        b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't', b'u', b'v', b'w',
        b'x', b'y', b'z', b'a', b'b', b'c', b'd', b'e',
        0x8A,
    ];
    assert_bytes_eq!(286, trie.as_bytes(), expected_bytes);
}

#[test]
fn test_at_wide_plus() {
    let litemap: LiteMap<&[u8], usize> = [
        (&b"abcdefghijklmnopqrstuvwxyz"[..], 1),
        (b"bcdefghijklmnopqrstuvwxyza", 2),
        (b"cdefghijklmnopqrstuvwxyzab", 3),
        (b"defghijklmnopqrstuvwxyzabc", 4),
        (b"efghijklmnopqrstuvwxyzabcd", 5),
        (b"fghijklmnopqrstuvwxyzabcde", 6),
        (b"ghijklmnopqrstuvwxyzabcdef", 7),
        (b"hijklmnopqrstuvwxyzabcdefg", 8),
        (b"ijklmnopqrstuvwxyzabcdefgh", 9),
        (b"jklmnopqrstuvwxyzabcdef", 10),
    ]
    .into_iter()
    .collect();
    let trie = ZeroTrieSimpleAscii::try_from(&litemap.as_sliced()).unwrap();
    assert_eq!(trie.get(b""), None);
    assert_eq!(trie.get(b"abc"), None);
    check_simple_ascii_trie(&litemap, &trie);
    #[rustfmt::skip]
    let expected_bytes = &[
        0b11100001, // branch lead
        0x6A, // branch trail
        // search array:
        b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j',
        // offset array (wide):
        0, 0, 0, 0, 0, 0, 0, 0, 0,
        26, 52, 78, 104, 130, 156, 182, 208, 234,
        // offset data:
        b'b', b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n',
        b'o', b'p', b'q', b'r', b's', b't', b'u', b'v', b'w', b'x', b'y', b'z',
        0x81,
        b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o',
        b'p', b'q', b'r', b's', b't', b'u', b'v', b'w', b'x', b'y', b'z', b'a',
        0x82,
        b'd', b'e', b'f', b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p',
        b'q', b'r', b's', b't', b'u', b'v', b'w', b'x', b'y', b'z', b'a', b'b',
        0x83,
        b'e', b'f', b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q',
        b'r', b's', b't', b'u', b'v', b'w', b'x', b'y', b'z', b'a', b'b', b'c',
        0x84,
        b'f', b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r',
        b's', b't', b'u', b'v', b'w', b'x', b'y', b'z', b'a', b'b', b'c', b'd',
        0x85,
        b'g', b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's',
        b't', b'u', b'v', b'w', b'x', b'y', b'z', b'a', b'b', b'c', b'd', b'e',
        0x86,
        b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't',
        b'u', b'v', b'w', b'x', b'y', b'z', b'a', b'b', b'c', b'd', b'e', b'f',
        0x87,
        b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't', b'u',
        b'v', b'w', b'x', b'y', b'z', b'a', b'b', b'c', b'd', b'e', b'f', b'g',
        0x88,
        b'j', b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't', b'u', b'v',
        b'w', b'x', b'y', b'z', b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h',
        0x89,
        b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't', b'u', b'v', b'w',
        b'x', b'y', b'z', b'a', b'b', b'c', b'd', b'e', b'f',
        0x8A,
    ];
    assert_bytes_eq!(287, trie.as_bytes(), expected_bytes);
}

#[test]
fn test_everything() {
    let litemap: LiteMap<&[u8], usize> = [
        (&b""[..], 0),
        (b"axb", 100),
        (b"ayc", 2),
        (b"azd", 3),
        (b"bxe", 4),
        (b"bxefg", 500),
        (b"bxefh", 6),
        (b"bxei", 7),
        (b"bxeikl", 8),
    ]
    .into_iter()
    .collect();
    let trie = ZeroTrieSimpleAscii::try_from(&litemap.as_sliced()).unwrap();
    assert_eq!(trie.get(b""), Some(0));
    assert_eq!(trie.get(b"a"), None);
    assert_eq!(trie.get(b"ax"), None);
    assert_eq!(trie.get(b"ay"), None);
    check_simple_ascii_trie(&litemap, &trie);
    let expected_bytes = &[
        0b10000000, // value 0
        0b11000010, // branch of 2
        b'a',       //
        b'b',       //
        13,         //
        0b11000011, // branch of 3
        b'x',       //
        b'y',       //
        b'z',       //
        3,          //
        5,          //
        b'b',       //
        0b10010000, // value 100 (lead)
        0x54,       // value 100 (trail)
        b'c',       //
        0b10000010, // value 2
        b'd',       //
        0b10000011, // value 3
        b'x',       //
        b'e',       //
        0b10000100, // value 4
        0b11000010, // branch of 2
        b'f',       //
        b'i',       //
        7,          //
        0b11000010, // branch of 2
        b'g',       //
        b'h',       //
        2,          //
        0b10010011, // value 500 (lead)
        0x64,       // value 500 (trail)
        0b10000110, // value 6
        0b10000111, // value 7
        b'k',       //
        b'l',       //
        0b10001000, // value 8
    ];
    assert_bytes_eq!(36, trie.as_bytes(), expected_bytes);

    #[rustfmt::skip]
    let expected_bytes = &[
        0b10000000, // value 0
        0b11000010, // branch of 2
        b'a',       //
        b'b',       //
        13,         //
        0b11000011, // start of 'a' subtree: branch of 3
        b'x',       //
        b'y',       //
        b'z',       //
        3,          //
        5,          //
        b'b',       //
        0b10010000, // value 100 (lead)
        0x54,       // value 100 (trail)
        b'c',       //
        0b10000010, // value 2
        b'd',       //
        0b10000011, // value 3
        b'x',       // start of 'b' subtree
        b'e',       //
        0b10000100, // value 4
        0b11000010, // branch of 2
        b'f',       //
        b'i',       //
        7,          //
        0b11000010, // branch of 2
        b'g',       //
        b'h',       //
        2,          //
        0b10010011, // value 500 (lead)
        0x64,       // value 500 (trail)
        0b10000110, // value 6
        0b10000111, // value 7
        b'k',       //
        b'l',       //
        0b10001000, // value 8
    ];
    let litemap_bytes = litemap.to_borrowed_keys::<[u8], Vec<_>>();
    let trie_phf = ZeroTriePerfectHash::try_from(&litemap_bytes).unwrap();
    assert_bytes_eq!(36, trie_phf.as_bytes(), expected_bytes);
    check_phf_ascii_trie(&litemap, &trie_phf);

    let zhm: zerovec::ZeroMap<[u8], u32> = litemap.iter().map(|(a, b)| (*a, *b as u32)).collect();
    let zhm_buf = postcard::to_allocvec(&zhm).unwrap();
    assert_eq!(zhm_buf.len(), 88);

    let zhm: zerovec::ZeroMap<[u8], u8> = litemap.iter().map(|(a, b)| (*a, *b as u8)).collect();
    let zhm_buf = postcard::to_allocvec(&zhm).unwrap();
    assert_eq!(zhm_buf.len(), 61);

    let zhm: zerovec::ZeroHashMap<[u8], u32> =
        litemap.iter().map(|(a, b)| (*a, *b as u32)).collect();
    let zhm_buf = postcard::to_allocvec(&zhm).unwrap();
    assert_eq!(zhm_buf.len(), 161);

    let zhm: zerovec::ZeroHashMap<[u8], u8> = litemap.iter().map(|(a, b)| (*a, *b as u8)).collect();
    let zhm_buf = postcard::to_allocvec(&zhm).unwrap();
    assert_eq!(zhm_buf.len(), 134);
}

macro_rules! utf8_byte {
    ($ch:expr, $i:literal) => {{
        let mut utf8_encoder_buf = [0u8; 4];
        $ch.encode_utf8(&mut utf8_encoder_buf);
        utf8_encoder_buf[$i]
    }};
}

#[test]
fn test_non_ascii() {
    let litemap: LiteMap<&[u8], usize> = [
        ("".as_bytes(), 0),
        ("axb".as_bytes(), 100),
        ("ayc".as_bytes(), 2),
        ("azd".as_bytes(), 3),
        ("bxe".as_bytes(), 4),
        ("bxefg".as_bytes(), 500),
        ("bxefh".as_bytes(), 6),
        ("bxei".as_bytes(), 7),
        ("bxeikl".as_bytes(), 8),
        ("bxeiklmΚαλημέρααα".as_bytes(), 9),
        ("bxeiklmαnλo".as_bytes(), 10),
        ("bxeiklmη".as_bytes(), 11),
    ]
    .into_iter()
    .collect();

    #[rustfmt::skip]
    let expected_bytes = &[
        0b10000000, // value 0
        0b11000010, // branch of 2
        b'a',       //
        b'b',       //
        13,         //
        0b11000011, // start of 'a' subtree: branch of 3
        b'x',       //
        b'y',       //
        b'z',       //
        3,          //
        5,          //
        b'b',       //
        0b10010000, // value 100 (lead)
        0x54,       // value 100 (trail)
        b'c',       //
        0b10000010, // value 2
        b'd',       //
        0b10000011, // value 3
        b'x',       // start of 'b' subtree
        b'e',       //
        0b10000100, // value 4
        0b11000010, // branch of 2
        b'f',       //
        b'i',       //
        7,          //
        0b11000010, // branch of 2
        b'g',       //
        b'h',       //
        2,          //
        0b10010011, // value 500 (lead)
        0x64,       // value 500 (trail)
        0b10000110, // value 6
        0b10000111, // value 7
        b'k',       //
        b'l',       //
        0b10001000, // value 8
        b'm',       //
        0b10100001, // span of length 1
        utf8_byte!('Κ', 0), // NOTE: all three letters have the same lead byte
        0b11000011, // branch of 3
        utf8_byte!('Κ', 1),
        utf8_byte!('α', 1),
        utf8_byte!('η', 1),
        21,
        27,
        0b10110000, // span of length 18 (lead)
        0b00000010, // span of length 18 (trail)
        utf8_byte!('α', 0),
        utf8_byte!('α', 1),
        utf8_byte!('λ', 0),
        utf8_byte!('λ', 1),
        utf8_byte!('η', 0),
        utf8_byte!('η', 1),
        utf8_byte!('μ', 0),
        utf8_byte!('μ', 1),
        utf8_byte!('έ', 0),
        utf8_byte!('έ', 1),
        utf8_byte!('ρ', 0),
        utf8_byte!('ρ', 1),
        utf8_byte!('α', 0),
        utf8_byte!('α', 1),
        utf8_byte!('α', 0),
        utf8_byte!('α', 1),
        utf8_byte!('α', 0),
        utf8_byte!('α', 1),
        0b10001001, // value 9
        b'n',
        0b10100010, // span of length 2
        utf8_byte!('λ', 0),
        utf8_byte!('λ', 1),
        b'o',
        0b10001010, // value 10
        0b10001011, // value 11
    ];
    let trie_phf = ZeroTriePerfectHash::try_from(&litemap).unwrap();
    assert_bytes_eq!(73, trie_phf.as_bytes(), expected_bytes);
    check_phf_bytes_trie(&litemap, &trie_phf);
}

#[test]
fn test_max_branch() {
    // Evaluate a branch with all 256 possible children
    let mut litemap: LiteMap<&[u8], usize> = LiteMap::new_vec();
    let all_bytes: Vec<u8> = (u8::MIN..=u8::MAX).collect();
    assert_eq!(all_bytes.len(), 256);
    let all_bytes_prefixed: Vec<[u8; 2]> = (u8::MIN..=u8::MAX).map(|x| [b'\0', x]).collect();
    for b in all_bytes.iter() {
        litemap.insert(core::slice::from_ref(b), *b as usize);
    }
    for s in all_bytes_prefixed.iter() {
        litemap.insert(s, s[1] as usize);
    }
    let trie_phf = ZeroTriePerfectHash::try_from(&litemap).unwrap();
    assert_eq!(trie_phf.byte_len(), 3042);
    check_phf_bytes_trie(&litemap, &trie_phf);
}

#[test]
fn test_short_subtags_10pct() {
    let litemap = strings_to_litemap(testdata::short_subtags_10pct::STRINGS);

    let trie = ZeroTrieSimpleAscii::try_from(&litemap).unwrap();
    assert_eq!(trie.byte_len(), 1050);
    check_simple_ascii_trie(&litemap, &trie);

    let litemap_bytes = litemap.to_borrowed_keys::<[u8], Vec<_>>();
    let trie_phf = ZeroTriePerfectHash::try_from(&litemap_bytes).unwrap();
    assert_eq!(trie_phf.byte_len(), 1100);
    check_phf_ascii_trie(&litemap, &trie_phf);

    let zhm: zerovec::ZeroMap<[u8], u32> = litemap.iter().map(|(a, b)| (*a, *b as u32)).collect();
    let zhm_buf = postcard::to_allocvec(&zhm).unwrap();
    assert_eq!(zhm_buf.len(), 1890);

    let zhm: zerovec::ZeroMap<[u8], u8> = litemap.iter().map(|(a, b)| (*a, *b as u8)).collect();
    let zhm_buf = postcard::to_allocvec(&zhm).unwrap();
    assert_eq!(zhm_buf.len(), 1326);

    let zhm: zerovec::ZeroHashMap<[u8], u32> =
        litemap.iter().map(|(a, b)| (*a, *b as u32)).collect();
    let zhm_buf = postcard::to_allocvec(&zhm).unwrap();
    assert_eq!(zhm_buf.len(), 3396);

    let zhm: zerovec::ZeroHashMap<[u8], u8> = litemap.iter().map(|(a, b)| (*a, *b as u8)).collect();
    let zhm_buf = postcard::to_allocvec(&zhm).unwrap();
    assert_eq!(zhm_buf.len(), 2832);
}

#[test]
fn test_short_subtags() {
    let litemap = strings_to_litemap(testdata::short_subtags::STRINGS);

    let trie = ZeroTrieSimpleAscii::try_from(&litemap).unwrap();
    assert_eq!(trie.byte_len(), 8793);
    check_simple_ascii_trie(&litemap, &trie);

    let litemap_bytes = litemap.to_borrowed_keys::<[u8], Vec<_>>();
    let trie_phf = ZeroTriePerfectHash::try_from(&litemap_bytes).unwrap();
    assert_eq!(trie_phf.byte_len(), 9400);
    check_phf_ascii_trie(&litemap, &trie_phf);

    let zm: zerovec::ZeroMap<[u8], u32> = litemap.iter().map(|(a, b)| (*a, *b as u32)).collect();
    let zhm_buf = postcard::to_allocvec(&zm).unwrap();
    assert_eq!(zhm_buf.len(), 18931);

    let zm: zerovec::ZeroMap<[u8], u8> = litemap.iter().map(|(a, b)| (*a, *b as u8)).collect();
    let zhm_buf = postcard::to_allocvec(&zm).unwrap();
    assert_eq!(zhm_buf.len(), 13300);

    let zhm: zerovec::ZeroHashMap<[u8], u32> =
        litemap.iter().map(|(a, b)| (*a, *b as u32)).collect();
    let zhm_buf = postcard::to_allocvec(&zhm).unwrap();
    assert_eq!(zhm_buf.len(), 33949);

    let zhm: zerovec::ZeroHashMap<[u8], u8> = litemap.iter().map(|(a, b)| (*a, *b as u8)).collect();
    let zhm_buf = postcard::to_allocvec(&zhm).unwrap();
    assert_eq!(zhm_buf.len(), 28318);
}
