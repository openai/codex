use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use serde_json::Value as JsonValue;
use serde_json::json;
use std::collections::BTreeMap;

/// Primitive JSON Schema type names we support in tool definitions.
///
/// This mirrors the OpenAI Structured Outputs subset for JSON Schema `type`:
/// string, number, boolean, integer, object, array, and null.
/// Keywords such as `enum`, `const`, `anyOf`, `allOf`, and `oneOf` are modeled
/// separately.
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum JsonSchemaType {
    Single(JsonSchemaPrimitiveType),
    Multiple(Vec<JsonSchemaPrimitiveType>),
}

/// Generic JSON-Schema subset needed for our tool definitions.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct JsonSchema {
    raw_schema: Option<JsonValue>,
    pub schema_type: Option<JsonSchemaType>,
    pub description: Option<String>,
    pub enum_values: Option<Vec<JsonValue>>,
    pub items: Option<Box<JsonSchema>>,
    pub properties: Option<BTreeMap<String, JsonSchema>>,
    pub required: Option<Vec<String>>,
    pub additional_properties: Option<AdditionalProperties>,
    pub any_of: Option<Vec<JsonSchema>>,
    pub all_of: Option<Vec<JsonSchema>>,
    pub one_of: Option<Vec<JsonSchema>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct JsonSchemaFields {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    schema_type: Option<JsonSchemaType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    enum_values: Option<Vec<JsonValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    items: Option<Box<JsonSchema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    properties: Option<BTreeMap<String, JsonSchema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    required: Option<Vec<String>>,
    #[serde(
        rename = "additionalProperties",
        skip_serializing_if = "Option::is_none"
    )]
    additional_properties: Option<AdditionalProperties>,
    #[serde(rename = "anyOf", skip_serializing_if = "Option::is_none")]
    any_of: Option<Vec<JsonSchema>>,
    #[serde(rename = "allOf", skip_serializing_if = "Option::is_none")]
    all_of: Option<Vec<JsonSchema>>,
    #[serde(rename = "oneOf", skip_serializing_if = "Option::is_none")]
    one_of: Option<Vec<JsonSchema>>,
}

impl From<JsonSchemaFields> for JsonSchema {
    fn from(fields: JsonSchemaFields) -> Self {
        Self {
            raw_schema: None,
            schema_type: fields.schema_type,
            description: fields.description,
            enum_values: fields.enum_values,
            items: fields.items,
            properties: fields.properties,
            required: fields.required,
            additional_properties: fields.additional_properties,
            any_of: fields.any_of,
            all_of: fields.all_of,
            one_of: fields.one_of,
        }
    }
}

impl From<&JsonSchema> for JsonSchemaFields {
    fn from(schema: &JsonSchema) -> Self {
        Self {
            schema_type: schema.schema_type.clone(),
            description: schema.description.clone(),
            enum_values: schema.enum_values.clone(),
            items: schema.items.clone(),
            properties: schema.properties.clone(),
            required: schema.required.clone(),
            additional_properties: schema.additional_properties.clone(),
            any_of: schema.any_of.clone(),
            all_of: schema.all_of.clone(),
            one_of: schema.one_of.clone(),
        }
    }
}

impl Serialize for JsonSchema {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match &self.raw_schema {
            Some(raw_schema) => raw_schema.serialize(serializer),
            None => JsonSchemaFields::from(self).serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for JsonSchema {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        JsonSchemaFields::deserialize(deserializer).map(Into::into)
    }
}

impl JsonSchema {
    pub fn from_raw_tool_input_schema(input_schema: JsonValue) -> Self {
        Self {
            raw_schema: Some(normalize_raw_tool_input_schema(input_schema)),
            ..Default::default()
        }
    }

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

