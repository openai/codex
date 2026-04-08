//! Bridges Apps SDK-style `openai/fileParams` metadata into Codex's MCP flow.
//!
//! Strategy:
//! - Inspect `_meta["openai/fileParams"]` to discover which tool arguments are
//!   file inputs.
//! - Rewrite model-visible schemas for those arguments from provider file
//!   payloads into absolute local file path strings, so the model can point
//!   Codex at local files.
//! - At tool execution time, upload those local files to OpenAI file storage
//!   and rewrite only the declared arguments into the provided-file payload
//!   shape expected by the downstream Apps tool.
//!
//! This keeps the generic MCP dispatch path simple: callers only pass in the
//! declared file-param names, and this module owns the feature-specific schema
//! masking and argument rewriting.

use crate::codex::Session;
use crate::codex::TurnContext;
use codex_api::CoreAuthProvider;
use codex_api::upload_local_file;
use codex_login::CodexAuth;
use serde_json::Map;
use serde_json::Value as JsonValue;

const META_OPENAI_FILE_PARAMS: &str = "openai/fileParams";

pub(crate) fn declared_openai_file_input_param_names(
    meta: Option<&Map<String, JsonValue>>,
) -> Vec<String> {
    declared_field_names(meta, META_OPENAI_FILE_PARAMS)
}

pub(crate) fn mask_input_schema_for_file_path_params(
    input_schema: &mut JsonValue,
    file_params: &[String],
) {
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
        mask_input_property_schema(property_schema);
    }
}

pub(crate) fn mask_model_visible_tool_input_schema(tool: &mut rmcp::model::Tool) {
    let file_params = declared_openai_file_input_param_names(tool.meta.as_deref());
    if file_params.is_empty() {
        return;
    }

    let mut input_schema = JsonValue::Object(tool.input_schema.as_ref().clone());
    mask_input_schema_for_file_path_params(&mut input_schema, &file_params);
    if let JsonValue::Object(input_schema) = input_schema {
        tool.input_schema = std::sync::Arc::new(input_schema);
    }
}

pub(crate) async fn rewrite_mcp_tool_arguments_for_openai_files(
    sess: &Session,
    turn_context: &TurnContext,
    arguments_value: Option<JsonValue>,
    openai_file_input_params: Option<&[String]>,
) -> Result<Option<JsonValue>, String> {
    let Some(openai_file_input_params) = openai_file_input_params else {
        return Ok(arguments_value);
    };

    let Some(arguments_value) = arguments_value else {
        return Ok(None);
    };
    let Some(arguments) = arguments_value.as_object() else {
        return Ok(Some(arguments_value));
    };
    let auth = sess.services.auth_manager.auth().await;
    let mut rewritten_arguments = arguments.clone();

    for field_name in openai_file_input_params {
        let Some(value) = arguments.get(field_name) else {
            continue;
        };
        let Some(uploaded_value) =
            rewrite_argument_value_for_openai_files(turn_context, auth.as_ref(), field_name, value)
                .await?
        else {
            continue;
        };
        rewritten_arguments.insert(field_name.clone(), uploaded_value);
    }

    if rewritten_arguments == *arguments {
        return Ok(Some(arguments_value));
    }

    Ok(Some(JsonValue::Object(rewritten_arguments)))
}

