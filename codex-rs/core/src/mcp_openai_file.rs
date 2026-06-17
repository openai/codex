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
//! Model-visible schema masking is owned by `codex-mcp` alongside MCP tool
//! inventory, so this module only handles the execution-time argument rewrite.

use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_api::OPENAI_FILE_UPLOAD_LIMIT_BYTES;
use codex_api::upload_openai_file;
use codex_login::CodexAuth;
use codex_protocol::permissions::ReadDenyMatcher;
use codex_utils_path_uri::PathUri;
use serde_json::Value as JsonValue;

const OPENAI_FILE_AUTH_RESOLUTION_FAILED_MESSAGE: &str =
    "failed to resolve ChatGPT auth for OpenAI file upload";

fn model_visible_auth_resolution_error(error: std::io::Error) -> String {
    tracing::warn!(
        error = %error,
        "failed to resolve auth for OpenAI file upload"
    );
    OPENAI_FILE_AUTH_RESOLUTION_FAILED_MESSAGE.to_string()
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
    let auth = sess
        .services
        .auth_manager
        .auth()
        .await
        .map_err(model_visible_auth_resolution_error)?;
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

async fn rewrite_argument_value_for_openai_files(
    turn_context: &TurnContext,
    auth: Option<&CodexAuth>,
    field_name: &str,
    value: &JsonValue,
) -> Result<Option<JsonValue>, String> {
    match value {
        JsonValue::String(file_path) => {
            let rewritten = build_uploaded_argument_value(
                turn_context,
                auth,
                field_name,
                /*index*/ None,
                file_path,
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
                    turn_context,
                    auth,
                    field_name,
                    Some(index),
                    file_path,
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
    turn_context: &TurnContext,
    auth: Option<&CodexAuth>,
    field_name: &str,
    index: Option<usize>,
    file_path: &str,
) -> Result<JsonValue, String> {
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
    let Some(turn_environment) = turn_context.environments.primary() else {
        return Err(contextualize_error(
            "no primary turn environment is available".to_string(),
        ));
    };
    // TODO(anp): Resolve app tool file arguments using the selected environment's native path
    // convention so uploads can read relative paths from foreign environments.
    let native_environment_cwd = turn_environment
        .cwd()
        .to_abs_path()
        .map_err(|error| contextualize_error(error.to_string()))?;
    let resolved_path = native_environment_cwd.join(file_path);
    let file_system_policy = turn_context.file_system_sandbox_policy();
    let read_deny_matcher =
        ReadDenyMatcher::new(&file_system_policy, native_environment_cwd.as_path());
    let is_readable = |path: &std::path::Path| {
        file_system_policy.can_read_path_with_cwd(path, native_environment_cwd.as_path())
            && !read_deny_matcher
                .as_ref()
                .is_some_and(|matcher| matcher.is_read_denied(path))
    };
    if !is_readable(resolved_path.as_path()) {
        return Err(contextualize_error(
            "path is not readable under the current sandbox policy".to_string(),
        ));
    }

    let requested_path_uri = PathUri::from_abs_path(&resolved_path);
    let fs = turn_environment.environment.get_filesystem();
    // Upload preprocessing runs in the Codex process, so enforce the turn's
    // read policy before opening either the requested path or a symlink target.
    let canonical_path_uri = fs
        .canonicalize(&requested_path_uri, /*sandbox*/ None)
        .await
        .map_err(|error| contextualize_error(error.to_string()))?;
    let canonical_path = canonical_path_uri
        .to_abs_path()
        .map_err(|error| contextualize_error(error.to_string()))?;
    if !is_readable(canonical_path.as_path()) {
        return Err(contextualize_error(
            "path is not readable under the current sandbox policy".to_string(),
        ));
    }
    let metadata = fs
        .get_metadata(&canonical_path_uri, /*sandbox*/ None)
        .await
        .map_err(|error| contextualize_error(error.to_string()))?;
    if !metadata.is_file {
        return Err(contextualize_error(format!(
            "path `{}` is not a file",
            resolved_path.display()
        )));
    }
    if metadata.size > OPENAI_FILE_UPLOAD_LIMIT_BYTES {
        return Err(contextualize_error(format!(
            "file `{}` is too large: {} bytes exceeds the limit of {} bytes",
            resolved_path.display(),
            metadata.size,
            OPENAI_FILE_UPLOAD_LIMIT_BYTES,
        )));
    }
    let contents = fs
        .read_file_stream(&canonical_path_uri, /*sandbox*/ None)
        .await
        .map_err(|error| contextualize_error(error.to_string()))?;
    let file_name = resolved_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("file")
        .to_string();
    let upload_auth = codex_model_provider::auth_provider_from_auth(auth);
    let uploaded = upload_openai_file(
        turn_context.config.chatgpt_base_url.trim_end_matches('/'),
        upload_auth.as_ref(),
        file_name,
        metadata.size,
        contents,
    )
    .await
    .map_err(|error| contextualize_error(error.to_string()))?;
    Ok(serde_json::json!({
        "download_url": uploaded.download_url,
        "file_id": uploaded.file_id,
        "mime_type": uploaded.mime_type,
        "file_name": uploaded.file_name,
        "uri": uploaded.uri,
        "file_size_bytes": uploaded.file_size_bytes,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::tests::make_session_and_context;
    use crate::session::turn_context::TurnEnvironment;
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemPath;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::FileSystemSandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use codex_utils_path_uri::PathUri;
    use pretty_assertions::assert_eq;
    use std::path::Path;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn auth_resolution_errors_hide_credential_source_details() {
        let credential_path = "/run/secrets/codex-wif/subject-token";
        let message = model_visible_auth_resolution_error(std::io::Error::other(format!(
            "file credential source could not read {credential_path}"
        )));

        assert_eq!(message, OPENAI_FILE_AUTH_RESOLUTION_FAILED_MESSAGE);
        assert!(!message.contains(credential_path));
    }

    fn set_primary_environment_cwd(turn_context: &mut TurnContext, cwd: &Path) {
        let cwd = AbsolutePathBuf::try_from(cwd).expect("absolute path");
        turn_context.permission_profile = codex_protocol::models::PermissionProfile::Disabled;
        let primary = turn_context
            .environments
            .turn_environments
            .first_mut()
            .expect("primary environment");
        *primary = TurnEnvironment::new(
            primary.environment_id.clone(),
            Arc::clone(&primary.environment),
            PathUri::from_abs_path(&cwd),
            primary.shell.clone(),
        );
    }

    fn deny_file_read(turn_context: &mut TurnContext, path: AbsolutePathBuf) {
        let mut file_system_policy = turn_context.file_system_sandbox_policy();
        file_system_policy.preserve_deny_read_restrictions_from(
            &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: path.clone() },
                access: FileSystemAccessMode::Deny,
            }]),
        );
        turn_context.permission_profile =
            PermissionProfile::from_runtime_permissions_with_enforcement(
                turn_context.permission_profile.enforcement(),
                &file_system_policy,
                turn_context.network_sandbox_policy(),
            );
        Arc::make_mut(&mut turn_context.config)
            .workload_identity_credential_deny_paths
            .push(path);
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
    async fn build_uploaded_argument_value_uploads_environment_file() {
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
        set_primary_environment_cwd(&mut turn_context, dir.path());

        let mut config = (*turn_context.config).clone();
        config.chatgpt_base_url = format!("{}/backend-api", server.uri());
        turn_context.config = Arc::new(config);

        let rewritten = build_uploaded_argument_value(
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
                "download_url": format!("{}/download/file_123", server.uri()),
                "file_id": "file_123",
                "mime_type": "text/csv",
                "file_name": "file_report.csv",
                "uri": "sediment://file_123",
                "file_size_bytes": 5,
            })
        );
    }

    #[tokio::test]
    async fn build_uploaded_argument_value_rejects_oversized_file_before_reading() {
        let (_, mut turn_context) = make_session_and_context().await;
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        let dir = tempdir().expect("temp dir");
        let file_path = dir.path().join("oversized.bin");
        let file = std::fs::File::create(&file_path).expect("create sparse file");
        file.set_len(OPENAI_FILE_UPLOAD_LIMIT_BYTES + 1)
            .expect("size sparse file");
        set_primary_environment_cwd(&mut turn_context, dir.path());

        let error = build_uploaded_argument_value(
            &turn_context,
            Some(&auth),
            "file",
            /*index*/ None,
            "oversized.bin",
        )
        .await
        .expect_err("oversized file should be rejected");

        assert!(error.contains("is too large"));
        assert!(error.contains(&(OPENAI_FILE_UPLOAD_LIMIT_BYTES + 1).to_string()));
    }

    #[tokio::test]
    async fn build_uploaded_argument_value_rejects_wif_credential_path() {
        let (_, mut turn_context) = make_session_and_context().await;
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        let dir = tempdir().expect("temp dir");
        let credential_path = dir.path().join("subject-token");
        tokio::fs::write(&credential_path, b"external.subject.token")
            .await
            .expect("write credential");
        set_primary_environment_cwd(&mut turn_context, dir.path());
        deny_file_read(
            &mut turn_context,
            AbsolutePathBuf::try_from(credential_path).expect("absolute credential path"),
        );

        let error = build_uploaded_argument_value(
            &turn_context,
            Some(&auth),
            "file",
            /*index*/ None,
            "subject-token",
        )
        .await
        .expect_err("WIF credential should not be uploaded");

        assert!(error.contains("not readable under the current sandbox policy"));
        assert!(!error.contains("external.subject.token"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn build_uploaded_argument_value_rejects_symlink_to_wif_credential() {
        let (_, mut turn_context) = make_session_and_context().await;
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        let dir = tempdir().expect("temp dir");
        let credential_path = dir.path().join("subject-token");
        let upload_path = dir.path().join("report.csv");
        tokio::fs::write(&credential_path, b"external.subject.token")
            .await
            .expect("write credential");
        std::os::unix::fs::symlink(&credential_path, &upload_path).expect("create symlink");
        set_primary_environment_cwd(&mut turn_context, dir.path());
        deny_file_read(
            &mut turn_context,
            AbsolutePathBuf::try_from(credential_path.clone()).expect("absolute credential path"),
        );

        let error = build_uploaded_argument_value(
            &turn_context,
            Some(&auth),
            "file",
            /*index*/ None,
            "report.csv",
        )
        .await
        .expect_err("symlinked WIF credential should not be uploaded");

        assert!(error.contains("not readable under the current sandbox policy"));
        assert!(!error.contains(credential_path.to_string_lossy().as_ref()));
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
        set_primary_environment_cwd(&mut turn_context, dir.path());

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
                "download_url": format!("{}/download/file_123", server.uri()),
                "file_id": "file_123",
                "mime_type": "text/csv",
                "file_name": "file_report.csv",
                "uri": "sediment://file_123",
                "file_size_bytes": 5,
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
        set_primary_environment_cwd(&mut turn_context, dir.path());

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
                    "download_url": format!("{}/download/file_1", server.uri()),
                    "file_id": "file_1",
                    "mime_type": "text/csv",
                    "file_name": "one.csv",
                    "uri": "sediment://file_1",
                    "file_size_bytes": 3,
                },
                {
                    "download_url": format!("{}/download/file_2", server.uri()),
                    "file_id": "file_2",
                    "mime_type": "text/csv",
                    "file_name": "two.csv",
                    "uri": "sediment://file_2",
                    "file_size_bytes": 3,
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