    pub fn all_of(variants: Vec<JsonSchema>, description: Option<String>) -> Self {
        Self {
            description,
            all_of: Some(variants),
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

    pub fn string_enum(values: Vec<JsonValue>, description: Option<String>) -> Self {
        Self {
            schema_type: Some(JsonSchemaType::Single(JsonSchemaPrimitiveType::String)),
            description,
            enum_values: Some(values),
            ..Default::default()
        }
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
    let schema: JsonSchema = serde_json::from_value(input_schema)?;
    if matches!(
        schema.schema_type,
        Some(JsonSchemaType::Single(JsonSchemaPrimitiveType::Null))
    ) {
        return Err(singleton_null_schema_error());
    }
    Ok(schema)
}

fn normalize_raw_tool_input_schema(input_schema: JsonValue) -> JsonValue {
    let JsonValue::Object(mut map) = input_schema else {
        return empty_root_tool_schema();
    };

    if !root_schema_allows_object(&map) {
        return empty_root_tool_schema();
    }

    if map.get("type").is_none_or(JsonValue::is_null) {
        map.insert("type".to_string(), JsonValue::String("object".to_string()));
    }

    if map.get("properties").is_none_or(JsonValue::is_null) {
        map.insert(
            "properties".to_string(),
            JsonValue::Object(serde_json::Map::new()),
        );
    }

    JsonValue::Object(map)
}

fn empty_root_tool_schema() -> JsonValue {
    json!({
        "type": "object",
        "properties": {}
    })
}

fn root_schema_allows_object(map: &serde_json::Map<String, JsonValue>) -> bool {
    match map.get("type") {
        None | Some(JsonValue::Null) => true,
        Some(JsonValue::String(schema_type)) => schema_type == "object",
        Some(JsonValue::Array(schema_types)) => schema_types
            .iter()
            .any(|schema_type| schema_type.as_str() == Some("object")),
        Some(_) => false,
    }
}

/// Sanitize a JSON Schema (as serde_json::Value) so it can fit our limited
/// schema representation. This function:
/// - Ensures every typed schema object has a `"type"` when required.
/// - Preserves explicit `anyOf`, `allOf`, and `oneOf`.
/// - Collapses `const` into single-value `enum`.
/// - Fills required child fields for object/array schema types, including
///   nullable unions, with permissive defaults when absent.
fn sanitize_json_schema(value: &mut JsonValue) {
    match value {
        JsonValue::Bool(true) => {
            // JSON Schema `true` is an accept-all schema.
            *value = json!({});
        }
        JsonValue::Bool(false) => {
            // `false` is unsatisfiable, which our tool schema subset cannot
            // express. Keep the existing permissive fallback for that rare
            // shape rather than rejecting the whole tool definition.
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
            for keyword in ["anyOf", "allOf", "oneOf"] {
                if let Some(value) = map.get_mut(keyword) {
                    sanitize_json_schema(value);
                }
            }

            if let Some(const_value) = map.remove("const") {
                map.insert("enum".to_string(), JsonValue::Array(vec![const_value]));
            }

            let mut schema_types = normalized_schema_types(map);

            if schema_types.is_empty() && (map.is_empty() || contains_schema_combiner(map)) {
                return;
            }

            if schema_types.is_empty() {
                if map.contains_key("properties")
                    || map.contains_key("required")
                    || map.contains_key("additionalProperties")
                {
                    schema_types.push(JsonSchemaPrimitiveType::Object);
                } else if map.contains_key("items") || map.contains_key("prefixItems") {
                    schema_types.push(JsonSchemaPrimitiveType::Array);
                } else if map.contains_key("enum") || map.contains_key("format") {
                    schema_types.push(JsonSchemaPrimitiveType::String);
                } else if map.contains_key("minimum")
                    || map.contains_key("maximum")
                    || map.contains_key("exclusiveMinimum")
                    || map.contains_key("exclusiveMaximum")
                    || map.contains_key("multipleOf")
                {
                    schema_types.push(JsonSchemaPrimitiveType::Number);
                } else {
                    schema_types.push(JsonSchemaPrimitiveType::String);
                }
            }

            write_schema_types(map, &schema_types);
            ensure_default_children_for_schema_types(map, &schema_types);
        }
        _ => {}
    }
}

fn contains_schema_combiner(map: &serde_json::Map<String, JsonValue>) -> bool {
    ["anyOf", "allOf", "oneOf"]
        .iter()
        .any(|keyword| map.contains_key(*keyword))
}

fn ensure_default_children_for_schema_types(
    map: &mut serde_json::Map<String, JsonValue>,
    schema_types: &[JsonSchemaPrimitiveType],
) {
    if schema_types.contains(&JsonSchemaPrimitiveType::Object) && !map.contains_key("properties") {
        map.insert(
            "properties".to_string(),
            JsonValue::Object(serde_json::Map::new()),
        );
    }

    if schema_types.contains(&JsonSchemaPrimitiveType::Array) && !map.contains_key("items") {
        map.insert("items".to_string(), json!({ "type": "string" }));
    }
}

fn normalized_schema_types(
    map: &serde_json::Map<String, JsonValue>,
) -> Vec<JsonSchemaPrimitiveType> {
    let Some(schema_type) = map.get("type") else {
        return Vec::new();
    };

    match schema_type {
        JsonValue::String(schema_type) => schema_type_from_str(schema_type).into_iter().collect(),
        JsonValue::Array(schema_types) => schema_types
            .iter()
            .filter_map(JsonValue::as_str)
            .filter_map(schema_type_from_str)
            .collect(),
        _ => Vec::new(),
    }
}

fn write_schema_types(
    map: &mut serde_json::Map<String, JsonValue>,
    schema_types: &[JsonSchemaPrimitiveType],
) {
    match schema_types {
        [] => {
            map.remove("type");
        }
        [schema_type] => {
            map.insert(
                "type".to_string(),
                JsonValue::String(schema_type_name(*schema_type).to_string()),
            );
        }
        _ => {
            map.insert(
                "type".to_string(),
                JsonValue::Array(
                    schema_types
                        .iter()
                        .map(|schema_type| {
                            JsonValue::String(schema_type_name(*schema_type).to_string())
                        })
                        .collect(),
                ),
            );
        }
    }
}

fn schema_type_from_str(schema_type: &str) -> Option<JsonSchemaPrimitiveType> {
    match schema_type {
        "string" => Some(JsonSchemaPrimitiveType::String),
        "number" => Some(JsonSchemaPrimitiveType::Number),
        "boolean" => Some(JsonSchemaPrimitiveType::Boolean),
        "integer" => Some(JsonSchemaPrimitiveType::Integer),
        "object" => Some(JsonSchemaPrimitiveType::Object),
        "array" => Some(JsonSchemaPrimitiveType::Array),
        "null" => Some(JsonSchemaPrimitiveType::Null),
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

fn singleton_null_schema_error() -> serde_json::Error {
    serde_json::Error::io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "tool input schema must not be a singleton null type",
    ))
}

#[cfg(test)]
#[path = "json_schema_tests.rs"]
mod tests;
