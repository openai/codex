//! Test Cases

extern crate alloc;

mod utils;

use crate::utils::{check_deserialization, check_error_deserialization, is_equal};
use alloc::collections::{BTreeMap, BTreeSet};
use core::{cmp, iter::FromIterator as _};
use expect_test::expect;
use fnv::{FnvHashMap as HashMap, FnvHashSet as HashSet};
use pretty_assertions::assert_eq;
use serde::{Deserialize, Serialize};

#[test]
fn prohibit_duplicate_value_hashset() {
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S(#[serde(with = "::serde_with::rust::sets_duplicate_value_is_error")] HashSet<usize>);

    is_equal(
        S(HashSet::from_iter(vec![1, 2, 3, 4])),
        expect![[r#"
            [
              4,
              1,
              3,
              2
            ]"#]],
    );
    check_error_deserialization::<S>(
        r#"[1, 2, 3, 4, 1]"#,
        expect![[r#"invalid entry: found duplicate value at line 1 column 15"#]],
    );
}

#[test]
fn prohibit_duplicate_value_btreeset() {
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S(#[serde(with = "::serde_with::rust::sets_duplicate_value_is_error")] BTreeSet<usize>);

    is_equal(
        S(BTreeSet::from_iter(vec![1, 2, 3, 4])),
        expect![[r#"
            [
              1,
              2,
              3,
              4
            ]"#]],
    );
    check_error_deserialization::<S>(
        r#"[1, 2, 3, 4, 1]"#,
        expect![[r#"invalid entry: found duplicate value at line 1 column 15"#]],
    );
}

#[test]
fn prohibit_duplicate_key_hashmap() {
    #[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
    struct S(
        #[serde(with = "::serde_with::rust::maps_duplicate_key_is_error")] HashMap<usize, usize>,
    );

    // Different value and key always works
    is_equal(
        S(HashMap::from_iter(vec![(1, 1), (2, 2), (3, 3)])),
        expect![[r#"
            {
              "1": 1,
              "3": 3,
              "2": 2
            }"#]],
    );

    // Same value for different keys is ok
    is_equal(
        S(HashMap::from_iter(vec![(1, 1), (2, 1), (3, 1)])),
        expect![[r#"
            {
              "1": 1,
              "3": 1,
              "2": 1
            }"#]],
    );

    // Duplicate keys are an error
    check_error_deserialization::<S>(
        r#"{"1": 1, "2": 2, "1": 3}"#,
        expect![[r#"invalid entry: found duplicate key at line 1 column 24"#]],
    );
}

#[test]
fn prohibit_duplicate_key_btreemap() {
    #[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
    struct S(
        #[serde(with = "::serde_with::rust::maps_duplicate_key_is_error")] BTreeMap<usize, usize>,
    );

    // Different value and key always works
    is_equal(
        S(BTreeMap::from_iter(vec![(1, 1), (2, 2), (3, 3)])),
        expect![[r#"
            {
              "1": 1,
              "2": 2,
              "3": 3
            }"#]],
    );

    // Same value for different keys is ok
    is_equal(
        S(BTreeMap::from_iter(vec![(1, 1), (2, 1), (3, 1)])),
        expect![[r#"
            {
              "1": 1,
              "2": 1,
              "3": 1
            }"#]],
    );

    // Duplicate keys are an error
    check_error_deserialization::<S>(
        r#"{"1": 1, "2": 2, "1": 3}"#,
        expect![[r#"invalid entry: found duplicate key at line 1 column 24"#]],
    );
}

#[test]
fn duplicate_key_first_wins_hashmap() {
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S(#[serde(with = "::serde_with::rust::maps_first_key_wins")] HashMap<usize, usize>);

    // Different value and key always works
    is_equal(
        S(HashMap::from_iter(vec![(1, 1), (2, 2), (3, 3)])),
        expect![[r#"
            {
              "1": 1,
              "3": 3,
              "2": 2
            }"#]],
    );

    // Same value for different keys is ok
    is_equal(
        S(HashMap::from_iter(vec![(1, 1), (2, 1), (3, 1)])),
        expect![[r#"
            {
              "1": 1,
              "3": 1,
              "2": 1
            }"#]],
    );

    // Duplicate keys, the first one is used
    check_deserialization(
        S(HashMap::from_iter(vec![(1, 1), (2, 2)])),
        r#"{"1": 1, "2": 2, "1": 3}"#,
    );
}

#[test]
fn duplicate_key_first_wins_btreemap() {
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S(#[serde(with = "::serde_with::rust::maps_first_key_wins")] BTreeMap<usize, usize>);

    // Different value and key always works
    is_equal(
        S(BTreeMap::from_iter(vec![(1, 1), (2, 2), (3, 3)])),
        expect![[r#"
            {
              "1": 1,
              "2": 2,
              "3": 3
            }"#]],
    );

    // Same value for different keys is ok
    is_equal(
        S(BTreeMap::from_iter(vec![(1, 1), (2, 1), (3, 1)])),
        expect![[r#"
            {
              "1": 1,
              "2": 1,
              "3": 1
            }"#]],
    );

    // Duplicate keys, the first one is used
    check_deserialization(
        S(BTreeMap::from_iter(vec![(1, 1), (2, 2)])),
        r#"{"1": 1, "2": 2, "1": 3}"#,
    );
}

#[test]
fn duplicate_value_first_wins_hashset() {
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S(HashSet<W>);
    // struct S(#[serde(with = "::serde_with::rust::sets_first_value_wins")] HashSet<W>);

    #[derive(Debug, Eq, Deserialize, Serialize)]
    struct W(i32, bool);
    impl PartialEq for W {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl std::hash::Hash for W {
        fn hash<H>(&self, state: &mut H)
        where
            H: std::hash::Hasher,
        {
            self.0.hash(state);
        }
    }

    // Different values always work
    is_equal(
        S(HashSet::from_iter(vec![
            W(1, true),
            W(2, false),
            W(3, true),
        ])),
        expect![[r#"
            [
              [
                1,
                true
              ],
              [
                3,
                true
              ],
              [
                2,
                false
              ]
            ]"#]],
    );

    let value: S = serde_json::from_str(
        r#"[
        [1, false],
        [1, true],
        [2, true],
        [2, false]
    ]"#,
    )
    .unwrap();
    let entries: Vec<_> = value.0.into_iter().collect();
    assert_eq!(1, entries[0].0);
    assert!(!entries[0].1);
    assert_eq!(2, entries[1].0);
    assert!(entries[1].1);
}

#[test]
fn duplicate_value_last_wins_hashset() {
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S(#[serde(with = "::serde_with::rust::sets_last_value_wins")] HashSet<W>);

    #[derive(Debug, Eq, Deserialize, Serialize)]
    struct W(i32, bool);
    impl PartialEq for W {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl std::hash::Hash for W {
        fn hash<H>(&self, state: &mut H)
        where
            H: std::hash::Hasher,
        {
            self.0.hash(state);
        }
    }

    // Different values always work
    is_equal(
        S(HashSet::from_iter(vec![
            W(1, true),
            W(2, false),
            W(3, true),
        ])),
        expect![[r#"
            [
              [
                1,
                true
              ],
              [
                3,
                true
              ],
              [
                2,
                false
              ]
            ]"#]],
    );

    let value: S = serde_json::from_str(
        r#"[
        [1, false],
        [1, true],
        [2, true],
        [2, false]
    ]"#,
    )
    .unwrap();
    let entries: Vec<_> = value.0.into_iter().collect();
    assert_eq!(1, entries[0].0);
    assert!(entries[0].1);
    assert_eq!(2, entries[1].0);
    assert!(!entries[1].1);
}

#[test]
fn duplicate_value_last_wins_btreeset() {
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S(#[serde(with = "::serde_with::rust::sets_last_value_wins")] BTreeSet<W>);
    #[derive(Debug, Eq, Deserialize, Serialize)]
    struct W(i32, bool);
    impl PartialEq for W {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl Ord for W {
        fn cmp(&self, other: &Self) -> cmp::Ordering {
            self.0.cmp(&other.0)
        }
    }
    impl PartialOrd for W {
        fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    // Different values always work
    is_equal(
        S(BTreeSet::from_iter(vec![
            W(1, true),
            W(2, false),
            W(3, true),
        ])),
        expect![[r#"
            [
              [
                1,
                true
              ],
              [
                2,
                false
              ],
              [
                3,
                true
              ]
            ]"#]],
    );

    let value: S = serde_json::from_str(
        r#"[
        [1, false],
        [1, true],
        [2, true],
        [2, false]
    ]"#,
    )
    .unwrap();
    let entries: Vec<_> = value.0.into_iter().collect();
    assert_eq!(1, entries[0].0);
    assert!(entries[0].1);
    assert_eq!(2, entries[1].0);
    assert!(!entries[1].1);
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
struct UnwrapOrSkipRef<'a> {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::unwrap_or_skip"
    )]
    option_field: Option<&'a str>,

    field: &'a str,
}

#[test]
fn unwrap_or_skip_ref_some() {
    let s0 = String::from("g1_&");
    let s1 = String::from("3&t");
    let value = UnwrapOrSkipRef {
        option_field: Some(s0.as_ref()),
        field: s1.as_ref(),
    };
    let expected = expect![[r#"
        {
          "option_field": "g1_&",
          "field": "3&t"
        }"#]];

    // I can't make the following check abstract over [`UnwrapOrSkipRef`]
    // because of incompatible lifetimes.
    let serialized = serde_json::to_string_pretty(&value).unwrap();
    expected.assert_eq(&serialized);
    assert_eq!(
        value,
        serde_json::from_str::<UnwrapOrSkipRef<'_>>(&serialized).unwrap(),
        "Deserialization differs from expected value."
    );
}

#[test]
fn unwrap_or_skip_ref_none() {
    let s1 = String::from("3&t");
    let value = UnwrapOrSkipRef {
        option_field: None,
        field: s1.as_ref(),
    };
    let expected = expect![[r#"
        {
          "field": "3&t"
        }"#]];

    let serialized = serde_json::to_string_pretty(&value).unwrap();
    expected.assert_eq(&serialized);
    assert_eq!(
        value,
        serde_json::from_str::<UnwrapOrSkipRef<'_>>(&serialized).unwrap(),
        "Deserialization differs from expected value."
    );
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
struct UnwrapOrSkip {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::unwrap_or_skip"
    )]
    option_field: Option<String>,

    field: String,
}

#[test]
fn unwrap_or_skip_some() {
    let value = UnwrapOrSkip {
        option_field: Some(String::from("g1_&")),
        field: String::from("3&t"),
    };
    let expected = expect![[r#"
        {
          "option_field": "g1_&",
          "field": "3&t"
        }"#]];

    let serialized = serde_json::to_string_pretty(&value).unwrap();
    expected.assert_eq(&serialized);
    assert_eq!(
        value,
        serde_json::from_str::<UnwrapOrSkip>(&serialized).unwrap(),
        "Deserialization differs from expected value."
    );
}

#[test]
fn unwrap_or_skip_none() {
    let value = UnwrapOrSkip {
        option_field: None,
        field: String::from("3&t"),
    };
    let expected = expect![[r#"
        {
          "field": "3&t"
        }"#]];

    let serialized = serde_json::to_string_pretty(&value).unwrap();
    expected.assert_eq(&serialized);
    assert_eq!(
        value,
        serde_json::from_str::<UnwrapOrSkip>(&serialized).unwrap(),
        "Deserialization differs from expected value."
    );
}
