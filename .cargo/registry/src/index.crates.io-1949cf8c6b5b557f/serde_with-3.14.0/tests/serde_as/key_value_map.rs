use super::*;
use core::str::FromStr;
use serde_test::Configure;
use serde_with::{serde_as, KeyValueMap};
use std::net::IpAddr;

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
struct KVMap<E> {
    #[serde_as(as = "KeyValueMap<_>")]
    #[serde(bound(serialize = "E: Serialize", deserialize = "E: Deserialize<'de>"))]
    foo: Vec<E>,
}

#[test]
fn test_kvmap_struct_json() {
    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Struct {
        #[serde(rename = "$key$")]
        key: String,
        bar: String,
        baz: i32,
    }

    let kvmap = KVMap {
        foo: vec![
            Struct {
                key: "a".to_string(),
                bar: "b".to_string(),
                baz: 1,
            },
            Struct {
                key: "c".to_string(),
                bar: "d".to_string(),
                baz: 2,
            },
        ],
    };

    is_equal(
        kvmap,
        expect![[r#"
            {
              "a": {
                "bar": "b",
                "baz": 1
              },
              "c": {
                "bar": "d",
                "baz": 2
              }
            }"#]],
    );
}

#[test]
fn test_kvmap_tuple_struct_json() {
    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TupleStruct(String, String, i32);

    let kvmap = KVMap {
        foo: vec![
            TupleStruct("a".to_string(), "b".to_string(), 1),
            TupleStruct("c".to_string(), "d".to_string(), 2),
        ],
    };

    is_equal(
        kvmap,
        expect![[r#"
            {
              "a": [
                "b",
                1
              ],
              "c": [
                "d",
                2
              ]
            }"#]],
    );
}

#[test]
fn test_kvmap_tuple_json() {
    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    struct Tuple((String, String, i32));

    let kvmap = KVMap {
        foo: vec![
            Tuple(("a".to_string(), "b".to_string(), 1)),
            Tuple(("c".to_string(), "d".to_string(), 2)),
        ],
    };

    is_equal(
        kvmap,
        expect![[r#"
            {
              "a": [
                "b",
                1
              ],
              "c": [
                "d",
                2
              ]
            }"#]],
    );
}

#[test]
fn test_kvmap_map_json() {
    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    struct Map(BTreeMap<String, String>);

    let kvmap = KVMap {
        foo: vec![
            Map(BTreeMap::from([
                ("$key$".to_string(), "a".to_string()),
                ("bar".to_string(), "b".to_string()),
                ("baz".to_string(), "1".to_string()),
            ])),
            Map(BTreeMap::from([
                ("$key$".to_string(), "c".to_string()),
                ("bar".to_string(), "d".to_string()),
                ("baz".to_string(), "2".to_string()),
            ])),
        ],
    };

    is_equal(
        kvmap,
        expect![[r#"
            {
              "a": {
                "bar": "b",
                "baz": "1"
              },
              "c": {
                "bar": "d",
                "baz": "2"
              }
            }"#]],
    );
}

#[test]
fn test_kvmap_seq_json() {
    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    struct Seq(Vec<String>);

    let kvmap = KVMap {
        foo: vec![
            Seq(vec!["a".to_string(), "b".to_string(), "1".to_string()]),
            Seq(vec!["c".to_string(), "d".to_string(), "2".to_string()]),
        ],
    };

    is_equal(
        kvmap,
        expect![[r#"
            {
              "a": [
                "b",
                "1"
              ],
              "c": [
                "d",
                "2"
              ]
            }"#]],
    );
}

#[test]
fn test_kvmap_complex_key_serde_test() {
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct ComplexKey {
        #[serde(rename = "$key$")]
        net: (IpAddr, u8),
        bar: String,
        baz: i32,
    }

    let kvmap = KVMap {
        foo: vec![
            ComplexKey {
                net: (IpAddr::from_str("127.0.0.0").unwrap(), 24),
                bar: "b".to_string(),
                baz: 1,
            },
            ComplexKey {
                net: (IpAddr::from_str("::7777:dead:beef").unwrap(), 48),
                bar: "d".to_string(),
                baz: 2,
            },
        ],
    };

    {
        use serde_test::Token::*;
        serde_test::assert_tokens(
            &kvmap.clone().readable(),
            &[
                Map {
                    len: Option::Some(2),
                },
                Tuple { len: 2 },
                Str("127.0.0.0"),
                U8(24),
                TupleEnd,
                Struct {
                    name: "ComplexKey",
                    len: 2,
                },
                Str("bar"),
                Str("b"),
                Str("baz"),
                I32(1),
                StructEnd,
                Tuple { len: 2 },
                Str("::7777:dead:beef"),
                U8(48),
                TupleEnd,
                Struct {
                    name: "ComplexKey",
                    len: 2,
                },
                Str("bar"),
                Str("d"),
                Str("baz"),
                I32(2),
                StructEnd,
                MapEnd,
            ],
        );

        serde_test::assert_tokens(
            &kvmap.compact(),
            &[
                Map {
                    len: Option::Some(2),
                },
                Tuple { len: 2 },
                NewtypeVariant {
                    name: "IpAddr",
                    variant: "V4",
                },
                Tuple { len: 4 },
                U8(127),
                U8(0),
                U8(0),
                U8(0),
                TupleEnd,
                U8(24),
                TupleEnd,
                Struct {
                    name: "ComplexKey",
                    len: 2,
                },
                Str("bar"),
                Str("b"),
                Str("baz"),
                I32(1),
                StructEnd,
                Tuple { len: 2 },
                NewtypeVariant {
                    name: "IpAddr",
                    variant: "V6",
                },
                Tuple { len: 16 },
                U8(0),
                U8(0),
                U8(0),
                U8(0),
                U8(0),
                U8(0),
                U8(0),
                U8(0),
                U8(0),
                U8(0),
                U8(0x77),
                U8(0x77),
                U8(0xde),
                U8(0xad),
                U8(0xbe),
                U8(0xef),
                TupleEnd,
                U8(48),
                TupleEnd,
                Struct {
                    name: "ComplexKey",
                    len: 2,
                },
                Str("bar"),
                Str("d"),
                Str("baz"),
                I32(2),
                StructEnd,
                MapEnd,
            ],
        );
    }
}

#[test]
fn test_kvmap_complex_key_yaml() {
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct ComplexKey {
        #[serde(rename = "$key$")]
        net: (IpAddr, u8),
        bar: String,
        baz: i32,
    }

    let kvmap = KVMap {
        foo: vec![
            ComplexKey {
                net: (IpAddr::from_str("127.0.0.0").unwrap(), 24),
                bar: "b".to_string(),
                baz: 1,
            },
            ComplexKey {
                net: (IpAddr::from_str("::7777:dead:beef").unwrap(), 48),
                bar: "d".to_string(),
                baz: 2,
            },
        ],
    };

    let yaml = serde_yaml::to_string(&kvmap).unwrap();
    expect_test::expect![[r#"
        ? - 127.0.0.0
          - 24
        : bar: b
          baz: 1
        ? - ::7777:dead:beef
          - 48
        : bar: d
          baz: 2
    "#]]
    .assert_eq(&yaml);
    let deser_values = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(kvmap, deser_values);
}
