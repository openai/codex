use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use serde_json::json;
use std::collections::BTreeMap;

/// Primitive JSON Schema type names we support in tool definitions.
///
/// This mirrors the OpenAI Structured Outputs "Supported types" subset:
/// string, number, boolean, integer, object, array, enum, and anyOf.
/// See <https://developers.openai.com/api/docs/guides/structured-outputs#supported-schemas>.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum JsonSchemaPrimitiveType {
    String,
    Number,
    Boolean,
    Integer,
    Object,
    Array,
    Null,
}

/// JSON Schema `type` supports either a single type name or a union of names.
///
/// OpenAI Structured Outputs allows `anyOf`, while the root schema must still
/// be an object. Nested unions can be represented either through `anyOf` or a
/// multi-valued `type`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum JsonSchemaType {
    Single(JsonSchemaPrimitiveType),
    Multiple(Vec<JsonSchemaPrimitiveType>),
}

/// Generic JSON-Schema subset needed for our tool definitions.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct JsonSchema {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub schema_type: Option<JsonSchemaType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<JsonValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<JsonSchema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<BTreeMap<String, JsonSchema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    #[serde(
        rename = "additionalProperties",
        skip_serializing_if = "Option::is_none"
    )]
    pub additional_properties: Option<AdditionalProperties>,
    #[serde(rename = "anyOf", skip_serializing_if = "Option::is_none")]
    pub any_of: Option<Vec<JsonSchema>>,
}

impl JsonSchema {
    /// Construct a scalar/object/array schema with a single JSON Schema type.
    fn typed(schema_type: JsonSchemaPrimitiveType, description: Option<String>) -> Self {
        Self {
            schema_type: Some(JsonSchemaType::Single(schema_type)),
            description,
            ..Default::default()
        }
    }

    pub fn any_of(variants: Vec<JsonSchema>, description: Option<String>) -> Self {
        Self {
            description,
            any_of: Some(variants),
            ..Default::default()
        }
    }

    pub fn boolean(description: Option<String>) -> Self {
        Self::typed(JsonSchemaPrimitiveType::Boolean, description)
    }

    pub fn string(description: Option<String>) -> Self {
        Self::typed(JsonSchemaPrimitiveType::String, description)
    }

    pub fn number(description: Option<String>) -> Self {
        Self::typed(JsonSchemaPrimitiveType::Number, description)
    }

    pub fn integer(description: Option<String>) -> Self {
        Self::typed(JsonSchemaPrimitiveType::Integer, description)
    }

    pub fn null(description: Option<String>) -> Self {
        Self::typed(JsonSchemaPrimitiveType::Null, description)
    }

    pub fn enumeration(values: Vec<JsonValue>, description: Option<String>) -> Self {
        Self {
            schema_type: infer_enum_type(Some(&values)).map(JsonSchemaType::Single),
            description,
            enum_values: Some(values),
            ..Default::default()
        }
    }

    pub fn string_enum(values: Vec<JsonValue>, description: Option<String>) -> Self {
        Self::enumeration(values, description)
    }

    pub fn array(items: JsonSchema, description: Option<String>) -> Self {
        Self {
            schema_type: Some(JsonSchemaType::Single(JsonSchemaPrimitiveType::Array)),
            description,
            items: Some(Box::new(items)),
            ..Default::default()
        }
    }

    pub fn object(
        properties: BTreeMap<String, JsonSchema>,
        required: Option<Vec<String>>,
        additional_properties: Option<AdditionalProperties>,
    ) -> Self {
        Self {
            schema_type: Some(JsonSchemaType::Single(JsonSchemaPrimitiveType::Object)),
            properties: Some(properties),
            required,
            additional_properties,
            ..Default::default()
        }
    }
}

/// Whether additional properties are allowed, and if so, any required schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum AdditionalProperties {
    Boolean(bool),
    Schema(Box<JsonSchema>),
}

