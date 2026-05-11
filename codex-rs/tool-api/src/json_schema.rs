use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;

/// Primitive JSON Schema type names we support in tool definitions.
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

/// Generic JSON-Schema subset needed for tool definitions.
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
