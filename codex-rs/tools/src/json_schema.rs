//! Normalize tool schemas and prune unreachable definitions before applying the size policy.

mod compaction;
mod traversal;
mod types;

pub use types::AdditionalProperties;
pub use types::JsonSchema;
pub use types::JsonSchemaPrimitiveType;
pub use types::JsonSchemaType;

use compaction::DEFAULT_COMPACT_TOOL_SCHEMA_BYTES;
use compaction::compact_large_tool_schema;
use traversal::DefinitionTraversal;
use traversal::for_each_schema_child;

use serde_json::Value as JsonValue;
use serde_json::json;
use std::collections::BTreeSet;

const DEFINITION_TABLE_KEYS: [&str; 2] = ["$defs", "definitions"];
const SCHEMA_CHILD_KEYS: [&str; 4] = ["items", "anyOf", "oneOf", "allOf"];
const COMPOSITION_SCHEMA_KEYS: [&str; 3] = ["anyOf", "oneOf", "allOf"];

/// Parse the tool `input_schema` or return an error for invalid schema.
pub fn parse_tool_input_schema(input_schema: &JsonValue) -> Result<JsonSchema, serde_json::Error> {
    parse_tool_input_schema_with_max_bytes(input_schema, DEFAULT_COMPACT_TOOL_SCHEMA_BYTES)
}

/// Compact a tool input schema using the supplied threshold.
pub(crate) fn parse_tool_input_schema_with_max_bytes(
    input_schema: &JsonValue,
    max_bytes: usize,
) -> Result<JsonSchema, serde_json::Error> {
    let mut input_schema = prepare_tool_input_schema(input_schema);
    compact_large_tool_schema(&mut input_schema, max_bytes);
    deserialize_tool_input_schema(input_schema)
}

/// Parse a trusted tool `input_schema` without running large-schema compaction.
pub fn parse_tool_input_schema_without_compaction(
    input_schema: &JsonValue,
) -> Result<JsonSchema, serde_json::Error> {
    deserialize_tool_input_schema(prepare_tool_input_schema(input_schema))
}

fn prepare_tool_input_schema(input_schema: &JsonValue) -> JsonValue {
    let mut input_schema = input_schema.clone();
    sanitize_json_schema(&mut input_schema);
    prune_unreachable_definitions(&mut input_schema);
    input_schema
}

fn deserialize_tool_input_schema(input_schema: JsonValue) -> Result<JsonSchema, serde_json::Error> {
    let schema: JsonSchema = serde_json::from_value(input_schema)?;
    if matches!(
        schema.schema_type,
        Some(JsonSchemaType::Single(JsonSchemaPrimitiveType::Null))
    ) {
        return Err(singleton_null_schema_error());
    }
    Ok(schema)
}

fn has_composition_keyword(map: &serde_json::Map<String, JsonValue>) -> bool {
    COMPOSITION_SCHEMA_KEYS
        .into_iter()
        .any(|key| map.contains_key(key))
}

/// Sanitize a JSON Schema (as serde_json::Value) so it can fit our limited
/// schema representation. This function:
/// - Ensures every typed schema object has a `"type"` when required.
/// - Preserves explicit `anyOf`, `oneOf`, and `allOf`.
/// - Preserves `$ref` and reachable local `$defs` / `definitions`.
/// - Collapses `const` into single-value `enum`.
/// - Fills required child fields for object/array schema types, including
///   nullable unions, with permissive defaults when absent.
/// - Coerces object schemas with no recognized schema hints into `{}`.
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
            for key in COMPOSITION_SCHEMA_KEYS {
                if let Some(value) = map.get_mut(key) {
                    sanitize_json_schema(value);
                }
            }
            for table in DEFINITION_TABLE_KEYS {
                sanitize_schema_table(map, table);
            }

            if let Some(const_value) = map.remove("const") {
                map.insert("enum".to_string(), JsonValue::Array(vec![const_value]));
            }

            let mut schema_types = normalized_schema_types(map);

            if schema_types.is_empty() && (map.contains_key("$ref") || has_composition_keyword(map))
            {
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
                    map.clear();
                    return;
                }
            }

            write_schema_types(map, &schema_types);
            ensure_default_children_for_schema_types(map, &schema_types);
        }
        _ => {}
    }
}

