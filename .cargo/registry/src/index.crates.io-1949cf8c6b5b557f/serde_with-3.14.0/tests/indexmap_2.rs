//! Test Cases

mod utils;

use crate::utils::{check_deserialization, check_error_deserialization, is_equal};
use expect_test::expect;
use indexmap_2::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr, Same};
use std::net::IpAddr;

#[test]
fn test_indexmap() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "IndexMap<DisplayFromStr, DisplayFromStr>")] IndexMap<u8, u32>);

    // Normal
    is_equal(
        S([(1, 1), (3, 3), (111, 111)].iter().copied().collect()),
        expect![[r#"
            {
              "1": "1",
              "3": "3",
              "111": "111"
            }"#]],
    );
    is_equal(S(IndexMap::default()), expect![[r#"{}"#]]);
}

#[test]
fn test_indexset() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "IndexSet<DisplayFromStr>")] IndexSet<u32>);

    // Normal
    is_equal(
        S([1, 2, 3, 4, 5].iter().copied().collect()),
        expect![[r#"
            [
              "1",
              "2",
              "3",
              "4",
              "5"
            ]"#]],
    );
    is_equal(S(IndexSet::default()), expect![[r#"[]"#]]);
}

#[test]
fn test_map_as_tuple_list() {
    let ip = "1.2.3.4".parse().unwrap();
    let ip2 = "255.255.255.255".parse().unwrap();

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SI(#[serde_as(as = "Vec<(DisplayFromStr, DisplayFromStr)>")] IndexMap<u32, IpAddr>);

    let map: IndexMap<_, _> = vec![(1, ip), (10, ip), (200, ip2)].into_iter().collect();
    is_equal(
        SI(map.clone()),
        expect![[r#"
            [
              [
                "1",
                "1.2.3.4"
              ],
              [
                "10",
                "1.2.3.4"
              ],
              [
                "200",
                "255.255.255.255"
              ]
            ]"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SI2(#[serde_as(as = "Vec<(Same, DisplayFromStr)>")] IndexMap<u32, IpAddr>);

    is_equal(
        SI2(map),
        expect![[r#"
            [
              [
                1,
                "1.2.3.4"
              ],
              [
                10,
                "1.2.3.4"
              ],
              [
                200,
                "255.255.255.255"
              ]
            ]"#]],
    );
}

#[test]
fn duplicate_key_first_wins_indexmap() {
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S(#[serde(with = "::serde_with::rust::maps_first_key_wins")] IndexMap<usize, usize>);

    // Different value and key always works
    is_equal(
        S(IndexMap::from_iter(vec![(1, 1), (2, 2), (3, 3)])),
        expect![[r#"
            {
              "1": 1,
              "2": 2,
              "3": 3
            }"#]],
    );

    // Same value for different keys is ok
    is_equal(
        S(IndexMap::from_iter(vec![(1, 1), (2, 1), (3, 1)])),
        expect![[r#"
            {
              "1": 1,
              "2": 1,
              "3": 1
            }"#]],
    );

    // Duplicate keys, the first one is used
    check_deserialization(
        S(IndexMap::from_iter(vec![(1, 1), (2, 2)])),
        r#"{"1": 1, "2": 2, "1": 3}"#,
    );
}

#[test]
fn prohibit_duplicate_key_indexmap() {
    #[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
    struct S(
        #[serde(with = "::serde_with::rust::maps_duplicate_key_is_error")] IndexMap<usize, usize>,
    );

    // Different value and key always works
    is_equal(
        S(IndexMap::from_iter(vec![(1, 1), (2, 2), (3, 3)])),
        expect![[r#"
            {
              "1": 1,
              "2": 2,
              "3": 3
            }"#]],
    );

    // Same value for different keys is ok
    is_equal(
        S(IndexMap::from_iter(vec![(1, 1), (2, 1), (3, 1)])),
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
fn duplicate_value_last_wins_indexset() {
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S(#[serde(with = "::serde_with::rust::sets_last_value_wins")] IndexSet<W>);

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
        S(IndexSet::from_iter(vec![
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

#[test]
fn prohibit_duplicate_value_indexset() {
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S(#[serde(with = "::serde_with::rust::sets_duplicate_value_is_error")] IndexSet<usize>);

    is_equal(
        S(IndexSet::from_iter(vec![1, 2, 3, 4])),
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
fn test_map_skip_error_indexmap() {
    use serde_with::MapSkipError;

    #[serde_as]
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S {
        tag: String,
        #[serde_as(as = "MapSkipError<DisplayFromStr, _>")]
        values: IndexMap<u8, u8>,
    }

    check_deserialization(
        S {
            tag: "type".into(),
            values: [(0, 1), (10, 20)].into_iter().collect(),
        },
        r#"
        {
          "tag":"type",
          "values": {
            "0": 1,
            "str": 2,
            "3": "str",
            "4": [10, 11],
            "5": {},
            "10": 20
          }
        }"#,
    );
    check_error_deserialization::<S>(
        r#"{"tag":"type", "values":{"0": 1,}}"#,
        expect!["trailing comma at line 1 column 33"],
    );
    is_equal(
        S {
            tag: "round-trip".into(),
            values: [(0, 0), (255, 255)].into_iter().collect(),
        },
        expect![[r#"
        {
          "tag": "round-trip",
          "values": {
            "0": 0,
            "255": 255
          }
        }"#]],
    );
}

#[test]
fn test_map_skip_error_indexmap_flatten() {
    use serde_with::MapSkipError;

    #[serde_as]
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S {
        tag: String,
        #[serde_as(as = "MapSkipError<DisplayFromStr, _>")]
        #[serde(flatten)]
        values: IndexMap<u8, u8>,
    }

    check_deserialization(
        S {
            tag: "type".into(),
            values: [(0, 1), (10, 20)].into_iter().collect(),
        },
        r#"
        {
          "tag":"type",
          "0": 1,
          "str": 2,
          "3": "str",
          "4": [10, 11],
          "5": {},
          "10": 20
        }"#,
    );
    is_equal(
        S {
            tag: "round-trip".into(),
            values: [(0, 0), (255, 255)].into_iter().collect(),
        },
        expect![[r#"
        {
          "tag": "round-trip",
          "0": 0,
          "255": 255
        }"#]],
    );
}
