pub type PositiveInteger = i64;
pub type PositiveIntegerDefault0 = serde_json::Value;
pub type SchemaArray = Vec<Schema>;
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
#[serde(rename = "simpleTypes")]
pub enum SimpleTypes {
    #[serde(rename = "array")]
    Array,
    #[serde(rename = "boolean")]
    Boolean,
    #[serde(rename = "integer")]
    Integer,
    #[serde(rename = "null")]
    Null,
    #[serde(rename = "number")]
    Number,
    #[serde(rename = "object")]
    Object,
    #[serde(rename = "string")]
    String,
}
pub type StringArray = Vec<String>;
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct Schema {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "$ref")]
    pub ref_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "$schema")]
    pub schema: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "additionalItems")]
    pub additional_items: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "additionalProperties")]
    pub additional_properties: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "allOf")]
    pub all_of: Option<SchemaArray>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "anyOf")]
    pub any_of: Option<SchemaArray>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(default)]
    pub definitions: ::std::collections::BTreeMap<String, Schema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<::std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "enum")]
    pub enum_: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "enumNames")]
    pub enum_names: Option<StringArray>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "exclusiveMaximum")]
    pub exclusive_maximum: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "exclusiveMinimum")]
    pub exclusive_minimum: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    #[serde(with = "::schemafy_core::one_or_many")]
    pub items: Vec<Schema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "maxItems")]
    pub max_items: Option<PositiveInteger>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "maxLength")]
    pub max_length: Option<PositiveInteger>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "maxProperties")]
    pub max_properties: Option<PositiveInteger>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "minItems")]
    pub min_items: Option<PositiveIntegerDefault0>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "minLength")]
    pub min_length: Option<PositiveIntegerDefault0>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "minProperties")]
    pub min_properties: Option<PositiveIntegerDefault0>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "multipleOf")]
    pub multiple_of: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not: Option<Box<Schema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "oneOf")]
    pub one_of: Option<SchemaArray>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(default)]
    #[serde(rename = "patternProperties")]
    pub pattern_properties: ::std::collections::BTreeMap<String, Schema>,
    #[serde(default)]
    pub properties: ::std::collections::BTreeMap<String, Schema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<StringArray>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    #[serde(with = "::schemafy_core::one_or_many")]
    #[serde(rename = "type")]
    pub type_: Vec<SimpleTypes>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "uniqueItems")]
    pub unique_items: Option<bool>,
}
