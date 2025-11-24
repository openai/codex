use super::*;
use core::{fmt::Write as _, str::FromStr};
use serde_test::Configure;
use serde_with::EnumMap;
use std::net::IpAddr;

fn bytes_debug_readable(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        match byte {
            non_printable if !(0x20..0x7f).contains(&non_printable) => {
                result.write_fmt(format_args!("\\x{byte:02x}")).unwrap();
            }
            b'\\' => result.push_str("\\\\"),
            _ => {
                result.push(char::from(byte));
            }
        }
    }
    result
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum EnumValue {
    Int(i32),
    String(String),
    Unit,
    Tuple(i32, String, bool),
    Struct {
        a: i32,
        b: String,
        c: bool,
    },
    Ip(IpAddr, IpAddr),
    #[serde(rename = "$value")]
    Extra(serde_json::Value),
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct VecEnumValues {
    #[serde_as(as = "EnumMap")]
    vec: Vec<EnumValue>,
}

#[test]
fn json_round_trip() {
    let values = VecEnumValues {
        vec: vec![
            EnumValue::Int(123),
            EnumValue::String("FooBar".to_string()),
            EnumValue::Int(456),
            EnumValue::String("XXX".to_string()),
            EnumValue::Unit,
            EnumValue::Tuple(1, "Middle".to_string(), false),
            EnumValue::Struct {
                a: 666,
                b: "BBB".to_string(),
                c: true,
            },
        ],
    };

    let json = serde_json::to_string_pretty(&values).unwrap();
    expect_test::expect![[r#"
            {
              "vec": {
                "Int": 123,
                "String": "FooBar",
                "Int": 456,
                "String": "XXX",
                "Unit": null,
                "Tuple": [
                  1,
                  "Middle",
                  false
                ],
                "Struct": {
                  "a": 666,
                  "b": "BBB",
                  "c": true
                }
              }
            }"#]]
    .assert_eq(&json);
    let deser_values: VecEnumValues = serde_json::from_str(&json).unwrap();
    assert_eq!(values, deser_values);
}

#[test]
fn ron_serialize() {
    let values = VecEnumValues {
        vec: vec![
            EnumValue::Int(123),
            EnumValue::String("FooBar".to_string()),
            EnumValue::Int(456),
            EnumValue::String("XXX".to_string()),
            EnumValue::Unit,
            EnumValue::Tuple(1, "Middle".to_string(), false),
            EnumValue::Struct {
                a: 666,
                b: "BBB".to_string(),
                c: true,
            },
        ],
    };

    let pretty_config = ron::ser::PrettyConfig::new().new_line("\n");
    let ron = ron::ser::to_string_pretty(&values, pretty_config).unwrap();
    expect_test::expect![[r#"
            (
                vec: {
                    "Int": 123,
                    "String": "FooBar",
                    "Int": 456,
                    "String": "XXX",
                    "Unit": (),
                    "Tuple": (1, "Middle", false),
                    "Struct": (
                        a: 666,
                        b: "BBB",
                        c: true,
                    ),
                },
            )"#]]
    .assert_eq(&ron);
    // TODO deserializing a Strings as an Identifier seems unsupported
    let deser_values: ron::Value = ron::de::from_str(&ron).unwrap();
    expect_test::expect![[r#"
        Map(
            Map(
                {
                    String(
                        "vec",
                    ): Map(
                        Map(
                            {
                                String(
                                    "Int",
                                ): Number(
                                    U16(
                                        456,
                                    ),
                                ),
                                String(
                                    "String",
                                ): String(
                                    "XXX",
                                ),
                                String(
                                    "Struct",
                                ): Map(
                                    Map(
                                        {
                                            String(
                                                "a",
                                            ): Number(
                                                U16(
                                                    666,
                                                ),
                                            ),
                                            String(
                                                "b",
                                            ): String(
                                                "BBB",
                                            ),
                                            String(
                                                "c",
                                            ): Bool(
                                                true,
                                            ),
                                        },
                                    ),
                                ),
                                String(
                                    "Tuple",
                                ): Seq(
                                    [
                                        Number(
                                            U8(
                                                1,
                                            ),
                                        ),
                                        String(
                                            "Middle",
                                        ),
                                        Bool(
                                            false,
                                        ),
                                    ],
                                ),
                                String(
                                    "Unit",
                                ): Unit,
                            },
                        ),
                    ),
                },
            ),
        )
    "#]]
    .assert_debug_eq(&deser_values);
}

#[test]
fn xml_round_trip() {
    let values = VecEnumValues {
        vec: vec![
            EnumValue::Int(123),
            EnumValue::String("FooBar".to_string()),
            EnumValue::Int(456),
            EnumValue::String("XXX".to_string()),
            EnumValue::Unit,
            // serialize_tuple and variants are not supported by XML
            // EnumValue::Tuple(1, "Middle".to_string(), false),
            // Cannot be deserialized. It serializes to:
            // <Struct><EnumValue><a>666</a><b>BBB</b><c>true</c></EnumValue></Struct>
            // EnumValue::Struct {
            //     a: 666,
            //     b: "BBB".to_string(),
            //     c: true,
            // },
        ],
    };

    let xml = serde_xml_rs::to_string(&values).unwrap();
    expect_test::expect![[r#"<?xml version="1.0" encoding="utf-8"?><VecEnumValues><vec><Int>123</Int><String>FooBar</String><Int>456</Int><String>XXX</String><Unit /></vec></VecEnumValues>"#]]
        .assert_eq(&xml);
    let deser_values: VecEnumValues = serde_xml_rs::from_str(&xml).unwrap();
    assert_eq!(values, deser_values);
}

#[test]
fn serde_test_round_trip() {
    let values = VecEnumValues {
        vec: vec![
            EnumValue::Int(123),
            EnumValue::String("FooBar".to_string()),
            EnumValue::Int(456),
            EnumValue::String("XXX".to_string()),
            EnumValue::Unit,
            EnumValue::Tuple(1, "Middle".to_string(), false),
            EnumValue::Struct {
                a: 666,
                b: "BBB".to_string(),
                c: true,
            },
        ],
    };

    use serde_test::Token::*;
    serde_test::assert_tokens(
        &values.readable(),
        &[
            Struct {
                name: "VecEnumValues",
                len: 1,
            },
            Str("vec"),
            Map {
                len: Option::Some(7),
            },
            Str("Int"),
            I32(123),
            Str("String"),
            Str("FooBar"),
            Str("Int"),
            I32(456),
            Str("String"),
            Str("XXX"),
            Str("Unit"),
            Unit,
            Str("Tuple"),
            TupleStruct {
                name: "EnumValue",
                len: 3,
            },
            I32(1),
            Str("Middle"),
            Bool(false),
            TupleStructEnd,
            Str("Struct"),
            Struct {
                name: "EnumValue",
                len: 3,
            },
            Str("a"),
            I32(666),
            Str("b"),
            Str("BBB"),
            Str("c"),
            Bool(true),
            StructEnd,
            MapEnd,
            StructEnd,
        ],
    );
}

#[test]
fn serde_test_round_trip_human_readable() {
    let values = VecEnumValues {
        vec: vec![EnumValue::Ip(
            IpAddr::from_str("127.0.0.1").unwrap(),
            IpAddr::from_str("::7777:dead:beef").unwrap(),
        )],
    };

    use serde_test::Token::*;
    serde_test::assert_tokens(
        &values.clone().readable(),
        &[
            Struct {
                name: "VecEnumValues",
                len: 1,
            },
            Str("vec"),
            Map {
                len: Option::Some(1),
            },
            Str("Ip"),
            TupleStruct {
                name: "EnumValue",
                len: 2,
            },
            Str("127.0.0.1"),
            Str("::7777:dead:beef"),
            TupleStructEnd,
            MapEnd,
            StructEnd,
        ],
    );

    serde_test::assert_tokens(
        &values.compact(),
        &[
            Struct {
                name: "VecEnumValues",
                len: 1,
            },
            Str("vec"),
            Map {
                len: Option::Some(1),
            },
            Str("Ip"),
            TupleStruct {
                name: "EnumValue",
                len: 2,
            },
            NewtypeVariant {
                name: "IpAddr",
                variant: "V4",
            },
            Tuple { len: 4 },
            U8(127),
            U8(0),
            U8(0),
            U8(1),
            TupleEnd,
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
            TupleStructEnd,
            MapEnd,
            StructEnd,
        ],
    );
}

// Bincode does not support Deserializer::deserialize_identifier
// https://github.com/bincode-org/bincode/blob/e0ac3245162ba668ba04591897dd88ff5b3096b8/src/de/mod.rs#L442

#[test]
fn rmp_round_trip() {
    let values = VecEnumValues {
        vec: vec![
            EnumValue::Int(123),
            EnumValue::String("FooBar".to_string()),
            EnumValue::Int(456),
            EnumValue::String("XXX".to_string()),
            EnumValue::Unit,
            EnumValue::Tuple(1, "Middle".to_string(), false),
            EnumValue::Struct {
                a: 666,
                b: "BBB".to_string(),
                c: true,
            },
            EnumValue::Ip(
                IpAddr::from_str("127.0.0.1").unwrap(),
                IpAddr::from_str("::7777:dead:beef").unwrap(),
            ),
        ],
    };

    let rmp = rmp_serde::to_vec(&values).unwrap();
    expect_test::expect![[r"\x91\x88\xa3Int{\xa6String\xa6FooBar\xa3Int\xcd\x01\xc8\xa6String\xa3XXX\xa4Unit\xc0\xa5Tuple\x93\x01\xa6Middle\xc2\xa6Struct\x93\xcd\x02\x9a\xa3BBB\xc3\xa2Ip\x92\x81\xa2V4\x94\x7f\x00\x00\x01\x81\xa2V6\xdc\x00\x10\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00ww\xcc\xde\xcc\xad\xcc\xbe\xcc\xef"]]
        .assert_eq(&bytes_debug_readable(&rmp));
    let deser_values: VecEnumValues = rmp_serde::from_read(&*rmp).unwrap();
    assert_eq!(values, deser_values);
}

#[test]
fn yaml_round_trip() {
    // Duplicate enum variants do not work with YAML
    let values = VecEnumValues {
        vec: vec![
            EnumValue::Int(123),
            EnumValue::String("FooBar".to_string()),
            // EnumValue::Int(456),
            // EnumValue::String("XXX".to_string()),
            EnumValue::Unit,
            EnumValue::Tuple(1, "Middle".to_string(), false),
            EnumValue::Struct {
                a: 666,
                b: "BBB".to_string(),
                c: true,
            },
        ],
    };

    let yaml = serde_yaml::to_string(&values).unwrap();
    expect_test::expect![[r#"
        vec:
          Int: 123
          String: FooBar
          Unit: null
          Tuple:
          - 1
          - Middle
          - false
          Struct:
            a: 666
            b: BBB
            c: true
    "#]]
    .assert_eq(&yaml);
    let deser_values: VecEnumValues = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(values, deser_values);
}
