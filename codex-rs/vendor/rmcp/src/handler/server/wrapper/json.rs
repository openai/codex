use std::borrow::Cow;

use schemars::JsonSchema;
use serde::Serialize;

use crate::{
    handler::server::tool::IntoCallToolResult,
    model::{CallToolResult, IntoContents},
};

/// Json wrapper for structured output
///
/// When used with tools, this wrapper indicates that the value should be
/// serialized as structured JSON content with an associated schema.
/// The framework will place the JSON in the `structured_content` field
/// of the tool result rather than the regular `content` field.
pub struct Json<T>(pub T);

// Implement JsonSchema for Json<T> to delegate to T's schema
impl<T: JsonSchema> JsonSchema for Json<T> {
    fn schema_name() -> Cow<'static, str> {
        T::schema_name()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        T::json_schema(generator)
    }
}

// Implementation for Json<T> to create structured content
impl<T: Serialize + JsonSchema + 'static> IntoCallToolResult for Json<T> {
    fn into_call_tool_result(self) -> Result<CallToolResult, crate::ErrorData> {
        let value = serde_json::to_value(self.0).map_err(|e| {
            crate::ErrorData::internal_error(
                format!("Failed to serialize structured content: {}", e),
                None,
            )
        })?;

        Ok(CallToolResult::structured(value))
    }
}

// Implementation for Result<Json<T>, E>
impl<T: Serialize + JsonSchema + 'static, E: IntoContents> IntoCallToolResult
    for Result<Json<T>, E>
{
    fn into_call_tool_result(self) -> Result<CallToolResult, crate::ErrorData> {
        match self {
            Ok(value) => value.into_call_tool_result(),
            Err(error) => Ok(CallToolResult::error(error.into_contents())),
        }
    }
}
