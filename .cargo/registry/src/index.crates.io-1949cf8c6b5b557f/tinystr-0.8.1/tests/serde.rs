// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use tinystr::*;

// Tests largely adapted from `tinystr` crate
// https://github.com/zbraniecki/tinystr/blob/4e4eab55dd6bded7f29a18b41452c506c461716c/tests/serde.rs

macro_rules! test_roundtrip {
    ($f:ident, $n:literal, $val:expr) => {
        #[test]
        fn $f() {
            let tiny: TinyAsciiStr<$n> = $val.parse().unwrap();
            let json_string = serde_json::to_string(&tiny).unwrap();
            let expected_json = concat!("\"", $val, "\"");
            assert_eq!(json_string, expected_json);
            let recover: TinyAsciiStr<$n> = serde_json::from_str(&json_string).unwrap();
            assert_eq!(&*tiny, &*recover);

            let bin = bincode::serialize(&tiny).unwrap();
            assert_eq!(bin, &tiny.all_bytes()[..]);
            let debin: TinyAsciiStr<$n> = bincode::deserialize(&bin).unwrap();
            assert_eq!(&*tiny, &*debin);

            let post = postcard::to_stdvec(&tiny).unwrap();
            assert_eq!(post, &tiny.all_bytes()[..]);
            let unpost: TinyAsciiStr<$n> = postcard::from_bytes(&post).unwrap();
            assert_eq!(&*tiny, &*unpost);
        }
    };
}

test_roundtrip!(test_roundtrip4_1, 4, "en");
test_roundtrip!(test_roundtrip4_2, 4, "Latn");
test_roundtrip!(test_roundtrip8, 8, "calendar");
test_roundtrip!(test_roundtrip16, 16, "verylongstring");
test_roundtrip!(test_roundtrip10, 11, "shortstring");
test_roundtrip!(test_roundtrip30, 24, "veryveryverylongstring");
