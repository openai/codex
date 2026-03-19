use serde_json::Map;
use serde_json::Value as JsonValue;

use crate::openai_files::META_OPENAI_FILE_OUTPUTS;
use crate::openai_files::META_OPENAI_FILE_PARAMS;

pub(crate) fn declared_openai_file_params(meta: Option<&Map<String, JsonValue>>) -> Vec<String> {
    declared_top_level_fields(meta, META_OPENAI_FILE_PARAMS)
}

pub(crate) fn declared_openai_file_outputs(meta: Option<&Map<String, JsonValue>>) -> Vec<String> {
    declared_top_level_fields(meta, META_OPENAI_FILE_OUTPUTS)
}

pub(crate) fn mask_input_schema_for_model(input_schema: &mut JsonValue, file_params: &[String]) {
    mask_object_properties(input_schema, file_params, MaskTarget::Input);
}

pub(crate) fn mask_output_schema_for_model(output_schema: &mut JsonValue, file_outputs: &[String]) {
    mask_object_properties(output_schema, file_outputs, MaskTarget::Output);
}

fn declared_top_level_fields(meta: Option<&Map<String, JsonValue>>, key: &str) -> Vec<String> {
    let Some(meta) = meta else {
        return Vec::new();
    };

    meta.get(key)
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter_map(JsonValue::as_str)
        .filter(|value| is_top_level_field_name(value))
        .map(str::to_string)
        .collect()
}

fn is_top_level_field_name(field_name: &str) -> bool {
    !field_name.is_empty()
        && !field_name.contains('.')
        && !field_name.contains('/')
        && !field_name.contains('[')
        && !field_name.contains(']')
}

#[derive(Clone, Copy)]
enum MaskTarget {
    Input,
    Output,
}

fn mask_object_properties(schema: &mut JsonValue, file_fields: &[String], target: MaskTarget) {
    let Some(properties) = schema
        .as_object_mut()
        .and_then(|schema| schema.get_mut("properties"))
        .and_then(JsonValue::as_object_mut)
    else {
        return;
    };

    for field_name in file_fields {
        let Some(property_schema) = properties.get_mut(field_name) else {
            continue;
        };
        mask_property_schema(property_schema, target);
    }
}

fn mask_property_schema(schema: &mut JsonValue, target: MaskTarget) {
    let Some(object) = schema.as_object_mut() else {
        return;
    };

    let mut description = object
        .get("description")
        .and_then(JsonValue::as_str)
        .map(str::to_string)
        .unwrap_or_default();
    let guidance = match target {
        MaskTarget::Input => {
            "Pass a local file path string. Codex will upload it before invoking the tool."
        }
        MaskTarget::Output => {
            "This field returns a local temp file path after Codex auto-downloads supported OpenAI file handles."
        }
    };
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
        object.insert(
            "items".to_string(),
            serde_json::json!({
                "type": "string"
            }),
        );
    } else {
        object.insert("type".to_string(), JsonValue::String("string".to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn declared_openai_file_fields_ignore_nested_names() {
        let meta = serde_json::json!({
            "openai/fileParams": ["file", "nested.value", "files[0]", "attachments"],
            "openai/fileOutputs": ["output", "artifacts/0"]
        });
        let meta = meta.as_object().expect("meta object");

        assert_eq!(
            declared_openai_file_params(Some(meta)),
            vec!["file".to_string(), "attachments".to_string()]
        );
        assert_eq!(
            declared_openai_file_outputs(Some(meta)),
            vec!["output".to_string()]
        );
    }

    #[test]
    fn mask_input_schema_for_model_rewrites_scalar_and_array_fields() {
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "object",
                    "description": "Original file payload."
                },
                "files": {
                    "type": "array",
                    "items": {"type": "object"}
                }
            }
        });

        mask_input_schema_for_model(&mut schema, &["file".to_string(), "files".to_string()]);

        assert_eq!(
            schema,
            serde_json::json!({
                "type": "object",
                "properties": {
                    "file": {
                        "type": "string",
                        "description": "Original file payload. Pass a local file path string. Codex will upload it before invoking the tool."
                    },
                    "files": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Pass a local file path string. Codex will upload it before invoking the tool."
                    }
                }
            })
        );
    }

    #[test]
    fn mask_output_schema_for_model_rewrites_declared_output_fields() {
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": {
                "outputFile": {
                    "type": "object"
                }
            }
        });

        mask_output_schema_for_model(&mut schema, &["outputFile".to_string()]);

        assert_eq!(
            schema,
            serde_json::json!({
                "type": "object",
                "properties": {
                    "outputFile": {
                        "type": "string",
                        "description": "This field returns a local temp file path after Codex auto-downloads supported OpenAI file handles."
                    }
                }
            })
        );
    }
}
