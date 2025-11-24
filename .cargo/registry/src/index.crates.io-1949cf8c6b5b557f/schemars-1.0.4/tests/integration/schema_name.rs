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
#[schemars(rename = "new name~{W}/{T}/{T}!")]
struct RenamedTypeParams<T, U, V, W> {
    t: T,
    u: U,
    v: V,
    w: W,
}

#[test]
fn type_params() {
    test!(TypeParams<u8, String, bool, ()>)
        .assert_allows_ser_roundtrip_default()
        .assert_identical::<TypeParams<u8, Box<str>, bool, ()>>();

    assert_ne!(
        <TypeParams<u8, String, bool, ()>>::schema_id(),
        <TypeParams<(), u8, String, bool>>::schema_id()
    );

    test!(RenamedTypeParams<u8, String, bool, ()>)
        .assert_allows_ser_roundtrip_default()
        .assert_identical::<RenamedTypeParams<u8, Box<str>, bool, ()>>()
        .custom(|schema, _| {
            assert_eq!(
                schema.get("title"),
                Some(&"new name~null/uint8/uint8!".into())
            )
        });

    assert_ne!(
        <RenamedTypeParams<u8, String, bool, ()>>::schema_id(),
        <RenamedTypeParams<(), u8, String, bool>>::schema_id()
    );

    test!((
        TypeParams<u8, String, bool, ()>,
        RenamedTypeParams<u8, String, bool, ()>
    ))
    .assert_allows_ser_roundtrip_default()
    .custom(|schema, _| {
        assert_eq!(
            schema.pointer("/prefixItems/1/$ref"),
            Some(&("#/$defs/new%20name~0null~1uint8~1uint8!".into())),
            "name should be correctly encoded for $ref value"
        )
    });
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct ConstGeneric<const INT: usize, const CHAR: char> {
    #[schemars(range(max = INT))]
    foo: i32,
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
#[schemars(rename = "{{new-name-{INT}}}")]
struct RenamedConstGeneric<const INT: usize, const CHAR: char> {
    #[schemars(range(max = INT))]
    foo: i32,
}

#[test]
fn const_generics() {
    test!(ConstGeneric<123, 'X'>).assert_allows_ser_roundtrip_default();

    assert_ne!(
        <ConstGeneric<123, 'X'>>::schema_id(),
        <ConstGeneric<321, 'Y'>>::schema_id()
    );

    test!(RenamedConstGeneric<123, 'X'>)
        .assert_allows_ser_roundtrip_default()
        .custom(|schema, _| assert_eq!(schema.get("title"), Some(&"{new-name-123}".into())));

    assert_ne!(
        <RenamedConstGeneric<123, 'X'>>::schema_id(),
        <RenamedConstGeneric<321, 'Y'>>::schema_id()
    );
}
