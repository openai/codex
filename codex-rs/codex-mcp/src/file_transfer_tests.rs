use super::*;
use pretty_assertions::assert_eq;
use rmcp::model::Meta;

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
fn missing_transfer_modes_allows_inline_and_upload() {
    let tool = tool(
        serde_json::json!({
            "type": "object",
            "properties": {"file": {"x-mcp-file": {}}}
        }),
        /*meta*/ None,
    );
    assert_eq!(
        file_input_specs(&tool)[0].transfer_modes,
        BTreeSet::from([FileTransferMode::Inline, FileTransferMode::Upload])
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
                    "items": {
                        "type": "string",
                        "format": "uri",
                        "x-mcp-file": {
                            "accept": ["text/plain", 7, ""],
                            "maxSize": -1,
                            "transferModes": ["upload", "future-mode", 7]
                        }
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
fn transfer_descriptors_accept_sep_shape() {
    assert_eq!(
        serde_json::from_value::<FileTransferDescriptor>(serde_json::json!({
            "transport": "https",
            "method": "GET",
            "url": "https://example.com/download",
            "headers": {"Authorization": "Bearer secret"},
            "multipart": {
                "fileField": "payload",
                "fields": {"token": "abc123"}
            },
            "expiresAt": "2030-01-01T00:00:00Z"
        }))
        .expect("extended descriptor"),
        FileTransferDescriptor {
            transport: "https".to_string(),
            method: "GET".to_string(),
            url: "https://example.com/download".to_string(),
            headers: BTreeMap::from([("Authorization".to_string(), "Bearer secret".to_string(),)]),
            multipart: Some(FileTransferMultipart {
                file_field: "payload".to_string(),
                fields: BTreeMap::from([("token".to_string(), "abc123".to_string())]),
            }),
            expires_at: Some("2030-01-01T00:00:00Z".to_string()),
        }
    );
}

#[test]
fn authorize_upload_params_match_sep_wire_shape() {
    assert_eq!(
        serde_json::to_value(AuthorizeUploadParams {
            name: "report.pdf".to_string(),
            mime_type: "application/pdf".to_string(),
            size: 248_123,
            digest: Some(FileDigest {
                algorithm: "sha-256".to_string(),
                value: "digest-value".to_string(),
            }),
        })
        .expect("serialize params"),
        serde_json::json!({
            "name": "report.pdf",
            "mimeType": "application/pdf",
            "size": 248123,
            "digest": {"algorithm": "sha-256", "value": "digest-value"}
        })
    );
}

#[test]
fn authorize_results_match_sep_wire_shapes() {
    let file = serde_json::json!({
        "uri": "mcp-file://server/file-1",
        "name": "report.pdf",
        "mimeType": "application/pdf",
        "size": 248123,
        "digest": {"algorithm": "sha-256", "value": "digest-value"}
    });
    let upload = serde_json::json!({
        "transport": "https",
        "method": "POST",
        "url": "https://upload.example.com/file-1",
        "headers": {"X-Upload": "token"},
        "multipart": {"fileField": "file", "fields": {"token": "abc123"}},
        "expiresAt": "2030-01-01T00:00:00Z"
    });
    let download = serde_json::json!({
        "transport": "https",
        "method": "GET",
        "url": "https://download.example.com/file-1"
    });

    let result: AuthorizeUploadResult = serde_json::from_value(serde_json::json!({
        "file": file,
        "upload": upload,
        "download": download
    }))
    .expect("authorize upload result");
    assert_eq!(
        result,
        AuthorizeUploadResult {
            file: serde_json::from_value(file.clone()).expect("file value"),
            upload: serde_json::from_value(upload).expect("upload descriptor"),
            download: Some(serde_json::from_value(download.clone()).expect("download descriptor")),
        }
    );

    let result: AuthorizeDownloadResult = serde_json::from_value(serde_json::json!({
        "file": file,
        "download": download
    }))
    .expect("authorize download result");
    assert_eq!(
        result,
        AuthorizedFile {
            file: serde_json::from_value(file).expect("file value"),
            download: Some(serde_json::from_value(download).expect("download descriptor")),
        }
    );
}
