// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

// LiteMap is intended as a small and low-memory drop-in replacement for HashMap.
//
// This example does not use LiteMap. The reader may compare this HashMap example to the
// LiteMap example to showcase analogous operations between HashMap and LiteMap.

#![no_main] // https://github.com/unicode-org/icu4x/issues/395
icu_benchmark_macros::instrument!();

use icu_locale_core::subtags::{language, Language};
use std::collections::HashMap;

const DATA: [(Language, &str); 11] = [
    (language!("ar"), "Arabic"),
    (language!("bn"), "Bangla"),
    (language!("ccp"), "Chakma"),
    (language!("en"), "English"),
    (language!("es"), "Spanish"),
    (language!("fr"), "French"),
    (language!("ja"), "Japanese"),
    (language!("ru"), "Russian"),
    (language!("sr"), "Serbian"),
    (language!("th"), "Thai"),
    (language!("tr"), "Turkish"),
];

fn main() {
    let map = HashMap::<Language, &str>::from_iter(DATA);

    assert!(map.len() == 11);
    assert!(map.get(&language!("th")) == Some(&"Thai"));
    assert!(!map.contains_key(&language!("de")));
}
