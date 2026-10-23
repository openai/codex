use super::*;
use crate::JsonSchema;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn catalog_overrides_preserve_tool_fields_and_encryption_with_independent_fallbacks() {
    let bundled = ResponsesApiTool {
        name: "example".to_string(),
        description: "Bundled guidance.".to_string(),
        strict: true,
        defer_loading: Some(true),
        parameters: serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "text": {"type": "string", "encrypted": true},
                "title": {"type": "string", "encrypted": true},
                "path": {"type": "string"}
            }
        }))
        .unwrap(),
        output_schema: Some(json!({"type": "string"}).into()),
    };
    assert_eq!(
        with_catalog_tool_message(bundled.clone(), /*messages*/ None),
        bundled,
    );

    let mut catalog = json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Catalog guidance.", "encrypted": false},
            "title": {"type": "string"},
            "path": {"type": "string"}
        }
    });
    let parameters = catalog.to_string();
    catalog["properties"]["text"]["encrypted"] = json!(true);
    catalog["properties"]["title"]["encrypted"] = json!(true);
    let expected_parameters: JsonSchema = serde_json::from_value(catalog).unwrap();

    for (description, schema, expected_schema) in [
        (None, None, None),
        (Some(""), None, None),
        (None, Some(parameters.as_str()), Some(&expected_parameters)),
        (
            Some("  Catalog {{literal}} guidance.\n"),
            Some(parameters.as_str()),
            Some(&expected_parameters),
        ),
        (Some("Catalog guidance."), Some("invalid JSON"), None),
        (None, Some(r#"{"type":"string"}"#), None),
        (
            None,
            Some(r#"{"type":"object","properties":{"text":{"type":"unsupported"}}}"#),
            None,
        ),
        (
            Some(""),
            Some(r#"{"type":"object","properties":{"title":{"type":"string"}}}"#),
            None,
        ),
        (
            Some("Catalog guidance."),
            Some(r#"{"type":"object","properties":{"text":{"type":"string"}}}"#),
            None,
        ),
    ] {
        let messages = ToolMessage {
            description: description.map(str::to_owned),
            parameters: schema.map(str::to_owned),
        };
        let mut expected = bundled.clone();
        if let Some(description) = description {
            expected.description = description.to_owned();
        }
        if let Some(parameters) = expected_schema {
            expected.parameters = parameters.clone();
        }
        assert_eq!(
            with_catalog_tool_message(bundled.clone(), Some(&messages)),
            expected,
        );
    }
}
