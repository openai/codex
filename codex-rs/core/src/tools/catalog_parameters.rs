//! Parses catalog parameter overrides using the harness's supported JSON Schema subset.
//! Rejected schemas leave fallback and transport-specific requirements to the tool owner.

use codex_tools::JsonSchema;
use serde_json::Value;

pub(super) fn parse(parameters: &str) -> Result<JsonSchema, &'static str> {
    let parameters: Value =
        serde_json::from_str(parameters).map_err(|_| "schema is not valid JSON")?;
    if !parameters.is_object() || parameters["type"] != "object" {
        return Err("schema must declare an object type");
    }
    serde_json::from_value(parameters).map_err(|_| "schema uses unsupported JSON Schema structures")
}
