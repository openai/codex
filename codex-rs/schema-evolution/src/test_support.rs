use crate::ApiSchema;
use crate::SchemaBreakage;
use crate::find_request_narrowing;
use anyhow::Result;
use serde_json::Value;
use serde_json::json;

pub(crate) fn request_schema(params: Value) -> Value {
    request_schema_for("test/method", params)
}

pub(crate) fn request_schema_for(method: &str, params: Value) -> Value {
    json!({
        "oneOf": [{
            "properties": {
                "id": { "type": ["string", "integer"] },
                "method": { "enum": [method], "type": "string" },
                "params": params
            },
            "required": ["id", "method", "params"],
            "type": "object"
        }]
    })
}

pub(crate) fn compare(base: &Value, current: &Value) -> Result<Vec<SchemaBreakage>> {
    let base = ApiSchema::parse(base)?;
    let current = ApiSchema::parse(current)?;
    find_request_narrowing(&base, &current)
}

pub(crate) fn breakage(
    kind: crate::ViolationKind,
    path: &str,
    before: Value,
    after: Value,
) -> SchemaBreakage {
    SchemaBreakage {
        kind,
        method: "test/method".to_string(),
        path: path.to_string(),
        before,
        after,
    }
}
