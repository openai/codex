use std::collections::HashMap;
use std::sync::Arc;

use pretty_assertions::assert_eq;
use rmcp::model::JsonObject;
use rmcp::model::Meta;
use rmcp::model::Tool;

use super::*;
use crate::tools::ToolInfo;

fn tool_info(tool: Tool) -> ToolInfo {
    ToolInfo {
        server_name: "codex_apps".to_string(),
        supports_parallel_tool_calls: false,
        server_origin: None,
        callable_name: tool.name.to_string(),
        callable_namespace: "codex_apps".to_string(),
        namespace_description: None,
        tool,
        openai_file_input_optional_fields: HashMap::new(),
        connector_id: None,
        connector_name: None,
        plugin_display_names: Vec::new(),
    }
}

fn test_tool(name: &str) -> Tool {
    Tool::new(
        name.to_string(),
        format!("Test tool: {name}"),
        Arc::new(JsonObject::default()),
    )
}

#[test]
fn declared_openai_file_fields_treat_names_literally() {
    let meta = serde_json::json!({
        "openai/fileParams": ["file", "input_file", "attachments"]
    });
    let meta = meta.as_object().expect("meta object");

    assert_eq!(
        declared_openai_file_input_param_names(Some(meta)),
        vec![
            "file".to_string(),
            "input_file".to_string(),
            "attachments".to_string(),
        ]
    );
}

#[test]
fn prepare_openai_file_params_for_model_masks_file_params() {
    let mut tool = test_tool("upload");
    tool.input_schema = Arc::new(
        serde_json::json!({
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
        })
        .as_object()
        .expect("object")
        .clone(),
    );
    tool.meta = Some(Meta(
        serde_json::json!({
            "openai/fileParams": ["file", "files"]
        })
        .as_object()
        .expect("object")
        .clone(),
    ));
    let mut tool_info = tool_info(tool);

    prepare_openai_file_params_for_model(&mut tool_info);

    assert_eq!(
        *tool_info.tool.input_schema,
        serde_json::json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Original file payload. This parameter expects an absolute local file path. If you want to upload a file, provide the absolute path to that file here."
                },
                "files": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "This parameter expects an absolute local file path. If you want to upload a file, provide the absolute path to that file here."
                }
            }
        })
        .as_object()
        .expect("object")
        .clone()
    );
}

#[test]
fn prepare_openai_file_params_for_model_derives_supported_optional_fields() {
    let mut tool = Tool::new(
        "upload".to_string(),
        "Upload files".to_string(),
        Arc::new(
            serde_json::json!({
                "type": "object",
                "$defs": {
                    "Rich/File": {
                        "type": "object",
                        "properties": {
                            "download_url": {"type": "string"},
                            "file_id": {"type": "string"},
                            "file_name": {"type": "string"}
                        },
                        "additionalProperties": false
                    }
                },
                "properties": {
                    "photoshop_image": {
                        "type": "object",
                        "properties": {
                            "download_url": {"type": "string"},
                            "file_id": {"type": "string"}
                        },
                        "additionalProperties": false
                    },
                    "drive_import": {
                        "type": "object",
                        "properties": {
                            "download_url": {"type": "string"},
                            "file_id": {"type": "string"},
                            "mime_type": {"type": "string"},
                            "file_name": {"type": "string"}
                        },
                        "additionalProperties": false
                    },
                    "attachments": {
                        "anyOf": [
                            {
                                "type": "array",
                                "items": {
                                    "oneOf": [
                                        {
                                            "allOf": [
                                                {
                                                    "type": "object",
                                                    "properties": {
                                                        "download_url": {"type": "string"},
                                                        "file_id": {"type": "string"}
                                                    }
                                                },
                                                {
                                                    "type": "object",
                                                    "properties": {
                                                        "mime_type": {"type": "string"}
                                                    }
                                                }
                                            ]
                                        },
                                        {"type": "null"}
                                    ]
                                }
                            },
                            {"type": "null"}
                        ]
                    },
                    "referenced_file": {
                        "$ref": "#/$defs/Rich~1File"
                    },
                    "custom_file": {
                        "type": "object",
                        "properties": {
                            "download_url": {"type": "string"},
                            "file_id": {"type": "string"},
                            "mime_type": {"type": "string"},
                            "uri": {"type": "string"}
                        },
                        "additionalProperties": false
                    },
                    "open_file": {
                        "type": "object",
                        "properties": {
                            "download_url": {"type": "string"},
                            "file_id": {"type": "string"}
                        }
                    },
                    "explicitly_open_file": {
                        "type": "object",
                        "properties": {
                            "download_url": {"type": "string"},
                            "file_id": {"type": "string"}
                        },
                        "additionalProperties": true
                    },
                    "items_only_files": {
                        "items": {
                            "type": "object",
                            "properties": {
                                "download_url": {"type": "string"},
                                "file_id": {"type": "string"},
                                "file_name": {"type": "string"}
                            },
                            "additionalProperties": false
                        }
                    }
                }
            })
            .as_object()
            .expect("object")
            .clone(),
        ),
    );
    tool.meta = Some(Meta(
        serde_json::json!({
            "openai/fileParams": [
                "photoshop_image",
                "drive_import",
                "attachments",
                "referenced_file",
                "custom_file",
                "open_file",
                "explicitly_open_file",
                "items_only_files",
                "missing_file"
            ]
        })
        .as_object()
        .expect("object")
        .clone(),
    ));
    let mut tool_info = tool_info(tool);

    prepare_openai_file_params_for_model(&mut tool_info);

    assert_eq!(
        tool_info.openai_file_input_optional_fields,
        HashMap::from([
            ("photoshop_image".to_string(), Vec::new()),
            (
                "drive_import".to_string(),
                vec!["mime_type".to_string(), "file_name".to_string()]
            ),
            (
                "attachments".to_string(),
                vec!["mime_type".to_string(), "file_name".to_string()]
            ),
            ("referenced_file".to_string(), vec!["file_name".to_string()]),
            ("custom_file".to_string(), vec!["mime_type".to_string()]),
            (
                "open_file".to_string(),
                vec!["mime_type".to_string(), "file_name".to_string()]
            ),
            (
                "explicitly_open_file".to_string(),
                vec!["mime_type".to_string(), "file_name".to_string()]
            ),
            (
                "items_only_files".to_string(),
                vec!["file_name".to_string()]
            ),
            ("missing_file".to_string(), Vec::new()),
        ])
    );
}

#[test]
fn prepare_openai_file_params_for_model_leaves_tools_without_file_params_unchanged() {
    let original_tool = test_tool("upload");
    let mut tool_info = tool_info(original_tool.clone());

    prepare_openai_file_params_for_model(&mut tool_info);

    assert_eq!(tool_info.tool, original_tool);
    assert!(tool_info.openai_file_input_optional_fields.is_empty());
}