fn declared_field_names(meta: Option<&Map<String, JsonValue>>, key: &str) -> Vec<String> {
    let Some(meta) = meta else {
        return Vec::new();
    };

    meta.get(key)
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter_map(JsonValue::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

async fn rewrite_argument_value_for_openai_files(
    turn_context: &TurnContext,
    auth: Option<&CodexAuth>,
    field_name: &str,
    value: &JsonValue,
) -> Result<Option<JsonValue>, String> {
    match value {
        JsonValue::String(path_or_file_ref) => {
            let rewritten = build_uploaded_local_argument_value(
                turn_context,
                auth,
                field_name,
                /*index*/ None,
                path_or_file_ref,
            )
            .await?;
            Ok(Some(rewritten))
        }
        JsonValue::Array(values) => {
            let mut rewritten_values = Vec::with_capacity(values.len());
            for (index, item) in values.iter().enumerate() {
                let Some(path_or_file_ref) = item.as_str() else {
                    return Ok(None);
                };
                let rewritten = build_uploaded_local_argument_value(
                    turn_context,
                    auth,
                    field_name,
                    Some(index),
                    path_or_file_ref,
                )
                .await?;
                rewritten_values.push(rewritten);
            }
            Ok(Some(JsonValue::Array(rewritten_values)))
        }
        _ => Ok(None),
    }
}

async fn build_uploaded_local_argument_value(
    turn_context: &TurnContext,
    auth: Option<&CodexAuth>,
    field_name: &str,
    index: Option<usize>,
    file_path: &str,
) -> Result<JsonValue, String> {
    let resolved_path = turn_context.resolve_path(Some(file_path.to_string()));
    let Some(auth) = auth else {
        return Err(
            "ChatGPT auth is required to upload local files for Codex Apps tools".to_string(),
        );
    };
    let token_data = auth
        .get_token_data()
        .map_err(|error| format!("failed to read ChatGPT auth for file upload: {error}"))?;
    let upload_auth = CoreAuthProvider {
        token: Some(token_data.access_token),
        account_id: token_data.account_id,
    };
    let uploaded = upload_local_file(
        turn_context.config.chatgpt_base_url.trim_end_matches('/'),
        &upload_auth,
        &resolved_path,
    )
    .await
    .map_err(|error| match index {
        Some(index) => {
            format!("failed to upload `{file_path}` for `{field_name}[{index}]`: {error}")
        }
        None => format!("failed to upload `{file_path}` for `{field_name}`: {error}"),
    })?;
    Ok(serde_json::json!({
        "downloadUrl": uploaded.download_url,
        "fileId": uploaded.file_id,
        "mimeType": uploaded.mime_type,
        "fileName": uploaded.file_name,
        "uri": uploaded.uri,
        "fileSizeBytes": uploaded.file_size_bytes,
    }))
}

fn mask_input_property_schema(schema: &mut JsonValue) {
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
mod tests {
    use super::*;
    use crate::codex::make_session_and_context;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn declared_openai_file_fields_treat_names_literally() {
        let meta = serde_json::json!({
            "openai/fileParams": ["file", "input_file", "attachments"],
            "openai/fileOutputs": ["output", "artifact"]
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
    fn mask_input_schema_for_file_path_params_rewrites_scalar_and_array_fields() {
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

        mask_input_schema_for_file_path_params(
            &mut schema,
            &["file".to_string(), "files".to_string()],
        );

        assert_eq!(
            schema,
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
        );
    }

    #[test]
    fn mask_model_visible_tool_input_schema_leaves_tool_unchanged_without_declared_params() {
        let original = rmcp::model::Tool {
            name: "echo".into(),
            title: None,
            description: None,
            input_schema: std::sync::Arc::new(
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "file": {
                            "type": "object"
                        }
                    }
                })
                .as_object()
                .expect("object")
                .clone(),
            ),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        };
        let mut tool = original.clone();

        mask_model_visible_tool_input_schema(&mut tool);

        assert_eq!(tool, original);
    }

    #[tokio::test]
    async fn openai_file_argument_rewrite_requires_declared_file_params() {
        let (session, turn_context) = make_session_and_context().await;
        let arguments = Some(serde_json::json!({
            "file": "/tmp/codex-smoke-file.txt"
        }));

        let rewritten = rewrite_mcp_tool_arguments_for_openai_files(
            &session,
            &Arc::new(turn_context),
            arguments.clone(),
            /*openai_file_input_params*/ None,
        )
        .await
        .expect("rewrite should succeed");

        assert_eq!(rewritten, arguments);
    }

    #[tokio::test]
    async fn build_uploaded_local_argument_value_uploads_local_file_path() {
        use wiremock::Mock;
        use wiremock::MockServer;
        use wiremock::ResponseTemplate;
        use wiremock::matchers::body_json;
        use wiremock::matchers::header;
        use wiremock::matchers::method;
        use wiremock::matchers::path;

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/backend-api/files"))
            .and(header("chatgpt-account-id", "account_id"))
            .and(body_json(serde_json::json!({
                "file_name": "file_report.csv",
                "file_size": 5,
                "use_case": "codex",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "file_id": "file_123",
                "upload_url": format!("{}/upload/file_123", server.uri()),
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/upload/file_123"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/backend-api/files/file_123/uploaded"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "download_url": format!("{}/download/file_123", server.uri()),
                "file_name": "file_report.csv",
                "mime_type": "text/csv",
                "file_size_bytes": 5,
            })))
            .expect(1)
            .mount(&server)
            .await;

        let (_, mut turn_context) = make_session_and_context().await;
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        let dir = tempdir().expect("temp dir");
        let local_path = dir.path().join("file_report.csv");
        tokio::fs::write(&local_path, b"hello")
            .await
            .expect("write local file");
        turn_context.cwd = AbsolutePathBuf::try_from(dir.path()).expect("absolute path");

        let mut config = (*turn_context.config).clone();
        config.chatgpt_base_url = format!("{}/backend-api", server.uri());
        turn_context.config = Arc::new(config);

        let rewritten = build_uploaded_local_argument_value(
            &turn_context,
            Some(&auth),
            "file",
            /*index*/ None,
            "file_report.csv",
        )
        .await
        .expect("rewrite should upload the local file");

        assert_eq!(
            rewritten,
            serde_json::json!({
                "downloadUrl": format!("{}/download/file_123", server.uri()),
                "fileId": "file_123",
                "mimeType": "text/csv",
                "fileName": "file_report.csv",
                "uri": "sediment://file_123",
                "fileSizeBytes": 5,
            })
        );
    }

    #[tokio::test]
    async fn rewrite_argument_value_for_openai_files_rewrites_scalar_path() {
        use wiremock::Mock;
        use wiremock::MockServer;
        use wiremock::ResponseTemplate;
        use wiremock::matchers::body_json;
        use wiremock::matchers::header;
        use wiremock::matchers::method;
        use wiremock::matchers::path;

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/backend-api/files"))
            .and(header("chatgpt-account-id", "account_id"))
            .and(body_json(serde_json::json!({
                "file_name": "file_report.csv",
                "file_size": 5,
                "use_case": "codex",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "file_id": "file_123",
                "upload_url": format!("{}/upload/file_123", server.uri()),
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/upload/file_123"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/backend-api/files/file_123/uploaded"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "download_url": format!("{}/download/file_123", server.uri()),
                "file_name": "file_report.csv",
                "mime_type": "text/csv",
                "file_size_bytes": 5,
            })))
            .expect(1)
            .mount(&server)
            .await;

        let (_, mut turn_context) = make_session_and_context().await;
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        let dir = tempdir().expect("temp dir");
        let local_path = dir.path().join("file_report.csv");
        tokio::fs::write(&local_path, b"hello")
            .await
            .expect("write local file");
        turn_context.cwd = AbsolutePathBuf::try_from(dir.path()).expect("absolute path");

        let mut config = (*turn_context.config).clone();
        config.chatgpt_base_url = format!("{}/backend-api", server.uri());
        turn_context.config = Arc::new(config);
        let rewritten = rewrite_argument_value_for_openai_files(
            &turn_context,
            Some(&auth),
            "file",
            &serde_json::json!("file_report.csv"),
        )
        .await
        .expect("rewrite should succeed");

        assert_eq!(
            rewritten,
            Some(serde_json::json!({
                "downloadUrl": format!("{}/download/file_123", server.uri()),
                "fileId": "file_123",
                "mimeType": "text/csv",
                "fileName": "file_report.csv",
                "uri": "sediment://file_123",
                "fileSizeBytes": 5,
            }))
        );
    }

    #[tokio::test]
    async fn rewrite_argument_value_for_openai_files_rewrites_array_paths() {
        use wiremock::Mock;
        use wiremock::MockServer;
        use wiremock::ResponseTemplate;
        use wiremock::matchers::body_json;
        use wiremock::matchers::header;
        use wiremock::matchers::method;
        use wiremock::matchers::path;

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/backend-api/files"))
            .and(header("chatgpt-account-id", "account_id"))
            .and(body_json(serde_json::json!({
                "file_name": "one.csv",
                "file_size": 3,
                "use_case": "codex",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "file_id": "file_1",
                "upload_url": format!("{}/upload/file_1", server.uri()),
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/backend-api/files"))
            .and(header("chatgpt-account-id", "account_id"))
            .and(body_json(serde_json::json!({
                "file_name": "two.csv",
                "file_size": 3,
                "use_case": "codex",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "file_id": "file_2",
                "upload_url": format!("{}/upload/file_2", server.uri()),
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/upload/file_1"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/upload/file_2"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/backend-api/files/file_1/uploaded"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "download_url": format!("{}/download/file_1", server.uri()),
                "file_name": "one.csv",
                "mime_type": "text/csv",
                "file_size_bytes": 3,
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/backend-api/files/file_2/uploaded"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "download_url": format!("{}/download/file_2", server.uri()),
                "file_name": "two.csv",
                "mime_type": "text/csv",
                "file_size_bytes": 3,
            })))
            .expect(1)
            .mount(&server)
            .await;

        let (_, mut turn_context) = make_session_and_context().await;
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        let dir = tempdir().expect("temp dir");
        tokio::fs::write(dir.path().join("one.csv"), b"one")
            .await
            .expect("write first local file");
        tokio::fs::write(dir.path().join("two.csv"), b"two")
            .await
            .expect("write second local file");
        turn_context.cwd = AbsolutePathBuf::try_from(dir.path()).expect("absolute path");

        let mut config = (*turn_context.config).clone();
        config.chatgpt_base_url = format!("{}/backend-api", server.uri());
        turn_context.config = Arc::new(config);
        let rewritten = rewrite_argument_value_for_openai_files(
            &turn_context,
            Some(&auth),
            "files",
            &serde_json::json!(["one.csv", "two.csv"]),
        )
        .await
        .expect("rewrite should succeed");

        assert_eq!(
            rewritten,
            Some(serde_json::json!([
                {
                    "downloadUrl": format!("{}/download/file_1", server.uri()),
                    "fileId": "file_1",
                    "mimeType": "text/csv",
                    "fileName": "one.csv",
                    "uri": "sediment://file_1",
                    "fileSizeBytes": 3,
                },
                {
                    "downloadUrl": format!("{}/download/file_2", server.uri()),
                    "fileId": "file_2",
                    "mimeType": "text/csv",
                    "fileName": "two.csv",
                    "uri": "sediment://file_2",
                    "fileSizeBytes": 3,
                }
            ]))
        );
    }

    #[tokio::test]
    async fn rewrite_mcp_tool_arguments_for_openai_files_surfaces_upload_failures() {
        let (mut session, turn_context) = make_session_and_context().await;
        session.services.auth_manager = crate::test_support::auth_manager_from_auth(
            CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        );
        let error = rewrite_mcp_tool_arguments_for_openai_files(
            &session,
            &turn_context,
            Some(serde_json::json!({
                "file": "/definitely/missing/file.csv",
            })),
            Some(&["file".to_string()]),
        )
        .await
        .expect_err("missing file should fail");

        assert!(error.contains("failed to upload"));
        assert!(error.contains("file"));
    }
}
