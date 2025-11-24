// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

// LiteMap is intended as a small and low-memory drop-in replacement for
// HashMap. This example demonstrates how it works with Serde.

#![no_main] // https://github.com/unicode-org/icu4x/issues/395
icu_benchmark_macros::instrument!();

use litemap::LiteMap;

const DATA: [(&str, &str); 11] = [
    ("ar", "Arabic"),
    ("bn", "Bangla"),
    ("ccp", "Chakma"),
    ("en", "English"),
    ("es", "Spanish"),
    ("fr", "French"),
    ("ja", "Japanese"),
    ("ru", "Russian"),
    ("sr", "Serbian"),
    ("th", "Thai"),
    ("tr", "Turkish"),
];

#[allow(dead_code)]
const BINCODE: [u8; 278] = [
    11, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 97, 114, 6, 0, 0, 0, 0, 0, 0, 0, 65, 114, 97,
    98, 105, 99, 2, 0, 0, 0, 0, 0, 0, 0, 98, 110, 6, 0, 0, 0, 0, 0, 0, 0, 66, 97, 110, 103, 108,
    97, 3, 0, 0, 0, 0, 0, 0, 0, 99, 99, 112, 6, 0, 0, 0, 0, 0, 0, 0, 67, 104, 97, 107, 109, 97, 2,
    0, 0, 0, 0, 0, 0, 0, 101, 110, 7, 0, 0, 0, 0, 0, 0, 0, 69, 110, 103, 108, 105, 115, 104, 2, 0,
    0, 0, 0, 0, 0, 0, 101, 115, 7, 0, 0, 0, 0, 0, 0, 0, 83, 112, 97, 110, 105, 115, 104, 2, 0, 0,
    0, 0, 0, 0, 0, 102, 114, 6, 0, 0, 0, 0, 0, 0, 0, 70, 114, 101, 110, 99, 104, 2, 0, 0, 0, 0, 0,
    0, 0, 106, 97, 8, 0, 0, 0, 0, 0, 0, 0, 74, 97, 112, 97, 110, 101, 115, 101, 2, 0, 0, 0, 0, 0,
    0, 0, 114, 117, 7, 0, 0, 0, 0, 0, 0, 0, 82, 117, 115, 115, 105, 97, 110, 2, 0, 0, 0, 0, 0, 0,
    0, 115, 114, 7, 0, 0, 0, 0, 0, 0, 0, 83, 101, 114, 98, 105, 97, 110, 2, 0, 0, 0, 0, 0, 0, 0,
    116, 104, 4, 0, 0, 0, 0, 0, 0, 0, 84, 104, 97, 105, 2, 0, 0, 0, 0, 0, 0, 0, 116, 114, 7, 0, 0,
    0, 0, 0, 0, 0, 84, 117, 114, 107, 105, 115, 104,
];

/// Run this function to print new data to the console.
#[allow(dead_code)]
fn generate() {
    let mut map = LiteMap::new_vec();
    for (lang, name) in DATA.iter() {
        map.try_append(lang, name).ok_or(()).unwrap_err();
    }

    let buf = bincode::serialize(&map).unwrap();
    println!("{buf:?}");
}

fn main() {
    // Uncomment the following line to re-generate the binary data.
    // generate();

    let map: LiteMap<&str, &str> = bincode::deserialize(&BINCODE).unwrap();
    assert_eq!(map.get("tr"), Some(&"Turkish"));
}
