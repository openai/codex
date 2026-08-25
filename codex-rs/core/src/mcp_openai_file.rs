//! Bridges Apps SDK-style `openai/fileParams` metadata into Codex's MCP flow.
//!
//! Strategy:
//! - Inspect `_meta["openai/fileParams"]` to discover which tool arguments are
//!   file inputs.
//! - At tool execution time, read those files from the primary environment,
//!   upload them to OpenAI file storage,
//!   and rewrite only the declared arguments into the provided-file payload
//!   shape expected by the downstream Apps tool.
//!
//! The model-facing local-path schema is owned by `codex-mcp` alongside MCP tool inventory, so this
//! module only handles uploading the files and rewriting the execution-time arguments.

use crate::session::session::Session;
use crate::session::step_context::StepContext;
use codex_api::HostedFileUploadContext;
use codex_api::OPENAI_FILE_UPLOAD_LIMIT_BYTES;
use codex_api::upload_openai_file;
use codex_exec_server::GetMetadataOptions;
use codex_login::CodexAuth;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_sandboxing::policy_transforms::effective_file_system_sandbox_policy;
use codex_sandboxing::policy_transforms::merge_permission_profiles;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

struct FileArgumentLocation<'a> {
    field_name: &'a str,
    index: Option<usize>,
}

