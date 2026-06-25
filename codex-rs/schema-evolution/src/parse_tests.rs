use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn parses_methods_into_typed_argument_shapes() -> Result<()> {
    let schema = json!({
        "oneOf": [
            request("map", json!({ "properties": { "value": { "type": "string" } }, "type": "object" }), /*required*/ true),
            request("value", json!({ "type": "null" }), /*required*/ false),
            {
                "properties": { "method": { "const": "none" } },
                "required": ["method"],
                "type": "object"
            }
        ]
    });
    let parsed = ApiSchema::parse(&schema)?;

    assert!(matches!(parsed.methods["map"].arguments, Arguments::Map(_)));
    assert!(matches!(
        parsed.methods["value"].arguments,
        Arguments::Value(Argument {
            required: false,
            ..
        })
    ));
    assert_eq!(parsed.methods["none"].arguments, Arguments::None);
    let (_, request) = parsed.resolve(parsed.methods["map"].request)?;
    let SchemaNode::Rules(request) = request else {
        panic!("method request should contain rules");
    };
    let request = request.object.as_ref().unwrap();
    assert!(request.properties.contains_key("id"));
    assert!(request.properties.contains_key("method"));
    assert!(request.properties.contains_key("params"));
    Ok(())
}

#[test]
fn keeps_refs_typed_without_expanding_recursive_definitions() -> Result<()> {
    let mut schema = request_schema(json!({ "$ref": "#/definitions/Params" }));
    schema["definitions"] = json!({
        "Params": {
            "properties": {
                "child": { "$ref": "#/definitions/Params" },
                "name": { "allOf": [{ "$ref": "#/definitions/Name" }], "description": "alias" }
            },
            "type": "object"
        },
        "Name": { "type": "string" }
    });

    let parsed = ApiSchema::parse(&schema)?;
    assert!(matches!(
        parsed.methods["test/method"].arguments,
        Arguments::Map(_)
    ));
    Ok(())
}

#[test]
fn rejects_malformed_protocol_shapes_and_direct_ref_cycles() {
    let duplicate = json!({
        "oneOf": [
            request("same", json!(null), /*required*/ true),
            request("same", json!(null), /*required*/ true)
        ]
    });
    assert!(ApiSchema::parse(&duplicate).is_err());

    let mut missing_ref = request_schema(json!({ "$ref": "#/definitions/Missing" }));
    missing_ref["definitions"] = json!({});
    assert!(ApiSchema::parse(&missing_ref).is_err());

    let mut cycle = request_schema(json!({ "$ref": "#/definitions/A" }));
    cycle["definitions"] = json!({
        "A": { "$ref": "#/definitions/B" },
        "B": { "$ref": "#/definitions/A" }
    });
    assert!(ApiSchema::parse(&cycle).is_err());

    let unsupported_all_of = request_schema(json!({
        "allOf": [{ "type": "string" }, { "minLength": 1 }]
    }));
    assert!(ApiSchema::parse(&unsupported_all_of).is_err());
}

fn request_schema(params: serde_json::Value) -> serde_json::Value {
    json!({ "oneOf": [request("test/method", params, /*required*/ true)] })
}

fn request(method: &str, params: serde_json::Value, required: bool) -> serde_json::Value {
    let mut required_fields = vec![json!("id"), json!("method")];
    if required {
        required_fields.push(json!("params"));
    }
    json!({
        "properties": {
            "id": { "type": "string" },
            "method": { "enum": [method] },
            "params": params
        },
        "required": required_fields,
        "type": "object"
    })
}
