//! Apply the legacy best-effort size policy after schema sanitization and pruning.
//! Pass order and normalized-byte accounting are compatibility behavior.

use super::DEFINITION_TABLE_KEYS;
use super::JsonSchema;
use super::SCHEMA_CHILD_KEYS;
use super::has_composition_keyword;
use super::parse_local_definition_ref;
use super::traversal::DefinitionTraversal;
use super::traversal::for_each_schema_child_mut;
use serde_json::Value as JsonValue;
use serde_json::json;

// Measure compact normalized JSON in UTF-8 bytes.
pub(crate) const DEFAULT_COMPACT_TOOL_SCHEMA_BYTES: usize = 5_000;
const MAX_COMPACT_TOOL_SCHEMA_DEPTH: usize = 3;

/// Shrink unusually large tool schemas while preserving the top-level argument
/// surface. Compaction is best-effort rather than a hard cap: it runs only
/// after schema sanitization/pruning and applies increasingly lossy passes
/// while the schema remains over budget.
pub(super) fn compact_large_tool_schema(value: &mut JsonValue, max_bytes: usize) {
    for pass in LARGE_SCHEMA_COMPACTION_PASSES {
        if compact_schema_fits_budget(value, max_bytes) {
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
    prune_schema_compositions,
];

fn collapse_deep_schema_objects_from_root(value: &mut JsonValue) {
    collapse_deep_schema_objects(value, /*depth*/ 0);
}

fn compact_schema_fits_budget(value: &JsonValue, max_bytes: usize) -> bool {
    compact_normalized_schema_len(value) <= max_bytes
}

fn compact_normalized_schema_len(value: &JsonValue) -> usize {
    serde_json::from_value::<JsonSchema>(value.clone())
        .and_then(|schema| serde_json::to_vec(&schema))
        .map(|json| json.len())
        .unwrap_or(0)
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

fn prune_schema_compositions(value: &mut JsonValue) {
    match value {
        JsonValue::Array(values) => {
            for value in values {
                prune_schema_compositions(value);
            }
        }
        JsonValue::Object(map) => {
            if has_composition_keyword(map) {
                *value = json!({});
                return;
            }

            for_each_schema_child_mut(map, DefinitionTraversal::Skip, &mut |value| {
                prune_schema_compositions(value);
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

#[cfg(test)]
#[path = "compaction_tests.rs"]
mod tests;
