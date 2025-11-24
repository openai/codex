//! Test Cases

use crate::utils::{check_matches_schema, check_valid_json_schema};
use expect_test::expect_file;
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::json;
use serde_with::*;
use std::collections::BTreeSet;

// This avoids us having to add `#[schemars(crate = "::schemars_0_8")]` all
// over the place. We're not testing that and it is inconvenient.
extern crate schemars_0_8 as schemars;

mod utils;

/// Declare a snapshot tests for a struct.
///
/// The snapshot files are stored under the `schemars_0_8` folder alongside
/// this test file.
macro_rules! declare_snapshot_test {
    {$(
        $( #[$tattr:meta] )*
        $test:ident {
            $( #[$stattr:meta] )*
            struct $name:ident {
                $(
                    $( #[ $fattr:meta ] )*
                    $field:ident : $ty:ty
                ),*
                $(,)?
            }
        }
    )*} => {$(
        #[test]
        $(#[$tattr])*
        fn $test() {
            #[serde_as]
            #[derive(JsonSchema, Serialize)]
            $( #[$stattr] )*
            struct $name {
                $(
                    $( #[$fattr] )*
                    $field: $ty,
                )*
            }

            let schema = schemars::schema_for!($name);
            let _ = jsonschema::Validator::new(
                &serde_json::to_value(&schema).expect("generated schema is not valid json"),
            )
            .expect("generated schema is not valid");

            let mut schema = serde_json::to_string_pretty(&schema)
                .expect("schema could not be serialized");
            schema.push('\n');

            let filename = concat!("./", module_path!(), "::", stringify!($test), ".json")
                .replace("::", "/")
                .replace("schemars_0_8/", "");

            let expected = expect_file![filename];
            expected.assert_eq(&schema);
        }
    )*}
}

#[test]
fn schemars_basic() {
    #[serde_as]
    #[derive(JsonSchema, Serialize)]
    #[schemars(crate = "::schemars_0_8")]
    struct Basic {
        /// Basic field, no attribute
        bare_field: u32,

        /// Field that directly uses `DisplayFromStr`
        #[serde_as(as = "DisplayFromStr")]
        display_from_str: u32,

        /// Same does not implement `JsonSchema` directly so this checks that the
        /// correct schemars attribute was injected.
        #[serde_as(as = "Same")]
        same: u32,

        /// This checks that Same still works when wrapped in a box.
        #[serde_as(as = "Box<Same>")]
        box_same: Box<u32>,

        /// Same thing, but with a Vec this time.
        #[serde_as(as = "Vec<_>")]
        vec_same: Vec<u32>,
    }

    let schema = schemars::schema_for!(Basic);
    let mut schema = serde_json::to_string_pretty(&schema).expect("schema could not be serialized");
    schema.push('\n');

    let expected = expect_file!["./schemars_basic.json"];
    expected.assert_eq(&schema);
}

#[test]
fn schemars_other_cfg_attrs() {
    #[serde_as]
    #[derive(JsonSchema, Serialize)]
    struct Test {
        #[serde_as(as = "DisplayFromStr")]
        #[cfg_attr(any(), arbitrary("some" |weird| syntax::<bool, 2>()))]
        #[cfg_attr(any(), schemars(with = "i32"))]
        custom: i32,
    }

    check_matches_schema::<Test>(&json!({
        "custom": "23",
    }));
}

#[test]
fn schemars_custom_with() {
    #[serde_as]
    #[derive(JsonSchema, Serialize)]
    struct Test {
        #[serde_as(as = "DisplayFromStr")]
        #[schemars(with = "i32")]
        custom: i32,

        #[serde_as(as = "DisplayFromStr")]
        #[cfg_attr(any(), schemars(with = "i32"))]
        with_disabled: i32,

        #[serde_as(as = "DisplayFromStr")]
        #[cfg_attr(all(), schemars(with = "i32"))]
        always_enabled: i32,
    }

    check_matches_schema::<Test>(&json!({
        "custom": 3,
        "with_disabled": "5",
        "always_enabled": 7,
    }));
}

#[test]
fn schemars_deserialize_only_bug_735() {
    #[serde_as]
    #[derive(JsonSchema, Serialize)]
    #[schemars(crate = "::schemars_0_8")]
    struct Basic {
        /// Basic field, no attribute
        bare_field: u32,

        /// Will emit matching schemars attribute
        #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
        both: u32,

        /// Can emit schemars with `serialize_as`, but it will be ignored
        #[serde_as(serialize_as = "PickFirst<(_, DisplayFromStr)>")]
        serialize_only: u32,

        /// schemars doesn't support `deserialize_as`
        #[serde_as(deserialize_as = "PickFirst<(_, DisplayFromStr)>")]
        deserialize_only: u32,

        /// Can emit schemars with `serialize_as`, but it will be ignored
        /// schemars doesn't support `deserialize_as`
        #[serde_as(
            serialize_as = "PickFirst<(_, DisplayFromStr)>",
            deserialize_as = "PickFirst<(_, DisplayFromStr)>"
        )]
        serialize_and_deserialize: u32,
    }

    let schema = schemars::schema_for!(Basic);
    let mut schema = serde_json::to_string_pretty(&schema).expect("schema could not be serialized");
    schema.push('\n');

    let expected = expect_file!["./schemars_deserialize_only_bug_735.json"];
    expected.assert_eq(&schema);
}

#[test]
fn schemars_custom_schema_with() {
    fn custom_int(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        use schemars::schema::*;

        SchemaObject {
            instance_type: Some(InstanceType::Integer.into()),
            ..Default::default()
        }
        .into()
    }

    #[serde_as]
    #[derive(JsonSchema, Serialize)]
    struct Test {
        #[serde_as(as = "DisplayFromStr")]
        #[schemars(schema_with = "custom_int")]
        custom: i32,

        #[serde_as(as = "DisplayFromStr")]
        #[cfg_attr(any(), schemars(schema_with = "custom_int"))]
        with_disabled: i32,

        #[serde_as(as = "DisplayFromStr")]
        #[cfg_attr(all(), schemars(schema_with = "custom_int"))]
        always_enabled: i32,
    }

    check_matches_schema::<Test>(&json!({
        "custom": 3,
        "with_disabled": "5",
        "always_enabled": 7,
    }));
}

mod test_std {
    use super::*;
    use std::collections::{BTreeMap, VecDeque};

    declare_snapshot_test! {
        option {
            struct Test {
                #[serde_with(as = "Option<_>")]
                optional: Option<i32>,
            }
        }

        vec {
            struct Test {
                #[serde_with(as = "Vec<_>")]
                vec: Vec<String>
            }
        }

        vec_deque {
            struct Test {
                #[serde_with(as = "VecDeque<_>")]
                vec_deque: VecDeque<String>
            }
        }

        map {
            struct Test {
                #[serde_with(as = "BTreeMap<_, _>")]
                map: BTreeMap<String, i32>
            }
        }

        set {
            struct Test {
                #[serde_with(as = "BTreeSet<_>")]
                map: BTreeSet<String>,
            }
        }

        tuples {
            struct Test {
                #[serde_with(as = "()")]
                tuple0: (),

                #[serde_with(as = "(_ ,)")]
                tuple1: (i32,),

                #[serde_with(as = "(_, _)")]
                tuple2: (i32, i32),

                #[serde_with(as = "(_, _, _)")]
                tuple3: (i32, i32, String)
            }
        }
    }
}

mod snapshots {
    use super::*;
    use serde_with::formats::*;

    #[allow(dead_code)]
    #[derive(JsonSchema, Serialize)]
    enum Mappable {
        A(i32),
        B(String),
        C { c: i32, b: Option<u64> },
    }

    #[derive(JsonSchema, Serialize)]
    struct KvMapData {
        #[serde(rename = "$key$")]
        key: String,

        a: u32,
        b: String,
        c: f32,
        d: bool,
    }

    #[allow(dead_code, variant_size_differences)]
    #[derive(JsonSchema, Serialize)]
    #[serde(tag = "$key$")]
    enum KvMapEnum {
        TypeA { a: u32 },
        TypeB { b: String },
        TypeC { c: bool },
    }

    #[derive(JsonSchema, Serialize)]
    struct KvMapFlatten {
        #[serde(flatten)]
        data: KvMapEnum,
        extra: bool,
    }

    declare_snapshot_test! {
        bytes {
            struct Test {
                #[serde_as(as = "Bytes")]
                bytes: Vec<u8>,
            }
        }

        default_on_null {
            struct Test {
                #[serde_as(as = "DefaultOnNull<_>")]
                data: String,
            }
        }

        string_with_separator {
            struct Test {
                #[serde_as(as = "StringWithSeparator<CommaSeparator, String>")]
                data: Vec<String>,
            }
        }

        from_into {
            struct Test {
                #[serde_as(as = "FromInto<u64>")]
                data: u32,
            }
        }

        map {
            struct Test {
                #[serde_as(as = "Map<_, _>")]
                data: Vec<(String, u32)>,
            }
        }

        map_fixed {
            struct Test {
                #[serde_as(as = "Map<_, _>")]
                data: [(String, u32); 4],
            }
        }

        set_last_value_wins {
            struct Test {
                #[serde_as(as = "SetLastValueWins<_>")]
                data: BTreeSet<u32>,
            }
        }

        set_prevent_duplicates {
            struct Test {
                #[serde_as(as = "SetPreventDuplicates<_>")]
                data: BTreeSet<u32>,
            }
        }

        duration {
            struct Test {
                #[serde_as(as = "DurationSeconds<u64, Flexible>")]
                seconds: std::time::Duration,

                #[serde_as(as = "DurationSecondsWithFrac<f64, Flexible>")]
                frac: std::time::Duration,

                #[serde_as(as = "DurationSeconds<String, Flexible>")]
                flexible_string: std::time::Duration,

                #[serde_as(as = "DurationSeconds<u64, Strict>")]
                seconds_u64_strict: std::time::Duration,

                #[serde_as(as = "TimestampSeconds<i64, Flexible>")]
                time_i64: std::time::SystemTime,
            }
        }

        enum_map {
            struct Test {
                #[serde_as(as = "EnumMap")]
                data: Vec<Mappable>,
            }
        }

        key_value_map {
            struct Test {
                #[serde_as(as = "KeyValueMap<_>")]
                data: Vec<KvMapData>,
            }
        }

        key_value_map_enum {
            struct Test {
                #[serde_as(as = "KeyValueMap<_>")]
                data: Vec<KvMapEnum>,
            }
        }

        key_value_map_flatten {
            struct Test {
                #[serde_as(as = "KeyValueMap<_>")]
                data: Vec<KvMapFlatten>,
            }
        }

        one_or_many_prefer_one {
            #[serde(transparent)]
            struct Test {
                #[serde_as(as = "OneOrMany<_, PreferOne>")]
                data: Vec<i32>,
            }
        }

        pickfirst {
            #[serde(transparent)]
            struct Test {
                #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
                value: u32
            }
        }

        pickfirst_nested {
            #[serde(transparent)]
            struct Test {
                #[serde_as(as = "OneOrMany<PickFirst<(_, DisplayFromStr)>>")]
                optional_value: Vec<u32>
            }
        }

        one_or_many_nested {
            struct Test {
                #[serde_as(as = "Option<OneOrMany<_>>")]
                optional_many: Option<Vec<String>>,
            }
        }
    }
}

mod derive {
    use super::*;

    #[serde_as]
    #[derive(Serialize)]
    #[cfg_attr(all(), derive(JsonSchema))]
    struct Enabled {
        #[serde_as(as = "DisplayFromStr")]
        field: u32,
    }

    #[allow(dead_code)]
    #[serde_as]
    #[derive(Serialize)]
    #[cfg_attr(any(), derive(JsonSchema))]
    struct Disabled {
        // If we are incorrectly adding `#[schemars(with = ...)]` attributes
        // then we should get an error on this field.
        #[serde_as(as = "DisplayFromStr")]
        field: u32,
    }

    #[test]
    fn test_enabled_has_correct_schema() {
        check_valid_json_schema(&Enabled { field: 77 });
    }
}

mod array {
    use super::*;

    #[serde_as]
    #[derive(JsonSchema, Serialize)]
    struct FixedArray {
        #[serde_as(as = "[_; 3]")]
        array: [u32; 3],
    }

    #[test]
    fn test_serialized_is_valid() {
        let array = FixedArray { array: [1, 2, 3] };

        check_valid_json_schema(&array);
    }

    #[test]
    fn test_valid_json() {
        let value = json!({ "array": [1, 2, 3] });
        check_matches_schema::<FixedArray>(&value);
    }

    #[test]
    #[should_panic]
    fn test_too_short() {
        check_matches_schema::<FixedArray>(&json!({
            "array": [1],
        }));
    }

    #[test]
    #[should_panic]
    fn test_too_long() {
        check_matches_schema::<FixedArray>(&json!({
            "array": [1, 2, 3, 4]
        }));
    }

    #[test]
    #[should_panic]
    fn test_wrong_item_type() {
        check_matches_schema::<FixedArray>(&json!({
            "array": ["1", "2", "3"]
        }));
    }

    #[test]
    #[should_panic]
    fn test_oob_item() {
        check_matches_schema::<FixedArray>(&json!({
            "array": [-1, 0x1_0000_0000i64, 32]
        }));
    }
}

mod bool_from_int {
    use super::*;
    use serde_with::formats::{Flexible, Strict};

    #[serde_as]
    #[derive(Serialize, JsonSchema)]
    struct BoolStrict {
        #[serde_as(as = "BoolFromInt<Strict>")]
        value: bool,
    }

    #[serde_as]
    #[derive(Serialize, JsonSchema)]
    struct BoolFlexible {
        #[serde_as(as = "BoolFromInt<Flexible>")]
        value: bool,
    }

    #[test]
    fn test_serialized_strict_is_valid() {
        check_valid_json_schema(&vec![
            BoolStrict { value: true },
            BoolStrict { value: false },
        ]);
    }

    #[test]
    fn test_serialized_flexible_is_valid() {
        check_valid_json_schema(&vec![
            BoolFlexible { value: true },
            BoolFlexible { value: false },
        ]);
    }

    #[test]
    #[should_panic]
    fn strict_out_of_range() {
        check_matches_schema::<BoolStrict>(&json!({
            "value": 5
        }));
    }

    #[test]
    fn flexible_out_of_range() {
        check_matches_schema::<BoolFlexible>(&json!({
            "value": 5
        }));
    }

    #[test]
    #[should_panic]
    fn flexible_wrong_type() {
        check_matches_schema::<BoolFlexible>(&json!({
            "value": "seven"
        }));
    }

    #[test]
    #[should_panic]
    fn test_fractional_value_strict() {
        check_matches_schema::<BoolStrict>(&json!({
            "value": 0.5
        }));
    }

    #[test]
    #[should_panic]
    fn test_fractional_value_flexible() {
        check_matches_schema::<BoolFlexible>(&json!({
            "value": 0.5
        }));
    }
}

mod bytes_or_string {
    use super::*;

    #[serde_as]
    #[derive(Serialize, JsonSchema)]
    struct Test {
        #[serde_as(as = "BytesOrString")]
        bytes: Vec<u8>,
    }

    #[test]
    fn test_serialized_is_valid() {
        check_valid_json_schema(&Test {
            bytes: b"test".to_vec(),
        });
    }

    #[test]
    fn test_string_valid_json() {
        check_matches_schema::<Test>(&json!({
            "bytes": "test string"
        }));
    }

    #[test]
    fn test_bytes_valid_json() {
        check_matches_schema::<Test>(&json!({
            "bytes": [1, 2, 3, 4]
        }));
    }

    #[test]
    #[should_panic]
    fn test_int_not_valid_json() {
        check_matches_schema::<Test>(&json!({
            "bytes": 5
        }));
    }
}

mod enum_map {
    use super::*;

    #[derive(Serialize, JsonSchema)]
    struct InnerStruct {
        c: String,
        d: f64,
    }

    #[derive(Serialize, JsonSchema)]
    enum Inner {
        A(i32),
        B(String),
        C(InnerStruct),
    }

    #[serde_as]
    #[derive(Serialize, JsonSchema)]
    #[serde(transparent)]
    struct Outer(#[serde_as(as = "EnumMap")] Vec<Inner>);

    #[test]
    fn test_serialized_is_valid() {
        check_valid_json_schema(&Outer(vec![
            Inner::A(5),
            Inner::B("test".into()),
            Inner::C(InnerStruct {
                c: "c".into(),
                d: -34.0,
            }),
        ]));
    }

    #[test]
    fn test_matches_expected() {
        check_matches_schema::<Outer>(&json!({
            "A": 75,
            "B": "BBBBBB",
            "C": {
                "c": "inner C",
                "d": 777
            }
        }));
    }

    #[test]
    fn test_no_fields_required() {
        check_matches_schema::<Outer>(&json!({}));
    }

    #[test]
    #[should_panic]
    fn test_mixed_up_schemas() {
        check_matches_schema::<Outer>(&json!({
            "A": "b",
            "B": 5
        }));
    }

    #[test]
    #[should_panic]
    fn test_invalid_key() {
        check_matches_schema::<Outer>(&json!({
            "invalid": 4
        }));
    }
}

mod duration {
    use super::*;
    use serde_with::formats::{Flexible, Strict};
    use std::time::{Duration, SystemTime};

    #[serde_as]
    #[derive(Serialize, JsonSchema)]
    struct DurationTest {
        #[serde_as(as = "DurationSeconds<u64, Strict>")]
        strict_u64: Duration,

        #[serde_as(as = "DurationSeconds<String, Strict>")]
        strict_str: Duration,

        #[serde_as(as = "DurationSecondsWithFrac<f64, Strict>")]
        strict_f64: Duration,

        #[serde_as(as = "DurationSeconds<u64, Flexible>")]
        flexible_u64: Duration,

        #[serde_as(as = "DurationSeconds<f64, Flexible>")]
        flexible_f64: Duration,

        #[serde_as(as = "DurationSeconds<String, Flexible>")]
        flexible_str: Duration,
    }

    #[test]
    fn test_serialized_is_valid() {
        check_valid_json_schema(&DurationTest {
            strict_u64: Duration::from_millis(2500),
            strict_str: Duration::from_millis(2500),
            strict_f64: Duration::from_millis(2500),
            flexible_u64: Duration::from_millis(2500),
            flexible_f64: Duration::from_millis(2500),
            flexible_str: Duration::from_millis(2500),
        });
    }

    #[serde_as]
    #[derive(Serialize, JsonSchema)]
    struct FlexibleU64Duration(#[serde_as(as = "DurationSeconds<u64, Flexible>")] Duration);

    #[serde_as]
    #[derive(Serialize, JsonSchema)]
    struct FlexibleStringDuration(#[serde_as(as = "DurationSeconds<String, Flexible>")] Duration);

    #[serde_as]
    #[derive(Serialize, JsonSchema)]
    struct FlexibleTimestamp(#[serde_as(as = "TimestampSeconds<i64, Flexible>")] SystemTime);

    #[test]
    fn test_string_as_flexible_u64() {
        check_matches_schema::<FlexibleU64Duration>(&json!("32"));
    }

    #[test]
    fn test_integer_as_flexible_u64() {
        check_matches_schema::<FlexibleU64Duration>(&json!(16));
    }

    #[test]
    fn test_number_as_flexible_u64() {
        check_matches_schema::<FlexibleU64Duration>(&json!(54.1));
    }

    #[test]
    #[should_panic]
    fn test_negative_as_flexible_u64() {
        check_matches_schema::<FlexibleU64Duration>(&json!(-5));
    }

    #[test]
    fn test_string_as_flexible_string() {
        check_matches_schema::<FlexibleStringDuration>(&json!("32"));
    }

    #[test]
    fn test_integer_as_flexible_string() {
        check_matches_schema::<FlexibleStringDuration>(&json!(16));
    }

    #[test]
    fn test_number_as_flexible_string() {
        check_matches_schema::<FlexibleStringDuration>(&json!(54.1));
    }

    #[test]
    #[should_panic]
    fn test_negative_as_flexible_string() {
        check_matches_schema::<FlexibleStringDuration>(&json!(-5));
    }

    #[test]
    fn test_negative_as_flexible_timestamp() {
        check_matches_schema::<FlexibleTimestamp>(&json!(-50000));
    }

    #[test]
    fn test_negative_string_as_flexible_timestamp() {
        check_matches_schema::<FlexibleTimestamp>(&json!("-50000"));
    }
}

#[test]
fn test_borrow_cow() {
    use std::borrow::Cow;

    #[serde_as]
    #[derive(Serialize, JsonSchema)]
    struct Borrowed<'a> {
        #[serde_as(as = "BorrowCow")]
        data: Cow<'a, str>,
    }

    check_valid_json_schema(&Borrowed {
        data: Cow::Borrowed("test"),
    });
}

#[test]
fn test_map() {
    #[serde_as]
    #[derive(Serialize, JsonSchema)]
    struct Test {
        map: [(&'static str, u32); 2],
    }

    check_valid_json_schema(&Test {
        map: [("a", 1), ("b", 2)],
    });
}

#[test]
fn test_if_is_human_readable() {
    #[serde_as]
    #[derive(Serialize, JsonSchema)]
    struct Test {
        #[serde_as(as = "IfIsHumanReadable<DisplayFromStr>")]
        data: i32,
    }

    check_valid_json_schema(&Test { data: 5 });
    check_matches_schema::<Test>(&json!({ "data": "5" }));
}

#[test]
fn test_set_last_value_wins_with_duplicates() {
    #[serde_as]
    #[derive(Serialize, JsonSchema)]
    struct Test {
        #[serde_as(as = "SetLastValueWins<_>")]
        set: BTreeSet<u32>,
    }

    check_matches_schema::<Test>(&json!({
        "set": [ 1, 2, 3, 1, 4, 2 ]
    }));
}

#[test]
#[should_panic]
fn test_set_prevent_duplicates_with_duplicates() {
    #[serde_as]
    #[derive(Serialize, JsonSchema)]
    struct Test {
        #[serde_as(as = "SetPreventDuplicates<_>")]
        set: BTreeSet<u32>,
    }

    check_matches_schema::<Test>(&json!({
        "set": [ 1, 1 ]
    }));
}

mod key_value_map {
    use super::*;
    use schemars::schema::ObjectValidation;
    use std::{collections::BTreeMap, vec};

    #[serde_as]
    #[derive(Clone, Debug, JsonSchema, Serialize)]
    #[serde(transparent)]
    struct KVMap<E>(
        #[serde_as(as = "KeyValueMap<_>")]
        #[serde(bound(serialize = "E: Serialize", deserialize = "E: Deserialize<'de>"))]
        Vec<E>,
    );

    #[derive(Clone, Debug, JsonSchema, Serialize)]
    #[serde(untagged)]
    enum UntaggedEnum {
        A {
            #[serde(rename = "$key$")]
            key: String,
            field1: String,
        },
        B(String, i32),
    }

    #[test]
    fn test_untagged_enum() {
        let value = KVMap(vec![
            UntaggedEnum::A {
                key: "v1".into(),
                field1: "field".into(),
            },
            UntaggedEnum::B("v2".into(), 7),
        ]);

        check_valid_json_schema(&value);
    }

    #[derive(Clone, Debug, JsonSchema, Serialize)]
    #[serde(untagged)]
    enum UntaggedNestedEnum {
        Nested(UntaggedEnum),
        C {
            #[serde(rename = "$key$")]
            key: String,
            field2: i32,
        },
    }

    #[derive(Clone, Debug, Serialize)]
    #[serde(transparent)]
    struct LimitedProperties(serde_json::Value);

    impl JsonSchema for LimitedProperties {
        fn schema_name() -> String {
            "LimitedProperties".into()
        }

        fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
            schemars::schema::Schema::Object(schemars::schema::SchemaObject {
                instance_type: Some(schemars::schema::InstanceType::Object.into()),
                object: Some(Box::new(ObjectValidation {
                    max_properties: Some(5),
                    min_properties: Some(1),
                    additional_properties: Some(Box::new(schemars::schema::Schema::Object(
                        schemars::schema::SchemaObject {
                            instance_type: Some(schemars::schema::InstanceType::String.into()),
                            ..Default::default()
                        },
                    ))),
                    ..Default::default()
                })),

                ..Default::default()
            })
        }
    }

    #[test]
    fn test_untagged_nested_enum() {
        let value = KVMap(vec![
            UntaggedNestedEnum::Nested(UntaggedEnum::A {
                key: "v1".into(),
                field1: "field".into(),
            }),
            UntaggedNestedEnum::Nested(UntaggedEnum::B("v2".into(), 7)),
            UntaggedNestedEnum::C {
                key: "v2".into(),
                field2: 222,
            },
        ]);

        check_valid_json_schema(&value);
    }

    #[test]
    fn test_btreemap() {
        let value = KVMap(vec![
            BTreeMap::from_iter([("$key$", "a"), ("value", "b")]),
            BTreeMap::from_iter([("$key$", "b"), ("value", "d")]),
        ]);

        check_valid_json_schema(&value);
    }

    #[test]
    fn test_num_properties() {
        let value = KVMap(vec![
            LimitedProperties(serde_json::json!({
                "$key$": "A",
                "a": "b",
                "c": "d",
                "e": "f",
                "g": "h",
            })),
            LimitedProperties(serde_json::json!({
                "$key$": "B",
                "a": "b",
            })),
        ]);

        check_valid_json_schema(&value);
    }
}

mod one_or_many {
    use super::*;
    use serde_with::formats::{PreferMany, PreferOne};

    #[serde_as]
    #[derive(Clone, Debug, JsonSchema, Serialize)]
    #[serde(transparent)]
    struct WithPreferOne(#[serde_as(as = "OneOrMany<_, PreferOne>")] Vec<i32>);

    #[serde_as]
    #[derive(Clone, Debug, JsonSchema, Serialize)]
    #[serde(transparent)]
    struct WithPreferMany(#[serde_as(as = "OneOrMany<_, PreferMany>")] Vec<i32>);

    #[test]
    fn test_prefer_one() {
        let single = WithPreferOne(vec![7]);
        let multiple = WithPreferOne(vec![1, 2, 3]);

        check_valid_json_schema(&single);
        check_valid_json_schema(&multiple);
    }

    #[test]
    fn test_prefer_one_matches() {
        check_matches_schema::<WithPreferOne>(&json!(7));
        check_matches_schema::<WithPreferOne>(&json!([1, 2, 3]));
    }

    #[test]
    #[should_panic]
    fn test_prefer_one_no_invalid_type_one() {
        check_matches_schema::<WithPreferOne>(&json!("test"));
    }

    #[test]
    #[should_panic]
    fn test_prefer_one_no_invalid_type_many() {
        check_matches_schema::<WithPreferOne>(&json!(["test", 1]));
    }

    #[test]
    fn test_prefer_many() {
        let single = WithPreferMany(vec![7]);
        let multiple = WithPreferMany(vec![1, 2, 3]);

        check_valid_json_schema(&single);
        check_valid_json_schema(&multiple);
    }

    #[test]
    #[should_panic]
    fn test_prefer_many_no_invalid_type_one() {
        check_matches_schema::<WithPreferMany>(&json!("test"));
    }

    #[test]
    #[should_panic]
    fn test_prefer_many_no_invalid_type_many() {
        check_matches_schema::<WithPreferMany>(&json!(["test", 1]));
    }
}

#[test]
fn test_pickfirst() {
    #[serde_as]
    #[derive(JsonSchema, Serialize)]
    #[serde(transparent)]
    struct IntOrDisplay(#[serde_as(as = "PickFirst<(_, DisplayFromStr)>")] u32);

    check_matches_schema::<IntOrDisplay>(&json!(7));
    check_matches_schema::<IntOrDisplay>(&json!("17"));
}
