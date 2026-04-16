use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use serde_json::json;
use std::collections::BTreeMap;

/// Primitive JSON Schema type names we support in tool definitions.
///
/// This mirrors the OpenAI Structured Outputs subset for JSON Schema `type`:
/// string, number, boolean, integer, object, array, and null.
/// Keywords such as `enum`, `const`, and `anyOf` are modeled separately.
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
    let root_schema = input_schema.clone();
    let mut input_schema = root_schema.clone();
    sanitize_json_schema(&mut input_schema, &root_schema, &mut Vec::new());
    let schema: JsonSchema = serde_json::from_value(input_schema)?;
    if matches!(
        schema.schema_type,
        Some(JsonSchemaType::Single(JsonSchemaPrimitiveType::Null))
    ) {
        return Err(singleton_null_schema_error());
    }
    Ok(schema)
}

/// Sanitize a JSON Schema (as serde_json::Value) so it can fit our limited
/// schema representation. This function:
/// - Ensures every typed schema object has a `"type"` when required.
/// - Resolves local `$ref` indirections and unwraps single-variant
///   `oneOf`/`anyOf`/`allOf` wrappers before inferring a fallback type.
/// - Preserves explicit `anyOf`.
/// - Collapses `const` into single-value `enum`.
/// - Fills required child fields for object/array schema types, including
///   nullable unions, with permissive defaults when absent.
fn sanitize_json_schema(
    value: &mut JsonValue,
    root_schema: &JsonValue,
    active_refs: &mut Vec<String>,
) {
    match value {
        JsonValue::Bool(_) => {
            // JSON Schema boolean form: true/false. Coerce to an accept-all string.
            *value = json!({ "type": "string" });
        }
        JsonValue::Array(values) => {
            for value in values {
                sanitize_json_schema(value, root_schema, active_refs);
            }
        }
        JsonValue::Object(map) => {
            let active_ref_depth = active_refs.len();
            if let Some(pointer) = map
                .get("$ref")
                .and_then(JsonValue::as_str)
                .and_then(|reference| reference.strip_prefix('#'))
            {
                if active_refs.iter().any(|active_ref| active_ref == pointer) {
                    *value = cyclic_reference_fallback(map, root_schema.pointer(pointer));
                    sanitize_json_schema(value, root_schema, active_refs);
                    return;
                }

                active_refs.push(pointer.to_string());
            }

            if let Some(replacement) = resolve_json_schema_reference(map, root_schema) {
                *value = replacement;
                sanitize_json_schema(value, root_schema, active_refs);
                active_refs.pop();
                return;
            }

            if let Some(replacement) = unwrap_single_variant_combiner(map) {
                *value = replacement;
                sanitize_json_schema(value, root_schema, active_refs);
                return;
            }

            if let Some(properties) = map.get_mut("properties")
                && let Some(properties_map) = properties.as_object_mut()
            {
                for value in properties_map.values_mut() {
                    sanitize_json_schema(value, root_schema, active_refs);
                }
            }
            if let Some(items) = map.get_mut("items") {
                sanitize_json_schema(items, root_schema, active_refs);
            }
            if let Some(additional_properties) = map.get_mut("additionalProperties")
                && !matches!(additional_properties, JsonValue::Bool(_))
            {
                sanitize_json_schema(additional_properties, root_schema, active_refs);
            }
            if let Some(value) = map.get_mut("prefixItems") {
                sanitize_json_schema(value, root_schema, active_refs);
            }
            if let Some(value) = map.get_mut("anyOf") {
                sanitize_json_schema(value, root_schema, active_refs);
            }

            if let Some(const_value) = map.remove("const") {
                map.insert("enum".to_string(), JsonValue::Array(vec![const_value]));
            }

            let mut schema_types = normalized_schema_types(map);

            if schema_types.is_empty() && map.contains_key("anyOf") {
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

            active_refs.truncate(active_ref_depth);
        }
        _ => {}
    }
}

fn resolve_json_schema_reference(
    map: &serde_json::Map<String, JsonValue>,
    root_schema: &JsonValue,
) -> Option<JsonValue> {
    let reference = map.get("$ref")?.as_str()?;
    let pointer = reference.strip_prefix('#')?;
    let mut replacement = root_schema.pointer(pointer)?.clone();
    if matches!(replacement, JsonValue::Bool(true)) {
        replacement = JsonValue::Object(serde_json::Map::new());
    }
    if let JsonValue::Object(replacement_map) = &mut replacement {
        merge_json_schema_objects(replacement_map, map);
    }
    Some(replacement)
}

fn cyclic_reference_fallback(
    map: &serde_json::Map<String, JsonValue>,
    reference_target: Option<&JsonValue>,
) -> JsonValue {
    let mut fallback = reference_target
        .and_then(cyclic_reference_target_fallback)
        .unwrap_or_else(|| json!({ "type": "string" }));

    if let JsonValue::Object(fallback_map) = &mut fallback {
        merge_json_schema_objects(fallback_map, map);
        fallback_map.remove("$ref");
    }

    fallback
}

fn cyclic_reference_target_fallback(reference_target: &JsonValue) -> Option<JsonValue> {
    let target_map = reference_target.as_object()?;
    let schema_types = normalized_schema_types(target_map);

    if schema_types.contains(&JsonSchemaPrimitiveType::Object)
        || target_map.contains_key("properties")
        || target_map.contains_key("required")
        || target_map.contains_key("additionalProperties")
    {
        return Some(json!({ "type": "object", "properties": {} }));
    }

    if schema_types.contains(&JsonSchemaPrimitiveType::Array)
        || target_map.contains_key("items")
        || target_map.contains_key("prefixItems")
    {
        return Some(json!({ "type": "array", "items": { "type": "string" } }));
    }

    if schema_types.contains(&JsonSchemaPrimitiveType::Number) {
        return Some(json!({ "type": "number" }));
    }

    if schema_types.contains(&JsonSchemaPrimitiveType::Integer) {
        return Some(json!({ "type": "integer" }));
    }

    if schema_types.contains(&JsonSchemaPrimitiveType::Boolean) {
        return Some(json!({ "type": "boolean" }));
    }

    Some(json!({ "type": "string" }))
}

fn merge_json_schema_objects(
    base: &mut serde_json::Map<String, JsonValue>,
    overlay: &serde_json::Map<String, JsonValue>,
) {
    for (key, overlay_value) in overlay {
        if key == "$ref" {
            continue;
        }

        let Some(base_value) = base.get_mut(key) else {
            base.insert(key.clone(), overlay_value.clone());
            continue;
        };

        if base_value == overlay_value {
            continue;
        }

        match key.as_str() {
            "required" => merge_required_arrays(base_value, overlay_value),
            "properties" => merge_property_maps(base_value, overlay_value),
            "type" => merge_schema_types(base_value, overlay_value),
            "additionalProperties" => {
                if let Some(merged_value) =
                    merge_additional_properties(base_value.clone(), overlay_value.clone())
                {
                    *base_value = merged_value;
                }
            }
            "minimum" | "exclusiveMinimum" => merge_numeric_bound(base_value, overlay_value, true),
            "maximum" | "exclusiveMaximum" => merge_numeric_bound(base_value, overlay_value, false),
            _ => {
                if let (Some(base_map), Some(overlay_map)) =
                    (base_value.as_object_mut(), overlay_value.as_object())
                {
                    merge_json_schema_objects(base_map, overlay_map);
                }
            }
        }
    }
}

fn merge_required_arrays(base_value: &mut JsonValue, overlay_value: &JsonValue) {
    let Some(base_array) = base_value.as_array_mut() else {
        return;
    };
    let Some(overlay_array) = overlay_value.as_array() else {
        return;
    };

    for required_value in overlay_array {
        if !base_array.contains(required_value) {
            base_array.push(required_value.clone());
        }
    }
}

fn merge_property_maps(base_value: &mut JsonValue, overlay_value: &JsonValue) {
    let Some(base_map) = base_value.as_object_mut() else {
        return;
    };
    let Some(overlay_map) = overlay_value.as_object() else {
        return;
    };

    for (property_name, overlay_property) in overlay_map {
        let Some(base_property) = base_map.get_mut(property_name) else {
            base_map.insert(property_name.clone(), overlay_property.clone());
            continue;
        };

        if let (Some(base_property_map), Some(overlay_property_map)) =
            (base_property.as_object_mut(), overlay_property.as_object())
        {
            merge_json_schema_objects(base_property_map, overlay_property_map);
        }
    }
}

fn merge_schema_types(base_value: &mut JsonValue, overlay_value: &JsonValue) {
    let base_types = json_schema_type_names(base_value);
    let overlay_types = json_schema_type_names(overlay_value);
    if base_types.is_empty() || overlay_types.is_empty() {
        return;
    }

    let merged_types: Vec<String> = [
        "string", "number", "boolean", "integer", "object", "array", "null",
    ]
    .into_iter()
    .filter(|candidate| type_sets_overlap(&base_types, &overlay_types, candidate))
    .map(str::to_string)
    .collect();
    if merged_types.is_empty() {
        return;
    }

    *base_value = match merged_types.as_slice() {
        [schema_type] => JsonValue::String(schema_type.clone()),
        _ => JsonValue::Array(
            merged_types
                .into_iter()
                .map(JsonValue::String)
                .collect::<Vec<_>>(),
        ),
    };
}

fn type_sets_overlap(base_types: &[String], overlay_types: &[String], candidate: &str) -> bool {
    if base_types.iter().any(|base_type| base_type == candidate)
        && overlay_types
            .iter()
            .any(|overlay_type| overlay_type == candidate)
    {
        return true;
    }

    candidate == "integer"
        && ((base_types.iter().any(|base_type| base_type == "integer")
            && overlay_types
                .iter()
                .any(|overlay_type| overlay_type == "number"))
            || (base_types.iter().any(|base_type| base_type == "number")
                && overlay_types
                    .iter()
                    .any(|overlay_type| overlay_type == "integer")))
}

fn json_schema_type_names(value: &JsonValue) -> Vec<String> {
    match value {
        JsonValue::String(schema_type) => vec![schema_type.clone()],
        JsonValue::Array(values) => values
            .iter()
            .filter_map(JsonValue::as_str)
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn merge_additional_properties(
    base_value: JsonValue,
    overlay_value: JsonValue,
) -> Option<JsonValue> {
    match (base_value, overlay_value) {
        (JsonValue::Bool(false), _) | (_, JsonValue::Bool(false)) => Some(JsonValue::Bool(false)),
        (JsonValue::Bool(true), other) | (other, JsonValue::Bool(true)) => Some(other),
        (JsonValue::Object(mut base_map), JsonValue::Object(overlay_map)) => {
            merge_json_schema_objects(&mut base_map, &overlay_map);
            Some(JsonValue::Object(base_map))
        }
        (base_value, overlay_value) if base_value == overlay_value => Some(base_value),
        _ => None,
    }
}

fn merge_numeric_bound(base_value: &mut JsonValue, overlay_value: &JsonValue, take_max: bool) {
    let (Some(base_number), Some(overlay_number)) = (base_value.as_f64(), overlay_value.as_f64())
    else {
        return;
    };

    let merged_number = if take_max {
        base_number.max(overlay_number)
    } else {
        base_number.min(overlay_number)
    };
    if let Some(number) = serde_json::Number::from_f64(merged_number) {
        *base_value = JsonValue::Number(number);
    }
}

fn unwrap_single_variant_combiner(map: &serde_json::Map<String, JsonValue>) -> Option<JsonValue> {
    for combiner in ["oneOf", "anyOf", "allOf"] {
        let Some(variants) = map.get(combiner).and_then(JsonValue::as_array) else {
            continue;
        };
        if variants.len() != 1 {
            continue;
        }

        let mut replacement = variants[0].clone();
        if let JsonValue::Object(replacement_map) = &mut replacement {
            merge_json_schema_objects(replacement_map, map);
            replacement_map.remove(combiner);
        }
        return Some(replacement);
    }

    None
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
