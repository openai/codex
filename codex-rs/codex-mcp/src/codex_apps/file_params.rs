//! Apps SDK `openai/fileParams` metadata and schema shaping.
//!
//! For each declared file argument, this module derives the provided-file fields
//! accepted by its input schema and records them on `ToolInfo` for execution-time
//! argument rewriting. It also presents file arguments to the model as local paths.
//!
//! See <https://developers.openai.com/apps-sdk/reference#tool-descriptor-parameters>.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::Map;
use serde_json::Value as JsonValue;

use crate::tools::ToolInfo;

const META_OPENAI_FILE_PARAMS: &str = "openai/fileParams";

#[derive(Default)]
struct OpenAiFileSchemaInfo {
    accepts_mime_type: bool,
    accepts_file_name: bool,
}

pub fn declared_openai_file_input_param_names(
    meta: Option<&Map<String, JsonValue>>,
) -> Vec<String> {
    let Some(meta) = meta else {
        return Vec::new();
    };

    meta.get(META_OPENAI_FILE_PARAMS)
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter_map(JsonValue::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

/// Derives execution-time file capabilities from the raw schema, then masks
/// declared file arguments as local paths for the model.
pub(crate) fn prepare_openai_file_params_for_model(tool_info: &mut ToolInfo) {
    let file_params = declared_openai_file_input_param_names(tool_info.tool.meta.as_deref());
    tool_info.openai_file_input_optional_fields =
        supported_openai_file_input_optional_fields(&tool_info.tool, &file_params);

    if file_params.is_empty() {
        return;
    }

    let mut tool = tool_info.tool.clone();
    let mut input_schema = JsonValue::Object(tool.input_schema.as_ref().clone());
    rewrite_input_schema_for_local_file_paths(&mut input_schema, &file_params);
    if let JsonValue::Object(input_schema) = input_schema {
        tool.input_schema = Arc::new(input_schema);
    }
    tool_info.tool = tool;
}

fn supported_openai_file_input_optional_fields(
    tool: &Tool,
    file_params: &[String],
) -> HashMap<String, Vec<String>> {
    let properties = tool
        .input_schema
        .get("properties")
        .and_then(JsonValue::as_object);

    file_params
        .iter()
        .map(|field_name| {
            let optional_fields = properties
                .and_then(|properties| properties.get(field_name))
                .map(|schema| {
                    let schema_info = openai_file_schema_info(schema, tool.input_schema.as_ref());
                    let mut optional_fields = Vec::new();
                    if schema_info.accepts_mime_type {
                        optional_fields.push("mime_type".to_string());
                    }
                    if schema_info.accepts_file_name {
                        optional_fields.push("file_name".to_string());
                    }
                    optional_fields
                })
                .unwrap_or_default();
            (field_name.clone(), optional_fields)
        })
        .collect()
}

fn openai_file_schema_info(
    schema: &JsonValue,
    root_schema: &Map<String, JsonValue>,
) -> OpenAiFileSchemaInfo {
    let mut info = OpenAiFileSchemaInfo::default();
    let mut pending = vec![schema];
    let mut visited_refs = HashSet::new();

    while let Some(schema) = pending.pop() {
        let Some(schema) = schema.as_object() else {
            continue;
        };

        if let Some(schema_ref) = schema.get("$ref").and_then(JsonValue::as_str)
            && visited_refs.insert(schema_ref)
            && let Some(referenced_schema) = resolve_local_schema_ref(root_schema, schema_ref)
        {
            pending.push(referenced_schema);
        }

        for keyword in ["anyOf", "oneOf", "allOf"] {
            if let Some(variants) = schema.get(keyword).and_then(JsonValue::as_array) {
                pending.extend(variants);
            }
        }

        if schema.get("type").and_then(JsonValue::as_str) == Some("array")
            || schema.contains_key("items")
        {
            if let Some(items) = schema.get("items") {
                pending.push(items);
            }
            continue;
        }

        let properties = schema.get("properties").and_then(JsonValue::as_object);
        let is_object_schema = schema.get("type").and_then(JsonValue::as_str) == Some("object")
            || properties.is_some()
            || schema.contains_key("additionalProperties");
        if !is_object_schema {
            continue;
        }
        let accepts_additional_properties = !matches!(
            schema.get("additionalProperties"),
            Some(JsonValue::Bool(false) | JsonValue::Object(_))
        );
        info.accepts_mime_type |= accepts_additional_properties
            || properties.is_some_and(|properties| properties.contains_key("mime_type"));
        info.accepts_file_name |= accepts_additional_properties
            || properties.is_some_and(|properties| properties.contains_key("file_name"));
    }

    info
}

fn resolve_local_schema_ref<'a>(
    root_schema: &'a Map<String, JsonValue>,
    schema_ref: &str,
) -> Option<&'a JsonValue> {
    let pointer = schema_ref.strip_prefix("#/")?;
    let mut segments = pointer.split('/');
    let first_segment = segments.next()?.replace("~1", "/").replace("~0", "~");
    let mut referenced_schema = root_schema.get(&first_segment)?;

    for segment in segments {
        let segment = segment.replace("~1", "/").replace("~0", "~");
        referenced_schema = match referenced_schema {
            JsonValue::Object(object) => object.get(&segment)?,
            JsonValue::Array(array) => array.get(segment.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }

    Some(referenced_schema)
}

fn rewrite_input_schema_for_local_file_paths(input_schema: &mut JsonValue, file_params: &[String]) {
    let Some(properties) = input_schema
        .as_object_mut()
        .and_then(|schema| schema.get_mut("properties"))
        .and_then(JsonValue::as_object_mut)
    else {
        return;
    };

    for field_name in file_params {
        let Some(property_schema) = properties.get_mut(field_name) else {
            continue;
        };
        rewrite_input_property_schema_as_local_file_path(property_schema);
    }
}

fn rewrite_input_property_schema_as_local_file_path(schema: &mut JsonValue) {
    let Some(object) = schema.as_object_mut() else {
        return;
    };

    let mut description = object
        .get("description")
        .and_then(JsonValue::as_str)
        .map(str::to_string)
        .unwrap_or_default();
    let guidance = "This parameter expects an absolute local file path. If you want to upload a file, provide the absolute path to that file here.";
    if description.is_empty() {
        description = guidance.to_string();
    } else if !description.contains(guidance) {
        description = format!("{description} {guidance}");
    }

    let is_array = object.get("type").and_then(JsonValue::as_str) == Some("array")
        || object.get("items").is_some();
    object.clear();
    object.insert("description".to_string(), JsonValue::String(description));
    if is_array {
        object.insert("type".to_string(), JsonValue::String("array".to_string()));
        object.insert("items".to_string(), serde_json::json!({ "type": "string" }));
    } else {
        object.insert("type".to_string(), JsonValue::String("string".to_string()));
    }
}

#[cfg(test)]
#[path = "file_params_tests.rs"]
mod tests;