/// Sanitize a schema definition table before deserializing into `JsonSchema`.
///
/// Definition tables must be objects. Codex keeps valid definition tables and
/// recursively applies the same compatibility lowering used for inline schemas,
/// but drops malformed tables so `strict: false` tool registration degrades
/// gracefully instead of failing on an unreachable or invalid definition table.
fn sanitize_schema_table(map: &mut serde_json::Map<String, JsonValue>, key: &str) {
    let should_remove = match map.get_mut(key) {
        Some(JsonValue::Object(definitions)) => {
            for definition in definitions.values_mut() {
                sanitize_json_schema(definition);
            }
            false
        }
        Some(_) => true,
        None => false,
    };

    if should_remove {
        map.remove(key);
    }
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct DefinitionPointer {
    table: &'static str,
    name: String,
}

/// Prune unused root definition entries to avoid sending tokens for definitions
/// the tool schema never references.
fn prune_unreachable_definitions(value: &mut JsonValue) {
    let reachable = collect_reachable_definitions(value);
    let JsonValue::Object(map) = value else {
        return;
    };

    for table in DEFINITION_TABLE_KEYS {
        prune_schema_table(map, table, &reachable);
    }
}

fn prune_schema_table(
    map: &mut serde_json::Map<String, JsonValue>,
    table: &'static str,
    reachable: &BTreeSet<DefinitionPointer>,
) {
    let Some(JsonValue::Object(definitions)) = map.get_mut(table) else {
        return;
    };

    definitions.retain(|name, _| {
        reachable.contains(&DefinitionPointer {
            table,
            name: name.clone(),
        })
    });

    if definitions.is_empty() {
        map.remove(table);
    }
}

fn collect_reachable_definitions(value: &JsonValue) -> BTreeSet<DefinitionPointer> {
    let mut reachable = BTreeSet::new();
    let mut pending = Vec::new();

    collect_refs_outside_definitions(value, &mut pending);

    while let Some(pointer) = pending.pop() {
        if !reachable.insert(pointer.clone()) {
            continue;
        }

        if let Some(definition) = definition_for_pointer(value, &pointer) {
            collect_refs(definition, &mut pending);
        }
    }

    reachable
}

fn collect_refs_outside_definitions(value: &JsonValue, refs: &mut Vec<DefinitionPointer>) {
    match value {
        JsonValue::Array(values) => {
            for value in values {
                collect_refs_outside_definitions(value, refs);
            }
        }
        JsonValue::Object(map) => {
            collect_ref_from_map(map, refs);
            for_each_schema_child(map, DefinitionTraversal::Skip, &mut |value| {
                collect_refs_outside_definitions(value, refs);
            });
        }
        _ => {}
    }
}

fn collect_refs(value: &JsonValue, refs: &mut Vec<DefinitionPointer>) {
    match value {
        JsonValue::Array(values) => {
            for value in values {
                collect_refs(value, refs);
            }
        }
        JsonValue::Object(map) => {
            collect_ref_from_map(map, refs);
            for value in map.values() {
                collect_refs(value, refs);
            }
        }
        _ => {}
    }
}

fn collect_ref_from_map(
    map: &serde_json::Map<String, JsonValue>,
    refs: &mut Vec<DefinitionPointer>,
) {
    if let Some(JsonValue::String(schema_ref)) = map.get("$ref")
        && let Some(pointer) = parse_local_definition_ref(schema_ref)
    {
        refs.push(pointer);
    }
}

fn definition_for_pointer<'a>(
    value: &'a JsonValue,
    pointer: &DefinitionPointer,
) -> Option<&'a JsonValue> {
    let JsonValue::Object(map) = value else {
        return None;
    };

    map.get(pointer.table)
        .and_then(JsonValue::as_object)
        .and_then(|definitions| definitions.get(&pointer.name))
}

fn parse_local_definition_ref(schema_ref: &str) -> Option<DefinitionPointer> {
    let fragment = schema_ref.strip_prefix('#')?;
    let pointer = urlencoding::decode(fragment).ok()?;
    let pointer = jsonptr::Pointer::parse(pointer.as_ref()).ok()?;

    let (table_token, pointer) = pointer.split_front()?;
    let table = table_token.decoded();
    let table = DEFINITION_TABLE_KEYS
        .into_iter()
        .find(|candidate| table.as_ref() == *candidate)?;

    // Responses API non-strict mode accepts nested local refs such as
    // `#/$defs/User/properties/name`, so keep the parent definition reachable.
    let (name, _) = pointer.split_front()?;
    Some(DefinitionPointer {
        table,
        name: name.decoded().into_owned(),
    })
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