impl From<bool> for AdditionalProperties {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

impl From<JsonSchema> for AdditionalProperties {
    fn from(value: JsonSchema) -> Self {
        Self::Schema(Box::new(value))
    }
}

/// Parse the tool `input_schema` or return an error for invalid schema.
pub fn parse_tool_input_schema(input_schema: &JsonValue) -> Result<JsonSchema, serde_json::Error> {
    let mut input_schema = input_schema.clone();
    sanitize_json_schema(&mut input_schema);
    serde_json::from_value(input_schema)
}

/// Sanitize a JSON Schema (as serde_json::Value) so it can fit our limited
/// schema representation. This function:
/// - Ensures every typed schema object has a `"type"` when required.
/// - Preserves explicit `anyOf`.
/// - Collapses `const` into single-value `enum`.
/// - Fills required child fields (e.g. array items, object properties) with
///   permissive defaults when absent.
fn sanitize_json_schema(value: &mut JsonValue) {
    match value {
        JsonValue::Bool(_) => {
            // JSON Schema boolean form: true/false. Coerce to an accept-all string.
            *value = json!({ "type": "string" });
        }
        JsonValue::Array(values) => {
            for value in values {
                sanitize_json_schema(value);
            }
        }
        JsonValue::Object(map) => {
            if let Some(properties) = map.get_mut("properties")
                && let Some(properties_map) = properties.as_object_mut()
            {
                for value in properties_map.values_mut() {
                    sanitize_json_schema(value);
                }
            }
            if let Some(items) = map.get_mut("items") {
                sanitize_json_schema(items);
            }
            if let Some(additional_properties) = map.get_mut("additionalProperties")
                && !matches!(additional_properties, JsonValue::Bool(_))
            {
                sanitize_json_schema(additional_properties);
            }
            if let Some(value) = map.get_mut("prefixItems") {
                sanitize_json_schema(value);
            }
            if let Some(value) = map.get_mut("anyOf") {
                sanitize_json_schema(value);
            }

            if let Some(const_value) = map.remove("const") {
                map.insert("enum".to_string(), JsonValue::Array(vec![const_value]));
                if matches!(
                    map.get("type").and_then(JsonValue::as_str),
                    Some("const") | None
                ) {
                    map.remove("type");
                }
            }

            normalize_type_field(map);

            let mut schema_type = map
                .get("type")
                .and_then(JsonValue::as_str)
                .map(str::to_string);

            if matches!(schema_type.as_deref(), Some("enum") | Some("const")) {
                schema_type = None;
                map.remove("type");
            }

            if let Some(types) = map.get("type").and_then(JsonValue::as_array).cloned() {
                ensure_default_children_for_type_union(map, &types);
                return;
            }

            if schema_type.is_none() && map.contains_key("anyOf") {
                return;
            }

            if schema_type.is_none() {
                if map.contains_key("properties")
                    || map.contains_key("required")
                    || map.contains_key("additionalProperties")
                {
                    schema_type = Some("object".to_string());
                } else if map.contains_key("items") || map.contains_key("prefixItems") {
                    schema_type = Some("array".to_string());
                } else if map.contains_key("enum") || map.contains_key("format") {
                    schema_type = infer_enum_type(map.get("enum").and_then(JsonValue::as_array))
                        .map(schema_type_name)
                        .map(str::to_string)
                        .or_else(|| Some("string".to_string()));
                } else if map.contains_key("minimum")
                    || map.contains_key("maximum")
                    || map.contains_key("exclusiveMinimum")
                    || map.contains_key("exclusiveMaximum")
                    || map.contains_key("multipleOf")
                {
                    schema_type = Some("number".to_string());
                } else {
                    schema_type = Some("string".to_string());
                }
            }

            let schema_type = schema_type.unwrap_or_else(|| "string".to_string());
            map.insert("type".to_string(), JsonValue::String(schema_type.clone()));

            if schema_type == "object" && !map.contains_key("properties") {
                map.insert(
                    "properties".to_string(),
                    JsonValue::Object(serde_json::Map::new()),
                );
            }

            if schema_type == "array" && !map.contains_key("items") {
                map.insert("items".to_string(), json!({ "type": "string" }));
            }
        }
        _ => {}
    }
}

fn normalize_type_field(map: &mut serde_json::Map<String, JsonValue>) {
    if let Some(schema_type) = map.get("type").and_then(JsonValue::as_array).cloned() {
        let normalized = schema_type
            .into_iter()
            .filter_map(|value| value.as_str().and_then(normalize_schema_type_name))
            .map(|value| JsonValue::String(value.to_string()))
            .collect::<Vec<_>>();
        match normalized.as_slice() {
            [] => {
                map.remove("type");
            }
            [single] => {
                map.insert("type".to_string(), single.clone());
            }
            _ => {
                map.insert("type".to_string(), JsonValue::Array(normalized));
            }
        }
    } else if let Some(schema_type) = map.get("type").and_then(JsonValue::as_str)
        && normalize_schema_type_name(schema_type).is_none()
    {
        map.remove("type");
    }
}

fn ensure_default_children_for_type_union(
    map: &mut serde_json::Map<String, JsonValue>,
    types: &[JsonValue],
) {
    let has_object = types.iter().any(|value| value.as_str() == Some("object"));
    if has_object && !map.contains_key("properties") {
        map.insert(
            "properties".to_string(),
            JsonValue::Object(serde_json::Map::new()),
        );
    }

    let has_array = types.iter().any(|value| value.as_str() == Some("array"));
    if has_array && !map.contains_key("items") {
        map.insert("items".to_string(), json!({ "type": "string" }));
    }
}

fn infer_enum_type(values: Option<&Vec<JsonValue>>) -> Option<JsonSchemaPrimitiveType> {
    let values = values?;
    let first = infer_json_value_type(values.first()?)?;
    if values
        .iter()
        .all(|value| infer_json_value_type(value) == Some(first))
    {
        Some(first)
    } else {
        None
    }
}

fn infer_json_value_type(value: &JsonValue) -> Option<JsonSchemaPrimitiveType> {
    match value {
        JsonValue::String(_) => Some(JsonSchemaPrimitiveType::String),
        JsonValue::Number(_) => Some(JsonSchemaPrimitiveType::Number),
        JsonValue::Bool(_) => Some(JsonSchemaPrimitiveType::Boolean),
        JsonValue::Null => Some(JsonSchemaPrimitiveType::Null),
        _ => None,
    }
}

fn normalize_schema_type_name(schema_type: &str) -> Option<&'static str> {
    match schema_type {
        "string" => Some("string"),
        "number" => Some("number"),
        "boolean" => Some("boolean"),
        "integer" => Some("integer"),
        "object" => Some("object"),
        "array" => Some("array"),
        "null" => Some("null"),
        _ => None,
    }
}

fn schema_type_name(schema_type: JsonSchemaPrimitiveType) -> &'static str {
    match schema_type {
        JsonSchemaPrimitiveType::String => "string",
        JsonSchemaPrimitiveType::Number => "number",
        JsonSchemaPrimitiveType::Boolean => "boolean",
        JsonSchemaPrimitiveType::Integer => "integer",
        JsonSchemaPrimitiveType::Object => "object",
        JsonSchemaPrimitiveType::Array => "array",
        JsonSchemaPrimitiveType::Null => "null",
    }
}

#[cfg(test)]
#[path = "json_schema_tests.rs"]
mod tests;
