use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

const DEFINITION_TABLE_KEYS: [&str; 2] = ["$defs", "definitions"];
const SCHEMA_CHILD_KEYS: [&str; 2] = ["items", "anyOf"];

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
    #[serde(rename = "$ref", skip_serializing_if = "Option::is_none")]
    pub schema_ref: Option<String>,
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
    #[serde(rename = "$defs", skip_serializing_if = "Option::is_none")]
    pub defs: Option<BTreeMap<String, JsonSchema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definitions: Option<BTreeMap<String, JsonSchema>>,
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
    let mut input_schema = prepare_tool_input_schema(input_schema);
    compact_large_tool_schema(&mut input_schema);
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

// Use compact normalized JSON bytes as a cheap local proxy for the 1k-token
// schema budget.
const MAX_COMPACT_TOOL_SCHEMA_BYTES: usize = 4_000;
const MAX_COMPACT_TOOL_SCHEMA_DEPTH: usize = 2;

/// Shrink unusually large tool schemas while preserving the top-level argument
/// surface. Compaction is best-effort rather than a hard cap: it runs only
/// after schema sanitization/pruning and applies increasingly lossy passes
/// while the schema remains over budget.
fn compact_large_tool_schema(value: &mut JsonValue) {
    for pass in LARGE_SCHEMA_COMPACTION_PASSES {
        if compact_schema_fits_budget(value) {
            break;
        }
        pass(value);
    }
}

type LargeSchemaCompactionPass = fn(&mut JsonValue);

const LARGE_SCHEMA_COMPACTION_PASSES: &[LargeSchemaCompactionPass] = &[
    strip_schema_descriptions,
    drop_schema_definitions,
    collapse_deep_schema_objects_from_root,
];

fn collapse_deep_schema_objects_from_root(value: &mut JsonValue) {
    collapse_deep_schema_objects(value, /*depth*/ 0);
}

fn compact_schema_fits_budget(value: &JsonValue) -> bool {
    compact_normalized_schema_len(value) <= MAX_COMPACT_TOOL_SCHEMA_BYTES
}

fn compact_normalized_schema_len(value: &JsonValue) -> usize {
    serde_json::from_value::<JsonSchema>(value.clone())
        .and_then(|schema| serde_json::to_vec(&schema))
        .map(|json| json.len())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DefinitionTraversal {
    Include,
    Skip,
}

fn for_each_schema_child(
    map: &serde_json::Map<String, JsonValue>,
    definition_traversal: DefinitionTraversal,
    visitor: &mut impl FnMut(&JsonValue),
) {
    if let Some(properties) = map.get("properties")
        && let Some(properties_map) = properties.as_object()
    {
        for value in properties_map.values() {
            visitor(value);
        }
    }

    for key in SCHEMA_CHILD_KEYS {
        if let Some(value) = map.get(key) {
            visitor(value);
        }
    }

    if let Some(additional_properties) = map.get("additionalProperties")
        && !matches!(additional_properties, JsonValue::Bool(_))
    {
        visitor(additional_properties);
    }

    if definition_traversal == DefinitionTraversal::Include {
        for key in DEFINITION_TABLE_KEYS {
            if let Some(definitions) = map.get(key)
                && let Some(definitions_map) = definitions.as_object()
            {
                for value in definitions_map.values() {
                    visitor(value);
                }
            }
        }
    }
}

fn strip_schema_descriptions(value: &mut JsonValue) {
    match value {
        JsonValue::Array(values) => {
            for value in values {
                strip_schema_descriptions(value);
            }
        }
        JsonValue::Object(map) => {
            map.remove("description");
            for_each_schema_child_mut(map, DefinitionTraversal::Include, &mut |value| {
                strip_schema_descriptions(value);
            });
        }
        _ => {}
    }
}

fn for_each_schema_child_mut(
    map: &mut serde_json::Map<String, JsonValue>,
    definition_traversal: DefinitionTraversal,
    visitor: &mut impl FnMut(&mut JsonValue),
) {
    if let Some(properties) = map.get_mut("properties")
        && let Some(properties_map) = properties.as_object_mut()
    {
        for value in properties_map.values_mut() {
            visitor(value);
        }
    }

    for key in SCHEMA_CHILD_KEYS {
        if let Some(value) = map.get_mut(key) {
            visitor(value);
        }
    }

    if let Some(additional_properties) = map.get_mut("additionalProperties")
        && !matches!(additional_properties, JsonValue::Bool(_))
    {
        visitor(additional_properties);
    }

    if definition_traversal == DefinitionTraversal::Include {
        for key in DEFINITION_TABLE_KEYS {
            if let Some(definitions) = map.get_mut(key)
                && let Some(definitions_map) = definitions.as_object_mut()
            {
                for value in definitions_map.values_mut() {
                    visitor(value);
                }
            }
        }
    }
}

/// Replace local definition refs with empty schemas before dropping root
/// definition tables, so downstream behavior does not depend on how a schema
/// parser handles refs to missing definitions.
fn drop_schema_definitions(value: &mut JsonValue) {
    rewrite_definition_refs_to_empty_schemas(value);

    let JsonValue::Object(map) = value else {
        return;
    };

    for key in DEFINITION_TABLE_KEYS {
        map.remove(key);
    }
}

fn rewrite_definition_refs_to_empty_schemas(value: &mut JsonValue) {
    match value {
        JsonValue::Array(values) => {
            for value in values {
                rewrite_definition_refs_to_empty_schemas(value);
            }
        }
        JsonValue::Object(map) => {
            if map
                .get("$ref")
                .and_then(JsonValue::as_str)
                .and_then(parse_local_definition_ref)
                .is_some()
            {
                *value = json!({});
                return;
            }

            for_each_schema_child_mut(map, DefinitionTraversal::Skip, &mut |value| {
                rewrite_definition_refs_to_empty_schemas(value);
            });
        }
        _ => {}
    }
}

fn collapse_deep_schema_objects(value: &mut JsonValue, depth: usize) {
    match value {
        JsonValue::Array(values) => {
            for value in values {
                collapse_deep_schema_objects(value, depth);
            }
        }
        JsonValue::Object(map) => {
            if depth >= MAX_COMPACT_TOOL_SCHEMA_DEPTH && is_complex_schema_object(map) {
                *value = json!({});
                return;
            }

            for_each_schema_child_mut(map, DefinitionTraversal::Skip, &mut |value| {
                collapse_deep_schema_objects(value, depth + 1);
            });
        }
        _ => {}
    }
}

fn is_complex_schema_object(map: &serde_json::Map<String, JsonValue>) -> bool {
    SCHEMA_CHILD_KEYS.iter().any(|key| map.contains_key(*key))
        || map.contains_key("properties")
        || map.contains_key("additionalProperties")
        || map.contains_key("$ref")
}

/// Sanitize a JSON Schema (as serde_json::Value) so it can fit our limited
/// schema representation. This function:
/// - Ensures every typed schema object has a `"type"` when required.
/// - Preserves explicit `anyOf`.
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
            if let Some(value) = map.get_mut("anyOf") {
                sanitize_json_schema(value);
            }
            for table in DEFINITION_TABLE_KEYS {
                sanitize_schema_table(map, table);
            }

            if let Some(const_value) = map.remove("const") {
                map.insert("enum".to_string(), JsonValue::Array(vec![const_value]));
            }

            let mut schema_types = normalized_schema_types(map);

            if schema_types.is_empty() && (map.contains_key("$ref") || map.contains_key("anyOf")) {
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
