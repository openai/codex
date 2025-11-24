// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use icu_locale_core::{langid, subtags::language, subtags::region, LanguageIdentifier};
use std::borrow::Cow;
use writeable::Writeable;

const LIDS: &[LanguageIdentifier] = &[
    langid!("en"),
    langid!("pl"),
    langid!("fr-CA"),
    langid!("zh-Hans"),
    langid!("en-US"),
    langid!("en-Latn-US"),
    langid!("sr-Cyrl-BA"),
];

const LIDS_STR: &[&str] = &[
    "en",
    "pl",
    "fr-CA",
    "zh-Hans",
    "en-US",
    "en-Latn-US",
    "sr-Cyrl-BA",
];

fn bench_langid_constr() {
    // Tests the instructions required to construct a LID from an str.

    let _: Vec<LanguageIdentifier> = LIDS_STR
        .iter()
        .map(|l| l.parse().expect("Failed to parse"))
        .collect();
}

fn bench_langid_compare_components() {
    // Tests the cost of comparing LID components.

    let result = LIDS
        .iter()
        .filter(|l| l.language == language!("en") && l.region == Some(region!("US")))
        .count();

    assert_eq!(result, 2);
}

fn bench_langid_compare_components_str() {
    // Tests the cost of comparing LID components to str.

    let result = LIDS
        .iter()
        .filter(|l| {
            l.language == language!("en") && l.region.map(|r| r == region!("US")).unwrap_or(false)
        })
        .count();

    assert_eq!(result, 2);
}

fn bench_langid_strict_cmp() {
    // Tests the cost of comparing a langid against byte strings.
    use core::cmp::Ordering;

    let lid = langid!("en-us");

    let result = LIDS_STR
        .iter()
        .filter(|s| lid.strict_cmp(s.as_bytes()) == Ordering::Equal)
        .count();

    assert_eq!(result, 1);
}

fn bench_langid_matching() {
    // Tests matching a LID against other LIDs.

    let lid = langid!("en-us");

    let count = LIDS.iter().filter(|l| lid == **l).count();
    assert_eq!(count, 1);
}

fn bench_langid_matching_str() {
    // Tests matching a LID against list of str.

    let lid = langid!("en-us");

    let count = LIDS_STR.iter().filter(|&l| lid.normalizing_eq(l)).count();
    assert_eq!(count, 1);
}

fn bench_langid_serialize() {
    // Tests serialization of LIDs.

    let _: Vec<String> = LIDS.iter().map(|l| l.to_string()).collect();
}

fn bench_langid_serialize_writeable() {
    // Tests serialization of LIDs.

    let _: Vec<_> = LIDS.iter().map(|l| l.write_to_string()).collect();
}

fn bench_langid_canonicalize() {
    // Tests canonicalization of strings.

    let _: Vec<Cow<str>> = LIDS_STR
        .iter()
        .map(|l| LanguageIdentifier::normalize(l).expect("Normalization failed"))
        .collect();
}

iai::main!(
    bench_langid_constr,
    bench_langid_compare_components,
    bench_langid_compare_components_str,
    bench_langid_strict_cmp,
    bench_langid_matching,
    bench_langid_matching_str,
    bench_langid_serialize,
    bench_langid_serialize_writeable,
    bench_langid_canonicalize,
);
