//! Test Cases

extern crate alloc;

mod collections;
mod default_on;
mod enum_map;
mod frominto;
mod fromintoref;
mod key_value_map;
mod map_tuple_list;
mod pickfirst;
mod serde_as_macro;
mod serde_conv;
mod time;
#[path = "../utils.rs"]
mod utils;

use crate::utils::*;
use alloc::{
    collections::{BTreeMap, BTreeSet, LinkedList, VecDeque},
    rc::{Rc, Weak as RcWeak},
    sync::{Arc, Weak as ArcWeak},
};
use core::{
    cell::{Cell, RefCell},
    ops::{Bound, Range, RangeFrom, RangeInclusive, RangeTo},
    pin::Pin,
};
use expect_test::expect;
use serde::{Deserialize, Serialize};
use serde_with::{
    formats::{CommaSeparator, Flexible, Strict},
    serde_as, BoolFromInt, BytesOrString, DisplayFromStr, IfIsHumanReadable, Map,
    NoneAsEmptyString, OneOrMany, Same, Seq, StringWithSeparator,
};
use std::{
    collections::HashMap,
    sync::{Mutex, RwLock},
};

#[test]
fn test_basic_wrappers() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SBox(#[serde_as(as = "Box<DisplayFromStr>")] Box<u32>);

    is_equal(SBox(Box::new(123)), expect![[r#""123""#]]);

    // Deserialization in generally is not possible, only for unpin types
    #[serde_as]
    #[derive(Debug, Serialize, PartialEq)]
    struct SPin<'a>(#[serde_as(as = "Pin<&DisplayFromStr>")] Pin<&'a u32>);
    let tmp = 123;
    check_serialization(SPin(Pin::new(&tmp)), expect![[r#""123""#]]);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SPinBox(#[serde_as(as = "Pin<Box<DisplayFromStr>>")] Pin<Box<u32>>);

    is_equal(SPinBox(Box::pin(123)), expect![[r#""123""#]]);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SRc(#[serde_as(as = "Rc<DisplayFromStr>")] Rc<u32>);

    is_equal(SRc(Rc::new(123)), expect![[r#""123""#]]);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SPinRc(#[serde_as(as = "Pin<Rc<DisplayFromStr>>")] Pin<Rc<u32>>);

    is_equal(SPinRc(Rc::pin(123)), expect![[r#""123""#]]);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize)]
    struct SRcWeak(#[serde_as(as = "RcWeak<DisplayFromStr>")] RcWeak<u32>);

    check_serialization(SRcWeak(RcWeak::new()), expect![[r#"null"#]]);
    let s: SRcWeak = serde_json::from_str("null").unwrap();
    assert!(s.0.upgrade().is_none());
    let s: SRcWeak = serde_json::from_str("\"123\"").unwrap();
    assert!(s.0.upgrade().is_none());

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SArc(#[serde_as(as = "Arc<DisplayFromStr>")] Arc<u32>);

    is_equal(SArc(Arc::new(123)), expect![[r#""123""#]]);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SPinArc(#[serde_as(as = "Pin<Arc<DisplayFromStr>>")] Pin<Arc<u32>>);

    is_equal(SPinArc(Arc::pin(123)), expect![[r#""123""#]]);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize)]
    struct SArcWeak(#[serde_as(as = "ArcWeak<DisplayFromStr>")] ArcWeak<u32>);

    check_serialization(SArcWeak(ArcWeak::new()), expect![[r#"null"#]]);
    let s: SArcWeak = serde_json::from_str("null").unwrap();
    assert!(s.0.upgrade().is_none());
    let s: SArcWeak = serde_json::from_str("\"123\"").unwrap();
    assert!(s.0.upgrade().is_none());

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SCell(#[serde_as(as = "Cell<DisplayFromStr>")] Cell<u32>);

    is_equal(SCell(Cell::new(123)), expect![[r#""123""#]]);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SRefCell(#[serde_as(as = "RefCell<DisplayFromStr>")] RefCell<u32>);

    is_equal(SRefCell(RefCell::new(123)), expect![[r#""123""#]]);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize)]
    struct SMutex(#[serde_as(as = "Mutex<DisplayFromStr>")] Mutex<u32>);

    check_serialization(SMutex(Mutex::new(123)), expect![[r#""123""#]]);
    let s: SMutex = serde_json::from_str("\"123\"").unwrap();
    assert_eq!(*s.0.lock().unwrap(), 123);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize)]
    struct SRwLock(#[serde_as(as = "RwLock<DisplayFromStr>")] RwLock<u32>);

    let expected = expect![[r#""123""#]];
    check_serialization(SRwLock(RwLock::new(123)), expected);
    let s: SRwLock = serde_json::from_str("\"123\"").unwrap();
    assert_eq!(*s.0.read().unwrap(), 123);
}

#[test]
fn test_option() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "_")] Option<u32>);

    is_equal(S(None), expect![[r#"null"#]]);
    is_equal(S(Some(9)), expect![[r#"9"#]]);
    check_error_deserialization::<S>(
        r#"{}"#,
        expect![[r#"invalid type: map, expected u32 at line 1 column 0"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Struct {
        #[serde_as(as = "_")]
        value: Option<u32>,
    }
    check_error_deserialization::<Struct>(
        r#"{}"#,
        expect![[r#"missing field `value` at line 1 column 2"#]],
    );
}

#[test]
fn test_bound() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "Bound<DisplayFromStr>")] Bound<u32>);

    is_equal(S(Bound::Unbounded), expect![[r#""Unbounded""#]]);
    is_equal(
        S(Bound::Included(42)),
        expect![[r#"
        {
          "Included": "42"
        }"#]],
    );
    is_equal(
        S(Bound::Excluded(42)),
        expect![[r#"
        {
          "Excluded": "42"
        }"#]],
    );
    check_error_deserialization::<S>(r#"{}"#, expect![[r#"expected value at line 1 column 2"#]]);
}

#[test]
fn test_range() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "Range<DisplayFromStr>")] Range<u32>);

    is_equal(
        S(3..7),
        expect![[r#"
        {
          "start": "3",
          "end": "7"
        }"#]],
    );
    check_error_deserialization::<S>(
        r#"{"start": false}"#,
        expect!["invalid type: boolean `false`, expected a string at line 1 column 15"],
    );
}

#[test]
fn test_rangefrom() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "RangeFrom<DisplayFromStr>")] RangeFrom<u32>);

    is_equal(
        S(3..),
        expect![[r#"
        {
          "start": "3"
        }"#]],
    );
    check_error_deserialization::<S>(
        r#"{"start": false}"#,
        expect!["invalid type: boolean `false`, expected a string at line 1 column 15"],
    );
}

#[test]
fn test_rangeto() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "RangeTo<DisplayFromStr>")] RangeTo<u32>);

    is_equal(
        S(..7),
        expect![[r#"
        {
          "end": "7"
        }"#]],
    );
    check_error_deserialization::<S>(
        r#"{"end": false}"#,
        expect!["invalid type: boolean `false`, expected a string at line 1 column 13"],
    );
}

#[test]
fn test_rangeinclusive() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "RangeInclusive<DisplayFromStr>")] RangeInclusive<u32>);

    is_equal(
        S(3..=7),
        expect![[r#"
        {
          "start": "3",
          "end": "7"
        }"#]],
    );
    check_error_deserialization::<S>(
        r#"{"start": false}"#,
        expect!["invalid type: boolean `false`, expected a string at line 1 column 15"],
    );
}

#[test]
fn test_result() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(
        #[serde_as(as = "Result<StringWithSeparator<CommaSeparator, u32>, DisplayFromStr>")]
        Result<Vec<u32>, u32>,
    );

    is_equal(
        S(Ok(vec![1, 2, 3])),
        expect![[r#"
        {
          "Ok": "1,2,3"
        }"#]],
    );
    is_equal(
        S(Err(9)),
        expect![[r#"
            {
              "Err": "9"
            }"#]],
    );
    check_error_deserialization::<S>(r#"{}"#, expect![[r#"expected value at line 1 column 2"#]]);
}

#[test]
fn test_display_fromstr() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "DisplayFromStr")] u32);

    is_equal(S(123), expect![[r#""123""#]]);
}

#[test]
fn test_if_is_human_readable() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "IfIsHumanReadable<DisplayFromStr>")] u32);

    let ser_json = serde_json::to_string(&S(123)).unwrap();
    assert_eq!(ser_json, r#""123""#);
    let ser_rmp = rmp_serde::to_vec(&S(123)).unwrap();
    assert_eq!(ser_rmp, vec![123]);

    let de_json: S = serde_json::from_str(r#""123""#).unwrap();
    assert_eq!(S(123), de_json);
    let de_rmp: S = rmp_serde::from_read(&*vec![123]).unwrap();
    assert_eq!(S(123), de_rmp);
}

#[test]
fn test_tuples() {
    use std::net::IpAddr;
    let ip = "1.2.3.4".parse().unwrap();

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S1(#[serde_as(as = "(DisplayFromStr,)")] (u32,));
    is_equal(
        S1((1,)),
        expect![[r#"
            [
              "1"
            ]"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S2a(#[serde_as(as = "(DisplayFromStr, DisplayFromStr)")] (u32, IpAddr));
    is_equal(
        S2a((555_888, ip)),
        expect![[r#"
            [
              "555888",
              "1.2.3.4"
            ]"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S2b(#[serde_as(as = "(_, DisplayFromStr)")] (u32, IpAddr));
    is_equal(
        S2b((987, ip)),
        expect![[r#"
            [
              987,
              "1.2.3.4"
            ]"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S2c(#[serde_as(as = "(Same, DisplayFromStr)")] (u32, IpAddr));
    is_equal(
        S2c((987, ip)),
        expect![[r#"
            [
              987,
              "1.2.3.4"
            ]"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S6(
        #[serde_as(as = "(Same, Same, Same, Same, Same, Same)")] (u8, u16, u32, i8, i16, i32),
    );
    is_equal(
        S6((8, 16, 32, -8, 16, -32)),
        expect![[r#"
            [
              8,
              16,
              32,
              -8,
              16,
              -32
            ]"#]],
    );
}

#[test]
fn test_arrays() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S0(#[serde_as(as = "[DisplayFromStr; 0]")] [u32; 0]);
    is_equal(S0([]), expect![[r#"[]"#]]);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S1(#[serde_as(as = "[DisplayFromStr; 1]")] [u32; 1]);
    is_equal(
        S1([1]),
        expect![[r#"
            [
              "1"
            ]"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S2(#[serde_as(as = "[Same; 2]")] [u32; 2]);
    is_equal(
        S2([11, 22]),
        expect![[r#"
            [
              11,
              22
            ]"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S32(#[serde_as(as = "[Same; 32]")] [u32; 32]);
    is_equal(
        S32([
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31,
        ]),
        expect![[r#"
            [
              0,
              1,
              2,
              3,
              4,
              5,
              6,
              7,
              8,
              9,
              10,
              11,
              12,
              13,
              14,
              15,
              16,
              17,
              18,
              19,
              20,
              21,
              22,
              23,
              24,
              25,
              26,
              27,
              28,
              29,
              30,
              31
            ]"#]],
    );
}

#[test]
fn test_sequence_like_types() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S1(#[serde_as(as = "Box<[Same]>")] Box<[u32]>);
    is_equal(
        S1(vec![1, 2, 3, 99].into()),
        expect![[r#"
            [
              1,
              2,
              3,
              99
            ]"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S2(#[serde_as(as = "BTreeSet<Same>")] BTreeSet<u32>);
    is_equal(
        S2(vec![1, 2, 3, 99].into_iter().collect()),
        expect![[r#"
            [
              1,
              2,
              3,
              99
            ]"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S3(#[serde_as(as = "LinkedList<Same>")] LinkedList<u32>);
    is_equal(
        S3(vec![1, 2, 3, 99].into_iter().collect()),
        expect![[r#"
            [
              1,
              2,
              3,
              99
            ]"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S4(#[serde_as(as = "Vec<Same>")] Vec<u32>);
    is_equal(
        S4(vec![1, 2, 3, 99]),
        expect![[r#"
            [
              1,
              2,
              3,
              99
            ]"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S5(#[serde_as(as = "VecDeque<Same>")] VecDeque<u32>);
    is_equal(
        S5(vec![1, 2, 3, 99].into()),
        expect![[r#"
            [
              1,
              2,
              3,
              99
            ]"#]],
    );
}

#[test]
fn test_none_as_empty_string() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "NoneAsEmptyString")] Option<String>);

    is_equal(S(None), expect![[r#""""#]]);
    is_equal(S(Some("Hello".to_string())), expect![[r#""Hello""#]]);
}

#[test]
fn test_bytes_or_string() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "BytesOrString")] Vec<u8>);

    is_equal(
        S(vec![1, 2, 3]),
        expect![[r#"
            [
              1,
              2,
              3
            ]"#]],
    );
    check_deserialization(S(vec![72, 101, 108, 108, 111]), r#""Hello""#);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SVec(#[serde_as(as = "Vec<BytesOrString>")] Vec<Vec<u8>>);

    is_equal(
        SVec(vec![vec![1, 2, 3]]),
        expect![[r#"
            [
              [
                1,
                2,
                3
              ]
            ]"#]],
    );
    check_deserialization(
        SVec(vec![
            vec![72, 101, 108, 108, 111],
            vec![87, 111, 114, 108, 100],
            vec![1, 2, 3],
        ]),
        r#"["Hello","World",[1,2,3]]"#,
    );
}

#[test]
fn string_with_separator() {
    use serde_with::{
        formats::{CommaSeparator, DosLineSeparator, SpaceSeparator, UnixLineSeparator},
        StringWithSeparator,
    };

    #[serde_as]
    #[derive(Deserialize, Serialize)]
    struct A {
        #[serde_as(as = "StringWithSeparator::<SpaceSeparator, String>")]
        tags: Vec<String>,
        #[serde_as(as = "StringWithSeparator::<CommaSeparator, String>")]
        more_tags: BTreeSet<String>,
        #[serde_as(as = "StringWithSeparator::<UnixLineSeparator, String>")]
        lf_tags: BTreeSet<String>,
        #[serde_as(as = "StringWithSeparator::<DosLineSeparator, String>")]
        crlf_tags: BTreeSet<String>,
    }

    let v: A = serde_json::from_str(
        r##"{
    "tags": "#hello #world",
    "more_tags": "foo,bar,bar",
    "lf_tags": "foo\nbar\nbar",
    "crlf_tags": "foo\r\nbar\r\nbar"
}"##,
    )
    .unwrap();
    assert_eq!(vec!["#hello", "#world"], v.tags);
    assert_eq!(
        BTreeSet::from(["foo".to_string(), "bar".to_string()]),
        v.more_tags
    );
    assert_eq!(
        BTreeSet::from(["foo".to_string(), "bar".to_string()]),
        v.lf_tags
    );
    assert_eq!(
        BTreeSet::from(["foo".to_string(), "bar".to_string()]),
        v.crlf_tags
    );

    let x = A {
        tags: vec!["1".to_string(), "2".to_string(), "3".to_string()],
        more_tags: BTreeSet::default(),
        lf_tags: BTreeSet::default(),
        crlf_tags: BTreeSet::default(),
    };
    assert_eq!(
        r#"{"tags":"1 2 3","more_tags":"","lf_tags":"","crlf_tags":""}"#,
        serde_json::to_string(&x).unwrap()
    );
}

#[test]
fn test_vec_skip_error() {
    use serde_with::VecSkipError;

    #[serde_as]
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S {
        tag: String,
        #[serde_as(as = "VecSkipError<_>")]
        values: Vec<u8>,
    }

    check_deserialization(
        S {
            tag: "type".into(),
            values: vec![0, 1],
        },
        r#"{"tag":"type","values":[0, "str", 1, [10, 11], -2, {}, 300]}"#,
    );
    check_error_deserialization::<S>(
        r#"{"tag":"type", "values":[0, "str", 1, , 300]}"#,
        expect!["expected value at line 1 column 39"],
    );
    is_equal(
        S {
            tag: "round-trip".into(),
            values: vec![0, 255],
        },
        expect![[r#"
        {
          "tag": "round-trip",
          "values": [
            0,
            255
          ]
        }"#]],
    );
}

#[test]
fn test_map_skip_error_btreemap() {
    use serde_with::MapSkipError;

    #[serde_as]
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S {
        tag: String,
        #[serde_as(as = "MapSkipError<DisplayFromStr, _>")]
        values: BTreeMap<u8, u8>,
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
fn test_map_skip_error_btreemap_flatten() {
    use serde_with::MapSkipError;

    #[serde_as]
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S {
        tag: String,
        #[serde_as(as = "MapSkipError<DisplayFromStr, _>")]
        #[serde(flatten)]
        values: BTreeMap<u8, u8>,
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

#[test]
fn test_map_skip_error_hashmap() {
    use serde_with::MapSkipError;

    #[serde_as]
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S {
        tag: String,
        #[serde_as(as = "MapSkipError<DisplayFromStr, _>")]
        values: HashMap<u8, u8>,
    }

    check_deserialization(
        S {
            tag: "type".into(),
            values: [(0, 1)].into_iter().collect(),
        },
        r#"
        {
          "tag":"type",
          "values": {
            "0": 1,
            "str": 2,
            "3": "str",
            "4": [10, 11],
            "5": {}
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
            values: [(255, 0)].into_iter().collect(),
        },
        expect![[r#"
        {
          "tag": "round-trip",
          "values": {
            "255": 0
          }
        }"#]],
    );
}

#[test]
fn test_map_skip_error_hashmap_flatten() {
    use serde_with::MapSkipError;

    #[serde_as]
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S {
        tag: String,
        #[serde_as(as = "MapSkipError<DisplayFromStr, _>")]
        #[serde(flatten)]
        values: HashMap<u8, u8>,
    }

    check_deserialization(
        S {
            tag: "type".into(),
            values: [(0, 1)].into_iter().collect(),
        },
        r#"
        {
          "tag":"type",
          "0": 1,
          "str": 2,
          "3": "str",
          "4": [10, 11],
          "5": {}
        }"#,
    );
    is_equal(
        S {
            tag: "round-trip".into(),
            values: [(255, 0)].into_iter().collect(),
        },
        expect![[r#"
        {
          "tag": "round-trip",
          "255": 0
        }"#]],
    );
}

#[test]
fn test_serialize_reference() {
    #[serde_as]
    #[derive(Debug, Serialize)]
    struct S1<'a>(#[serde_as(as = "Vec<DisplayFromStr>")] &'a Vec<u32>);
    check_serialization(
        S1(&vec![1, 2]),
        expect![[r#"
        [
          "1",
          "2"
        ]"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize)]
    struct S1a<'a>(#[serde_as(as = "&Vec<DisplayFromStr>")] &'a Vec<u32>);
    check_serialization(
        S1a(&vec![1, 2]),
        expect![[r#"
        [
          "1",
          "2"
        ]"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize)]
    struct S1Mut<'a>(#[serde_as(as = "Vec<DisplayFromStr>")] &'a mut Vec<u32>);
    check_serialization(
        S1Mut(&mut vec![1, 2]),
        expect![[r#"
        [
          "1",
          "2"
        ]"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize)]
    struct S1aMut<'a>(#[serde_as(as = "&mut Vec<DisplayFromStr>")] &'a mut Vec<u32>);
    check_serialization(
        S1aMut(&mut vec![1, 2]),
        expect![[r#"
        [
          "1",
          "2"
        ]"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize)]
    struct S2<'a>(#[serde_as(as = "&[DisplayFromStr]")] &'a [u32]);
    check_serialization(
        S2(&[1, 2]),
        expect![[r#"
            [
              "1",
              "2"
            ]"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize)]
    struct S3<'a>(
        #[serde_as(as = "&BTreeMap<DisplayFromStr, DisplayFromStr>")] &'a BTreeMap<bool, u32>,
    );
    let bmap = vec![(false, 123), (true, 456)].into_iter().collect();
    check_serialization(
        S3(&bmap),
        expect![[r#"
            {
              "false": "123",
              "true": "456"
            }"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize)]
    struct S4<'a>(#[serde_as(as = "&Vec<(_, DisplayFromStr)>")] &'a BTreeMap<bool, u32>);
    let bmap = vec![(false, 123), (true, 456)].into_iter().collect();
    check_serialization(
        S4(&bmap),
        expect![[r#"
            [
              [
                false,
                "123"
              ],
              [
                true,
                "456"
              ]
            ]"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize)]
    struct S5<'a>(
        #[serde_as(as = "&BTreeMap<DisplayFromStr, &Vec<(_, _)>>")]
        &'a Vec<(u32, &'a BTreeMap<bool, String>)>,
    );
    let bmap0 = vec![(false, "123".to_string()), (true, "456".to_string())]
        .into_iter()
        .collect();
    let bmap1 = vec![(true, "Hello".to_string()), (false, "World".to_string())]
        .into_iter()
        .collect();
    let vec = vec![(111, &bmap0), (999, &bmap1)];
    check_serialization(
        S5(&vec),
        expect![[r#"
            {
              "111": [
                [
                  false,
                  "123"
                ],
                [
                  true,
                  "456"
                ]
              ],
              "999": [
                [
                  false,
                  "World"
                ],
                [
                  true,
                  "Hello"
                ]
              ]
            }"#]],
    );
}

#[test]
fn test_big_arrays() {
    // Single Big Array
    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct S1(#[serde_as(as = "[_; 64]")] [u8; 64]);
    is_equal_compact(
        S1([0; 64]),
        expect![[
            r#"[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]"#
        ]],
    );
    // Too few entries
    check_error_deserialization::<S1>(
        r#"[0,0,0,0,0,0,0,0,0,0,0,0,0,0]"#,
        expect![[r#"invalid length 14, expected an array of size 64 at line 1 column 29"#]],
    );
    // Too many entries
    check_error_deserialization::<S1>(
        r#"[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]"#,
        expect![[r#"trailing characters at line 1 column 130"#]],
    );

    // Single Big Array
    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct S2(#[serde_as(as = "[DisplayFromStr; 40]")] [u8; 40]);
    is_equal_compact(
        S2([0; 40]),
        expect![[
            r#"["0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0","0"]"#
        ]],
    );

    // Nested Big Arrays
    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct S3(#[serde_as(as = "[[_; 34]; 33]")] [[u8; 34]; 33]);
    is_equal_compact(
        S3([[0; 34]; 33]),
        expect![[
            r#"[[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]]"#
        ]],
    );
}

#[test]
fn test_bytes() {
    // The test case is copied from
    // https://github.com/serde-rs/bytes/blob/cbae606b9dc225fc094b031cc84eac9493da2058/tests/test_derive.rs
    // Original code by @dtolnay

    use alloc::borrow::Cow;
    use serde_test::{assert_de_tokens, assert_tokens, Token};
    use serde_with::Bytes;

    #[serde_as]
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Test<'a> {
        #[serde_as(as = "Bytes")]
        array: [u8; 52],

        #[serde_as(as = "Bytes")]
        slice: &'a [u8],

        #[serde_as(as = "Bytes")]
        vec: Vec<u8>,

        #[serde_as(as = "Bytes")]
        cow_slice: Cow<'a, [u8]>,

        #[serde_as(as = "Box<Bytes>")]
        boxed_array: Box<[u8; 52]>,

        #[serde_as(as = "Bytes")]
        boxed_array2: Box<[u8; 52]>,

        #[serde_as(as = "Bytes")]
        boxed_slice: Box<[u8]>,

        #[serde_as(as = "Option<Bytes>")]
        opt_slice: Option<&'a [u8]>,

        #[serde_as(as = "Option<Bytes>")]
        opt_vec: Option<Vec<u8>>,

        #[serde_as(as = "Option<Bytes>")]
        opt_cow_slice: Option<Cow<'a, [u8]>>,
    }

    let test = Test {
        array: *b"ABCDEFGHIJKLMNOPQRSTUVWXZYabcdefghijklmnopqrstuvwxyz",
        slice: b"...",
        vec: b"...".to_vec(),
        cow_slice: Cow::Borrowed(b"..."),
        boxed_array: Box::new(*b"ABCDEFGHIJKLMNOPQRSTUVWXZYabcdefghijklmnopqrstuvwxyz"),
        boxed_array2: Box::new(*b"ABCDEFGHIJKLMNOPQRSTUVWXZYabcdefghijklmnopqrstuvwxyz"),
        boxed_slice: b"...".to_vec().into_boxed_slice(),
        opt_slice: Some(b"..."),
        opt_vec: Some(b"...".to_vec()),
        opt_cow_slice: Some(Cow::Borrowed(b"...")),
    };

    assert_tokens(
        &test,
        &[
            Token::Struct {
                name: "Test",
                len: 10,
            },
            Token::Str("array"),
            Token::BorrowedBytes(b"ABCDEFGHIJKLMNOPQRSTUVWXZYabcdefghijklmnopqrstuvwxyz"),
            Token::Str("slice"),
            Token::BorrowedBytes(b"..."),
            Token::Str("vec"),
            Token::Bytes(b"..."),
            Token::Str("cow_slice"),
            Token::BorrowedBytes(b"..."),
            Token::Str("boxed_array"),
            Token::BorrowedBytes(b"ABCDEFGHIJKLMNOPQRSTUVWXZYabcdefghijklmnopqrstuvwxyz"),
            Token::Str("boxed_array2"),
            Token::BorrowedBytes(b"ABCDEFGHIJKLMNOPQRSTUVWXZYabcdefghijklmnopqrstuvwxyz"),
            Token::Str("boxed_slice"),
            Token::Bytes(b"..."),
            Token::Str("opt_slice"),
            Token::Some,
            Token::BorrowedBytes(b"..."),
            Token::Str("opt_vec"),
            Token::Some,
            Token::Bytes(b"..."),
            Token::Str("opt_cow_slice"),
            Token::Some,
            Token::BorrowedBytes(b"..."),
            Token::StructEnd,
        ],
    );

    // Test string deserialization
    assert_de_tokens(
        &test,
        &[
            Token::Struct {
                name: "Test",
                len: 10,
            },
            Token::Str("array"),
            Token::BorrowedStr("ABCDEFGHIJKLMNOPQRSTUVWXZYabcdefghijklmnopqrstuvwxyz"),
            Token::Str("slice"),
            Token::BorrowedStr("..."),
            Token::Str("vec"),
            Token::Bytes(b"..."),
            Token::Str("cow_slice"),
            Token::BorrowedStr("..."),
            Token::Str("boxed_array"),
            Token::BorrowedStr("ABCDEFGHIJKLMNOPQRSTUVWXZYabcdefghijklmnopqrstuvwxyz"),
            Token::Str("boxed_array2"),
            Token::BorrowedStr("ABCDEFGHIJKLMNOPQRSTUVWXZYabcdefghijklmnopqrstuvwxyz"),
            Token::Str("boxed_slice"),
            Token::Bytes(b"..."),
            Token::Str("opt_slice"),
            Token::Some,
            Token::BorrowedStr("..."),
            Token::Str("opt_vec"),
            Token::Some,
            Token::Bytes(b"..."),
            Token::Str("opt_cow_slice"),
            Token::Some,
            Token::BorrowedStr("..."),
            Token::StructEnd,
        ],
    );
}

#[test]
fn test_one_or_many_prefer_one() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S1Vec(#[serde_as(as = "OneOrMany<_>")] Vec<u32>);

    // Normal
    is_equal(S1Vec(vec![]), expect![[r#"[]"#]]);
    is_equal(S1Vec(vec![1]), expect![[r#"1"#]]);
    is_equal(
        S1Vec(vec![1, 2, 3]),
        expect![[r#"
            [
              1,
              2,
              3
            ]"#]],
    );
    check_deserialization(S1Vec(vec![1]), r#"1"#);
    check_deserialization(S1Vec(vec![1]), r#"[1]"#);
    check_error_deserialization::<S1Vec>(
        r#"{}"#,
        expect![[r#"
        OneOrMany could not deserialize any variant:
          One: invalid type: map, expected u32
          Many: invalid type: map, expected a sequence"#]],
    );
    check_error_deserialization::<S1Vec>(
        r#""xx""#,
        expect![[r#"
        OneOrMany could not deserialize any variant:
          One: invalid type: string "xx", expected u32
          Many: invalid type: string "xx", expected a sequence"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S2Vec(#[serde_as(as = "OneOrMany<DisplayFromStr>")] Vec<u32>);

    // Normal
    is_equal(S2Vec(vec![]), expect![[r#"[]"#]]);
    is_equal(S2Vec(vec![1]), expect![[r#""1""#]]);
    is_equal(
        S2Vec(vec![1, 2, 3]),
        expect![[r#"
            [
              "1",
              "2",
              "3"
            ]"#]],
    );
    check_deserialization(S2Vec(vec![1]), r#""1""#);
    check_deserialization(S2Vec(vec![1]), r#"["1"]"#);
    check_error_deserialization::<S2Vec>(
        r#"{}"#,
        expect![[r#"
        OneOrMany could not deserialize any variant:
          One: invalid type: map, expected a string
          Many: invalid type: map, expected a sequence"#]],
    );
    check_error_deserialization::<S2Vec>(
        r#""xx""#,
        expect![[r#"
        OneOrMany could not deserialize any variant:
          One: invalid digit found in string
          Many: invalid type: string "xx", expected a sequence"#]],
    );
}

#[test]
fn test_one_or_many_prefer_many() {
    use serde_with::formats::PreferMany;

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S1Vec(#[serde_as(as = "OneOrMany<_, PreferMany>")] Vec<u32>);

    // Normal
    is_equal(S1Vec(vec![]), expect![[r#"[]"#]]);
    is_equal(
        S1Vec(vec![1]),
        expect![[r#"
            [
              1
            ]"#]],
    );
    is_equal(
        S1Vec(vec![1, 2, 3]),
        expect![[r#"
            [
              1,
              2,
              3
            ]"#]],
    );
    check_deserialization(S1Vec(vec![1]), r#"1"#);
    check_deserialization(S1Vec(vec![1]), r#"[1]"#);
    check_error_deserialization::<S1Vec>(
        r#"{}"#,
        expect![[r#"
        OneOrMany could not deserialize any variant:
          One: invalid type: map, expected u32
          Many: invalid type: map, expected a sequence"#]],
    );
    check_error_deserialization::<S1Vec>(
        r#""xx""#,
        expect![[r#"
        OneOrMany could not deserialize any variant:
          One: invalid type: string "xx", expected u32
          Many: invalid type: string "xx", expected a sequence"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S2Vec(#[serde_as(as = "OneOrMany<DisplayFromStr, PreferMany>")] Vec<u32>);

    // Normal
    is_equal(S2Vec(vec![]), expect![[r#"[]"#]]);
    is_equal(
        S2Vec(vec![1]),
        expect![[r#"
            [
              "1"
            ]"#]],
    );
    is_equal(
        S2Vec(vec![1, 2, 3]),
        expect![[r#"
            [
              "1",
              "2",
              "3"
            ]"#]],
    );
    check_deserialization(S2Vec(vec![1]), r#""1""#);
    check_deserialization(S2Vec(vec![1]), r#"["1"]"#);
    check_error_deserialization::<S2Vec>(
        r#"{}"#,
        expect![[r#"
        OneOrMany could not deserialize any variant:
          One: invalid type: map, expected a string
          Many: invalid type: map, expected a sequence"#]],
    );
    check_error_deserialization::<S2Vec>(
        r#""xx""#,
        expect![[r#"
        OneOrMany could not deserialize any variant:
          One: invalid digit found in string
          Many: invalid type: string "xx", expected a sequence"#]],
    );
}

/// Test that Cow borrows from the input
#[test]
fn test_borrow_cow_str() {
    use alloc::borrow::Cow;
    use serde::de::{
        value::{BorrowedStrDeserializer, MapDeserializer},
        IntoDeserializer,
    };
    use serde_test::{assert_ser_tokens, Token};
    use serde_with::BorrowCow;

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S1<'a> {
        #[serde_as(as = "BorrowCow")]
        cow: Cow<'a, str>,
        #[serde_as(as = "Option<BorrowCow>")]
        opt: Option<Cow<'a, str>>,
        #[serde_as(as = "Box<BorrowCow>")]
        b: Box<Cow<'a, str>>,
        #[serde_as(as = "[BorrowCow; 1]")]
        arr: [Cow<'a, str>; 1],
    }

    assert_ser_tokens(
        &S1 {
            cow: "abc".into(),
            opt: Some("foo".into()),
            b: Box::new("bar".into()),
            arr: ["def".into()],
        },
        &[
            Token::Struct { name: "S1", len: 4 },
            Token::Str("cow"),
            Token::BorrowedStr("abc"),
            Token::Str("opt"),
            Token::Some,
            Token::BorrowedStr("foo"),
            Token::Str("b"),
            Token::BorrowedStr("bar"),
            Token::Str("arr"),
            Token::Tuple { len: 1 },
            Token::BorrowedStr("def"),
            Token::TupleEnd,
            Token::StructEnd,
        ],
    );
    let s1: S1<'_> = serde_json::from_str(
        r#"{
        "cow": "abc",
        "opt": "foo",
        "b": "bar",
        "arr": ["def"]
    }"#,
    )
    .unwrap();
    assert!(matches!(s1.cow, Cow::Borrowed(_)));
    assert!(matches!(s1.opt, Some(Cow::Borrowed(_))));
    assert!(matches!(*s1.b, Cow::Borrowed(_)));
    assert!(matches!(s1.arr, [Cow::Borrowed(_)]));
    let s1: S1<'_> = serde_json::from_str(
        r#"{
        "cow": "a\"c",
        "opt": "f\"o",
        "b": "b\"r",
        "arr": ["d\"f"]
    }"#,
    )
    .unwrap();
    assert!(matches!(s1.cow, Cow::Owned(_)));
    assert!(matches!(s1.opt, Some(Cow::Owned(_))));
    assert!(matches!(*s1.b, Cow::Owned(_)));
    assert!(matches!(s1.arr, [Cow::Owned(_)]));

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S2<'a> {
        #[serde_as(as = "BorrowCow")]
        cow: Cow<'a, [u8]>,
        #[serde_as(as = "Option<BorrowCow>")]
        opt: Option<Cow<'a, [u8]>>,
    }

    assert_ser_tokens(
        &S2 {
            cow: b"abc"[..].into(),
            opt: Some(b"foo"[..].into()),
        },
        &[
            Token::Struct { name: "S2", len: 2 },
            Token::Str("cow"),
            Token::Seq { len: Some(3) },
            Token::U8(b'a'),
            Token::U8(b'b'),
            Token::U8(b'c'),
            Token::SeqEnd,
            Token::Str("opt"),
            Token::Some,
            Token::Seq { len: Some(3) },
            Token::U8(b'f'),
            Token::U8(b'o'),
            Token::U8(b'o'),
            Token::SeqEnd,
            Token::StructEnd,
        ],
    );

    // Check that a manual borrow works too
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S3<'a> {
        #[serde(borrow = "'a")]
        #[serde_as(as = "BorrowCow")]
        borrowed: Cow<'a, [u8]>,
        // TODO add a test for Cow<'b, [u8; N]>
        // #[serde_as(as = "BorrowCow")]
        // Cow<'b, [u8; N]>,
    }

    struct BorrowedStr(&'static str);

    impl<'de> IntoDeserializer<'de> for BorrowedStr {
        type Deserializer = BorrowedStrDeserializer<'de, serde::de::value::Error>;

        fn into_deserializer(self) -> Self::Deserializer {
            BorrowedStrDeserializer::new(self.0)
        }
    }

    let deser = MapDeserializer::new(IntoIterator::into_iter([
        ("copied", BorrowedStr("copied")),
        ("borrowed", BorrowedStr("borrowed")),
    ]));
    let s3 = S3::deserialize(deser).unwrap();
    assert!(matches!(s3.borrowed, Cow::Borrowed(_)));
}

#[test]
fn test_boolfromint() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "BoolFromInt")] bool);

    is_equal(S(false), expect![[r#"0"#]]);
    is_equal(S(true), expect![[r#"1"#]]);
    check_error_deserialization::<S>(
        "2",
        expect![[r#"invalid value: integer `2`, expected 0 or 1 at line 1 column 1"#]],
    );
    check_error_deserialization::<S>(
        "-100",
        expect![[r#"invalid value: integer `-100`, expected 0 or 1 at line 1 column 4"#]],
    );
    check_error_deserialization::<S>(
        "18446744073709551615",
        expect![[
            r#"invalid value: integer `18446744073709551615`, expected 0 or 1 at line 1 column 20"#
        ]],
    );
    check_error_deserialization::<S>(
        r#""""#,
        expect![[r#"invalid type: string "", expected an integer 0 or 1 at line 1 column 2"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SStrict(#[serde_as(as = "BoolFromInt<Strict>")] bool);

    is_equal(SStrict(false), expect![[r#"0"#]]);
    is_equal(SStrict(true), expect![[r#"1"#]]);
    check_error_deserialization::<SStrict>(
        "2",
        expect![[r#"invalid value: integer `2`, expected 0 or 1 at line 1 column 1"#]],
    );
    check_error_deserialization::<SStrict>(
        "-100",
        expect![[r#"invalid value: integer `-100`, expected 0 or 1 at line 1 column 4"#]],
    );
    check_error_deserialization::<SStrict>(
        "18446744073709551615",
        expect![[
            r#"invalid value: integer `18446744073709551615`, expected 0 or 1 at line 1 column 20"#
        ]],
    );
    check_error_deserialization::<SStrict>(
        r#""""#,
        expect![[r#"invalid type: string "", expected an integer 0 or 1 at line 1 column 2"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SFlexible(#[serde_as(as = "BoolFromInt<Flexible>")] bool);

    is_equal(SFlexible(false), expect![[r#"0"#]]);
    is_equal(SFlexible(true), expect![[r#"1"#]]);
    check_deserialization::<SFlexible>(SFlexible(true), "2");
    check_deserialization::<SFlexible>(SFlexible(true), "-100");
    check_deserialization::<SFlexible>(SFlexible(true), "18446744073709551615");
    check_error_deserialization::<SFlexible>(
        r#""""#,
        expect![[r#"invalid type: string "", expected an integer at line 1 column 2"#]],
    );
}