pub(crate) async fn rewrite_mcp_tool_arguments_for_openai_files(
    sess: &Session,
    step_context: &StepContext,
    arguments_value: Option<JsonValue>,
    openai_file_input_optional_fields: Option<&HashMap<String, Vec<String>>>,
    hosted_upload: Option<&HostedFileUploadContext>,
) -> Result<Option<JsonValue>, String> {
    let Some(openai_file_input_optional_fields) = openai_file_input_optional_fields else {
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

    for (field_name, optional_fields) in openai_file_input_optional_fields {
        let Some(value) = arguments.get(field_name) else {
            continue;
        };
        let Some(uploaded_value) = rewrite_argument_value_for_openai_files(
            sess,
            step_context,
            auth.as_ref(),
            field_name,
            optional_fields,
            value,
            hosted_upload,
        )
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

async fn rewrite_argument_value_for_openai_files(
    sess: &Session,
    step_context: &StepContext,
    auth: Option<&CodexAuth>,
    field_name: &str,
    optional_fields: &[String],
    value: &JsonValue,
    hosted_upload: Option<&HostedFileUploadContext>,
) -> Result<Option<JsonValue>, String> {
    match value {
        JsonValue::String(file_path) => {
            let rewritten = build_uploaded_argument_value(
                sess,
                step_context,
                auth,
                FileArgumentLocation {
                    field_name,
                    index: None,
                },
                optional_fields,
                file_path,
                hosted_upload,
            )
            .await?;
            Ok(Some(rewritten))
        }
        JsonValue::Array(values) => {
            let mut rewritten_values = Vec::with_capacity(values.len());
            for (index, item) in values.iter().enumerate() {
                let Some(file_path) = item.as_str() else {
                    return Ok(None);
                };
                let rewritten = build_uploaded_argument_value(
                    sess,
                    step_context,
                    auth,
                    FileArgumentLocation {
                        field_name,
                        index: Some(index),
                    },
                    optional_fields,
                    file_path,
                    hosted_upload,
                )
                .await?;
                rewritten_values.push(rewritten);
            }
            Ok(Some(JsonValue::Array(rewritten_values)))
        }
        _ => Ok(None),
    }
}

async fn build_uploaded_argument_value(
    sess: &Session,
    step_context: &StepContext,
    auth: Option<&CodexAuth>,
    argument: FileArgumentLocation<'_>,
    optional_fields: &[String],
    file_path: &str,
    hosted_upload: Option<&HostedFileUploadContext>,
) -> Result<JsonValue, String> {
    let FileArgumentLocation { field_name, index } = argument;
    let contextualize_error = |error: String| match index {
        Some(index) => {
            format!("failed to upload `{file_path}` for `{field_name}[{index}]`: {error}")
        }
        None => format!("failed to upload `{file_path}` for `{field_name}`: {error}"),
    };
    let Some(auth) = auth else {
        return Err("ChatGPT auth is required to upload files for Codex Apps tools".to_string());
    };
    if !auth.uses_codex_backend() {
        return Err("ChatGPT auth is required to upload files for Codex Apps tools".to_string());
    }
    let turn_context = &step_context.turn;
    let Some(turn_environment) = step_context.environments.primary() else {
        return Err(contextualize_error(
            "no primary turn environment is available".to_string(),
        ));
    };
    let path_uri = turn_environment
        .cwd()
        .join(file_path)
        .map_err(|error| contextualize_error(error.to_string()))?;
    let additional_permissions = merge_permission_profiles(
        sess.granted_session_permissions(&turn_environment.selection.environment_id)
            .await
            .as_ref(),
        sess.granted_turn_permissions(&turn_environment.selection.environment_id)
            .await
            .as_ref(),
    );
    let file_system_policy = effective_file_system_sandbox_policy(
        &turn_environment
            .permission_profile()
            .file_system_sandbox_policy(),
        additional_permissions.as_ref(),
    );
    let requires_sandbox = !file_system_policy.has_full_disk_read_access()
        || file_system_policy
            .entries
            .iter()
            .any(|entry| entry.access == FileSystemAccessMode::Deny);
    let sandbox =
        requires_sandbox.then(|| turn_environment.sandbox_context(additional_permissions));
    if sandbox.is_some() {
        let environment_info = turn_environment
            .environment
            .info()
            .await
            .map_err(|error| contextualize_error(error.to_string()))?;
        if !environment_info.capabilities.sandboxed_file_streaming {
            return Err(contextualize_error(
                "selected executor does not support sandboxed file streaming".to_string(),
            ));
        }
    }
    let fs = turn_environment.environment.get_filesystem();
    let metadata = fs
        .get_metadata(&path_uri, GetMetadataOptions::default(), sandbox.as_ref())
        .await
        .map_err(|error| contextualize_error(error.to_string()))?;
    if !metadata.is_file {
        return Err(contextualize_error(format!(
            "path `{}` is not a file",
            path_uri.inferred_native_path_string()
        )));
    }
    if metadata.size > OPENAI_FILE_UPLOAD_LIMIT_BYTES {
        return Err(contextualize_error(format!(
            "file `{}` is too large: {} bytes exceeds the limit of {} bytes",
            path_uri.inferred_native_path_string(),
            metadata.size,
            OPENAI_FILE_UPLOAD_LIMIT_BYTES,
        )));
    }
    let contents = fs
        .read_file_stream(&path_uri, sandbox.as_ref())
        .await
        .map_err(|error| contextualize_error(error.to_string()))?;
    let file_name = path_uri
        .basename()
        .or_else(|| {
            path_uri.infer_path_convention().and_then(|convention| {
                convention
                    .path_segments(file_path)
                    .rfind(|segment| !segment.is_empty())
                    .map(str::to_string)
            })
        })
        .unwrap_or_else(|| "file".to_string());
    let upload_auth = codex_model_provider::auth_provider_from_auth(auth);
    let uploaded = upload_openai_file(
        turn_context.config.chatgpt_base_url.trim_end_matches('/'),
        upload_auth.as_ref(),
        &sess.services.openai_file_upload_client_pool,
        file_name,
        metadata.size,
        contents,
        hosted_upload,
    )
    .await
    .map_err(|error| contextualize_error(error.to_string()))?;
    let mut payload = serde_json::Map::new();
    payload.insert(
        "download_url".to_string(),
        JsonValue::String(uploaded.download_url),
    );
    payload.insert("file_id".to_string(), JsonValue::String(uploaded.file_id));
    if optional_fields
        .iter()
        .any(|optional_field| optional_field == "mime_type")
        && let Some(mime_type) = uploaded.mime_type
    {
        payload.insert("mime_type".to_string(), JsonValue::String(mime_type));
    }
    if optional_fields
        .iter()
        .any(|optional_field| optional_field == "file_name")
    {
        payload.insert(
            "file_name".to_string(),
            JsonValue::String(uploaded.file_name),
        );
    }
    Ok(JsonValue::Object(payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment_selection::TurnEnvironmentState;
    use crate::session::tests::make_session_and_context;
    use crate::session::turn_context::TurnContext;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use codex_utils_path_uri::PathUri;
    use pretty_assertions::assert_eq;
    use std::path::Path;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn set_primary_environment_cwd(turn_context: &mut TurnContext, cwd: &Path) {
        let cwd = AbsolutePathBuf::try_from(cwd).expect("absolute path");
        let TurnEnvironmentState::Ready(primary) = &mut turn_context.environments.environments[0]
        else {
            panic!("expected ready primary environment");
        };
        primary.selection.cwd = PathUri::from_abs_path(&cwd);
        primary.selection.workspace_roots.clear();
    }

    #[tokio::test]
    async fn openai_file_argument_rewrite_requires_declared_file_params() {
        let (session, turn_context) = make_session_and_context().await;
        let step_context = StepContext::for_test(Arc::new(turn_context));
        let arguments = Some(serde_json::json!({
            "file": "/tmp/codex-smoke-file.txt"
        }));

        let rewritten = rewrite_mcp_tool_arguments_for_openai_files(
            &session,
            &step_context,
            arguments.clone(),
            /*openai_file_input_optional_fields*/ None,
            /*hosted_upload*/ None,
        )
        .await
        .expect("rewrite should succeed");

        assert_eq!(rewritten, arguments);
    }

    #[tokio::test]
    async fn build_uploaded_argument_value_uses_environment_that_becomes_ready_during_turn() {
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

        let (session, mut turn_context) = make_session_and_context().await;
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        let dir = tempdir().expect("temp dir");
        let local_path = dir.path().join("file_report.csv");
        tokio::fs::write(&local_path, b"hello")
            .await
            .expect("write local file");
        set_primary_environment_cwd(&mut turn_context, dir.path());
        let environment = turn_context
            .environments
            .primary()
            .expect("ready primary environment");
        let selection = environment.selection();
        let environment_config = environment.config().clone();
        let environments = crate::environment_selection::ThreadEnvironments::new(
            session.services.turn_environments.environment_manager(),
            crate::shell::default_user_shell(),
            environment_config.clone(),
            crate::shell_snapshot::ShellSnapshot::disabled(),
            Default::default(),
            /*non_blocking_snapshots*/ true,
        );
        environments.update_selections(std::slice::from_ref(&selection), &environment_config);
        turn_context.environments = environments.snapshot().await;
        turn_context
            .environments
            .starting()
            .next()
            .expect("environment should initially be starting")
            .wait_until_ready()
            .await
            .expect("environment should become ready");
        let step_environments = turn_context.environments.refresh_readiness();

        let mut config = (*turn_context.config).clone();
        config.chatgpt_base_url = format!("{}/backend-api", server.uri());
        turn_context.config = Arc::new(config);
        let mut step_context = StepContext::for_test(Arc::new(turn_context));
        Arc::get_mut(&mut step_context)
            .expect("step context should be uniquely owned")
            .environments = step_environments;

        let rewritten = build_uploaded_argument_value(
            &session,
            &step_context,
            Some(&auth),
            FileArgumentLocation {
                field_name: "file",
                index: None,
            },
            &["mime_type".to_string(), "file_name".to_string()],
            "file_report.csv",
            /*hosted_upload*/ None,
        )
        .await
        .expect("rewrite should upload the local file");

        assert_eq!(
            rewritten,
            serde_json::json!({
                "download_url": format!("{}/download/file_123", server.uri()),
                "file_id": "file_123",
                "mime_type": "text/csv",
                "file_name": "file_report.csv",
            })
        );
    }

    #[tokio::test]
    async fn build_uploaded_argument_value_rejects_oversized_file_before_reading() {
        let (session, mut turn_context) = make_session_and_context().await;
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        let dir = tempdir().expect("temp dir");
        let file_path = dir.path().join("oversized.bin");
        let file = std::fs::File::create(&file_path).expect("create sparse file");
        file.set_len(OPENAI_FILE_UPLOAD_LIMIT_BYTES + 1)
            .expect("size sparse file");
        set_primary_environment_cwd(&mut turn_context, dir.path());
        let step_context = StepContext::for_test(Arc::new(turn_context));

        let error = build_uploaded_argument_value(
            &session,
            &step_context,
            Some(&auth),
            FileArgumentLocation {
                field_name: "file",
                index: None,
            },
            &[],
            "oversized.bin",
            /*hosted_upload*/ None,
        )
        .await
        .expect_err("oversized file should be rejected");

        assert!(error.contains("is too large"));
        assert!(error.contains(&(OPENAI_FILE_UPLOAD_LIMIT_BYTES + 1).to_string()));
    }

    #[tokio::test]
    async fn rewrite_argument_value_for_openai_files_omits_undeclared_optional_fields() {
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

        let (session, mut turn_context) = make_session_and_context().await;
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        let dir = tempdir().expect("temp dir");
        let local_path = dir.path().join("file_report.csv");
        tokio::fs::write(&local_path, b"hello")
            .await
            .expect("write local file");
        set_primary_environment_cwd(&mut turn_context, dir.path());

        let mut config = (*turn_context.config).clone();
        config.chatgpt_base_url = format!("{}/backend-api", server.uri());
        turn_context.config = Arc::new(config);
        let step_context = StepContext::for_test(Arc::new(turn_context));
        let rewritten = rewrite_argument_value_for_openai_files(
            &session,
            &step_context,
            Some(&auth),
            "file",
            &[],
            &serde_json::json!("file_report.csv"),
            /*hosted_upload*/ None,
        )
        .await
        .expect("rewrite should succeed");

        assert_eq!(
            rewritten,
            Some(serde_json::json!({
                "download_url": format!("{}/download/file_123", server.uri()),
                "file_id": "file_123",
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

        let (session, mut turn_context) = make_session_and_context().await;
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        let dir = tempdir().expect("temp dir");
        tokio::fs::write(dir.path().join("one.csv"), b"one")
            .await
            .expect("write first local file");
        tokio::fs::write(dir.path().join("two.csv"), b"two")
            .await
            .expect("write second local file");
        set_primary_environment_cwd(&mut turn_context, dir.path());

        let mut config = (*turn_context.config).clone();
        config.chatgpt_base_url = format!("{}/backend-api", server.uri());
        turn_context.config = Arc::new(config);
        let step_context = StepContext::for_test(Arc::new(turn_context));
        let rewritten = rewrite_argument_value_for_openai_files(
            &session,
            &step_context,
            Some(&auth),
            "files",
            &[],
            &serde_json::json!(["one.csv", "two.csv"]),
            /*hosted_upload*/ None,
        )
        .await
        .expect("rewrite should succeed");

        assert_eq!(
            rewritten,
            Some(serde_json::json!([
                {
                    "download_url": format!("{}/download/file_1", server.uri()),
                    "file_id": "file_1",
                },
                {
                    "download_url": format!("{}/download/file_2", server.uri()),
                    "file_id": "file_2",
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
        let step_context = StepContext::for_test(Arc::new(turn_context));
        let error = rewrite_mcp_tool_arguments_for_openai_files(
            &session,
            &step_context,
            Some(serde_json::json!({
                "file": "/definitely/missing/file.csv",
            })),
            Some(&HashMap::from([("file".to_string(), Vec::new())])),
            /*hosted_upload*/ None,
        )
        .await
        .expect_err("missing file should fail");

        assert!(error.contains("failed to upload"));
        assert!(error.contains("file"));
    }
}
