use crate::prelude::*;
use pretty_assertions::assert_eq;

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct SimpleStruct {
    foo: i32,
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
#[schemars(rename = "new-name")]
struct RenamedSimpleStruct {
    foo: i32,
}

#[test]
fn simple() {
    test!(SimpleStruct)
        .custom(|schema, _| assert_eq!(schema.get("title"), Some(&"SimpleStruct".into())));

    test!(RenamedSimpleStruct)
        .custom(|schema, _| assert_eq!(schema.get("title"), Some(&"new-name".into())));
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct TypeParams<T, U, V, W> {
    t: T,
    u: U,
    v: V,
    w: W,
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
#[schemars(rename = "new-name-{W}-{T}-{T}")]
struct RenamedTypeParams<T, U, V, W> {
    t: T,
    u: U,
    v: V,
    w: W,
}

#[test]
fn type_params() {
    test!(TypeParams<u8, String, bool, ()>).custom(|schema, _| {
        assert_eq!(
            schema.get("title"),
            Some(&"TypeParams_for_uint8_and_string_and_boolean_and_null".into())
        )
    });

    test!(RenamedTypeParams<u8, String, bool, ()>).custom(|schema, _| {
        assert_eq!(
            schema.get("title"),
            Some(&"new-name-null-uint8-uint8".into())
        )
    });
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct ConstGeneric<const INT: usize, const CHAR: char> {
    #[schemars(range(max = INT))]
    foo: i32,
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
#[schemars(rename = "new-name-{INT}")]
struct RenamedConstGeneric<const INT: usize, const CHAR: char> {
    #[schemars(range(max = INT))]
    foo: i32,
}

#[test]
fn const_generics() {
    test!(ConstGeneric<123, 'X'>).custom(|schema, _| {
        assert_eq!(
            schema.get("title"),
            Some(&"ConstGeneric_for_123_and_X".into())
        )
    });

    test!(RenamedConstGeneric<123, 'X'>)
        .custom(|schema, _| assert_eq!(schema.get("title"), Some(&"new-name-123".into())));
}
