use super::*;
use pretty_assertions::assert_eq;
use rmcp::model::Meta;
use rmcp::model::ServerCapabilities;

fn tool(input_schema: JsonValue, meta: Option<JsonValue>) -> Tool {
    let mut tool = Tool::new(
        "upload",
        "upload",
        input_schema.as_object().unwrap().clone(),
    );
    tool.meta = meta.map(|meta| Meta(meta.as_object().unwrap().clone()));
    tool
}

#[test]
fn parses_and_merges_mcp_and_openai_file_inputs() {
    let tool = tool(
        serde_json::json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "x-mcp-file": {
                        "accept": ["text/plain"],
                        "maxSize": 42,
                        "transferModes": ["inline", "upload"]
                    }
                }
            }
        }),
        Some(serde_json::json!({"openai/fileParams": ["file", "legacy"]})),
    );

    assert_eq!(
        file_input_specs(&tool),
        vec![
            FileInputSpec {
                path: "file".to_string(),
                accepts: vec!["text/plain".to_string()],
                max_size: Some(42),
                transfer_modes: BTreeSet::from([
                    FileTransferMode::Inline,
                    FileTransferMode::Upload,
                ]),
                sources: BTreeSet::from([FileInputSource::Mcp, FileInputSource::OpenAiFileParams,]),
                is_array: false,
            },
            FileInputSpec {
                path: "legacy".to_string(),
                accepts: Vec::new(),
                max_size: None,
                transfer_modes: BTreeSet::new(),
                sources: BTreeSet::from([FileInputSource::OpenAiFileParams]),
                is_array: false,
            },
        ]
    );
}

#[test]
fn missing_transfer_modes_defaults_to_inline() {
    let tool = tool(
        serde_json::json!({
            "type": "object",
            "properties": {"file": {"x-mcp-file": {}}}
        }),
        /*meta*/ None,
    );
    assert_eq!(
        file_input_specs(&tool)[0].transfer_modes,
        BTreeSet::from([FileTransferMode::Inline])
    );
}

#[test]
fn malformed_extension_values_are_ignored_and_arrays_are_preserved() {
    let tool = tool(
        serde_json::json!({
            "type": "object",
            "properties": {
                "files": {
                    "type": "array",
                    "items": {"type": "string"},
                    "x-mcp-file": {
                        "accept": ["text/plain", 7, ""],
                        "maxSize": -1,
                        "transferModes": ["upload", "future-mode", 7]
                    }
                }
            }
        }),
        /*meta*/ None,
    );

    assert_eq!(
        file_input_specs(&tool),
        vec![FileInputSpec {
            path: "files".to_string(),
            accepts: vec!["text/plain".to_string()],
            max_size: None,
            transfer_modes: BTreeSet::from([FileTransferMode::Upload]),
            sources: BTreeSet::from([FileInputSource::Mcp]),
            is_array: true,
        }]
    );
    let masked = tool_with_file_input_schema(
        &tool, /*honor_openai_file_params*/ false, /*mcp_file_transfer_enabled*/ true,
    );
    assert_eq!(
        masked.input_schema["properties"]["files"]["items"],
        serde_json::json!({"type": "string"})
    );
}

#[test]
fn mcp_schema_masking_is_gated_while_legacy_masking_is_not() {
    let tool = tool(
        serde_json::json!({
            "type": "object",
            "properties": {
                "mcp": {"type": "object", "x-mcp-file": {}},
                "legacy": {"type": "object"}
            }
        }),
        Some(serde_json::json!({"openai/fileParams": ["legacy"]})),
    );

    let disabled = tool_with_file_input_schema(
        &tool, /*honor_openai_file_params*/ true, /*mcp_file_transfer_enabled*/ false,
    );
    let disabled_properties = disabled.input_schema["properties"].as_object().unwrap();
    assert_eq!(disabled_properties["mcp"]["type"], "object");
    assert_eq!(disabled_properties["legacy"]["type"], "string");

    let enabled = tool_with_file_input_schema(
        &tool, /*honor_openai_file_params*/ true, /*mcp_file_transfer_enabled*/ true,
    );
    let enabled_properties = enabled.input_schema["properties"].as_object().unwrap();
    assert_eq!(enabled_properties["mcp"]["type"], "string");
    assert_eq!(enabled_properties["legacy"]["type"], "string");
}

#[test]
fn file_capabilities_use_sep_extension_when_available() {
    let mut capabilities = ServerCapabilities::default();
    capabilities.extensions = Some(std::collections::BTreeMap::from([(
        "io.modelcontextprotocol/files".to_string(),
        serde_json::json!({
            "prepareUpload": true,
            "completeUpload": true,
            "getDownload": false
        })
        .as_object()
        .expect("object")
        .clone(),
    )]));

    assert_eq!(
        McpFileCapabilities::from_server_and_tools(&capabilities, &[]),
        McpFileCapabilities {
            prepare_upload: true,
            complete_upload: true,
            get_download: false,
        }
    );
}

#[test]
fn file_capabilities_infer_openai_compatibility_from_upload_schema() {
    let tool = tool(
        serde_json::json!({
            "type": "object",
            "properties": {
                "file": {"x-mcp-file": {"transferModes": ["upload"]}}
            }
        }),
        /*meta*/ None,
    );
    let tool = crate::ToolInfo {
        server_name: "server".to_string(),
        supports_parallel_tool_calls: false,
        server_origin: None,
        callable_name: "upload".to_string(),
        callable_namespace: "server".to_string(),
        namespace_description: None,
        tool,
        connector_id: None,
        connector_name: None,
        plugin_display_names: Vec::new(),
    };

    assert_eq!(
        McpFileCapabilities::from_server_and_tools(&ServerCapabilities::default(), &[tool]),
        McpFileCapabilities {
            prepare_upload: true,
            complete_upload: true,
            get_download: true,
        }
    );
}

#[test]
fn transfer_descriptors_accept_draft_and_extended_shapes() {
    assert_eq!(
        serde_json::from_value::<FileTransferDescriptor>(serde_json::json!({
            "method": "PUT",
            "url": "https://example.com/upload"
        }))
        .expect("draft descriptor"),
        FileTransferDescriptor {
            transport: None,
            method: "PUT".to_string(),
            url: "https://example.com/upload".to_string(),
            expires_at: None,
        }
    );
    assert_eq!(
        serde_json::from_value::<FileTransferDescriptor>(serde_json::json!({
            "transport": "https",
            "method": "GET",
            "url": "https://example.com/download",
            "expiresAt": "2030-01-01T00:00:00Z"
        }))
        .expect("extended descriptor"),
        FileTransferDescriptor {
            transport: Some("https".to_string()),
            method: "GET".to_string(),
            url: "https://example.com/download".to_string(),
            expires_at: Some("2030-01-01T00:00:00Z".to_string()),
        }
    );
}
