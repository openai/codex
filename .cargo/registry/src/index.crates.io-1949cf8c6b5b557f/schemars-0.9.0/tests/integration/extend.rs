#![allow(clippy::approx_constant)]

use crate::prelude::*;
use pretty_assertions::assert_eq;

static THREE: f64 = 3.0;

#[allow(dead_code)]
#[derive(JsonSchema)]
#[schemars(extend("obj" = {"array": [null, ()]}))]
#[schemars(extend("3" = THREE), extend("pi" = THREE + 0.14))]
struct Struct {
    #[schemars(extend("foo" = "bar"))]
    value: Value,
    #[schemars(extend("type" = ["number", "string"]))]
    int: i32,
}

#[test]
fn extend_struct() {
    test!(Struct).assert_snapshot().custom(|schema, _| {
        assert_eq!(schema.get("obj"), Some(&json!({ "array": [null, null] })));
        assert_eq!(schema.get("3"), Some(&json!(3.0)));
        assert_eq!(schema.get("pi"), Some(&json!(3.14)));
        assert_eq!(
            schema.as_value().pointer("/properties/value"),
            Some(&json!({ "foo": "bar" }))
        );
        assert_eq!(
            schema.as_value().pointer("/properties/int/type"),
            Some(&json!(["number", "string"]))
        );
    });
}

#[allow(dead_code)]
#[derive(JsonSchema)]
#[schemars(extend("obj" = {"array": [null, ()]}))]
#[schemars(extend("3" = THREE), extend("pi" = THREE + 0.14))]
struct TupleStruct(
    #[schemars(extend("foo" = "bar"))] Value,
    #[schemars(extend("type" = ["number", "string"]))] usize,
);

#[test]
fn extend_tuple_struct() {
    test!(TupleStruct).assert_snapshot().custom(|schema, _| {
        assert_eq!(schema.get("obj"), Some(&json!({ "array": [null, null] })));
        assert_eq!(schema.get("3"), Some(&json!(3.0)));
        assert_eq!(schema.get("pi"), Some(&json!(3.14)));
        assert_eq!(
            schema.as_value().pointer("/prefixItems/0"),
            Some(&json!({ "foo": "bar" }))
        );
        assert_eq!(
            schema.as_value().pointer("/prefixItems/1/type"),
            Some(&json!(["number", "string"]))
        );
    });
}

#[allow(dead_code)]
#[derive(JsonSchema)]
#[schemars(extend("foo" = "bar"))]
enum ExternalEnum {
    #[schemars(extend("foo" = "bar"))]
    Unit,
    #[schemars(extend("foo" = "bar"))]
    NewType(Value),
    #[schemars(extend("foo" = "bar"))]
    Tuple(i32, bool),
    #[schemars(extend("foo" = "bar"))]
    Struct { i: i32, b: bool },
}

#[test]
fn extend_externally_tagged_enum() {
    test!(ExternalEnum).assert_snapshot().custom(|schema, _| {
        assert_eq!(schema.get("foo"), Some(&json!("bar")));

        for i in 0..4 {
            assert_eq!(
                schema.as_value().pointer(&format!("/oneOf/{i}/foo")),
                Some(&json!("bar"))
            );
        }
    });
}

#[allow(dead_code)]
#[derive(JsonSchema)]
#[schemars(tag = "t", extend("foo" = "bar"))]
enum InternalEnum {
    #[schemars(extend("foo" = "bar"))]
    Unit,
    #[schemars(extend("foo" = "bar"))]
    NewType(Value),
    #[schemars(extend("foo" = "bar"))]
    Struct { i: i32, b: bool },
}

#[test]
fn extend_internally_tagged_enum() {
    test!(InternalEnum).assert_snapshot().custom(|schema, _| {
        assert_eq!(schema.get("foo"), Some(&json!("bar")));

        for i in 0..3 {
            assert_eq!(
                schema.as_value().pointer(&format!("/oneOf/{i}/foo")),
                Some(&json!("bar"))
            );
        }
    });
}

#[allow(dead_code)]
#[derive(JsonSchema)]
#[schemars(tag = "t", content = "c", extend("foo" = "bar"))]
enum AdjacentEnum {
    #[schemars(extend("foo" = "bar"))]
    Unit,
    #[schemars(extend("foo" = "bar"))]
    NewType(Value),
    #[schemars(extend("foo" = "bar"))]
    Tuple(i32, bool),
    #[schemars(extend("foo" = "bar"))]
    Struct { i: i32, b: bool },
}

#[test]
fn extend_adjacently_tagged_enum() {
    test!(AdjacentEnum).assert_snapshot().custom(|schema, _| {
        assert_eq!(schema.get("foo"), Some(&json!("bar")));

        for i in 0..4 {
            assert_eq!(
                schema.as_value().pointer(&format!("/oneOf/{i}/foo")),
                Some(&json!("bar"))
            );
        }
    });
}

#[allow(dead_code)]
#[derive(JsonSchema)]
#[schemars(untagged, extend("foo" = "bar"))]
enum UntaggedEnum {
    #[schemars(extend("foo" = "bar"))]
    Unit,
    #[schemars(extend("foo" = "bar"))]
    NewType(Value),
    #[schemars(extend("foo" = "bar"))]
    Tuple(i32, bool),
    #[schemars(extend("foo" = "bar"))]
    Struct { i: i32, b: bool },
}

#[test]
fn extend_untagged_enum() {
    test!(UntaggedEnum).assert_snapshot().custom(|schema, _| {
        assert_eq!(schema.get("foo"), Some(&json!("bar")));

        for i in 0..4 {
            assert_eq!(
                schema.as_value().pointer(&format!("/anyOf/{i}/foo")),
                Some(&json!("bar"))
            );
        }
    });
}
