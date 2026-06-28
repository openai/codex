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
use crate::session::turn_context::TurnContext;
use codex_api::OPENAI_FILE_UPLOAD_LIMIT_BYTES;
use codex_api::upload_openai_file;
use codex_login::CodexAuth;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use codex_utils_path_uri::PathUriParseError;
use serde_json::Value as JsonValue;

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

fn resolve_environment_file_path(
    cwd: &PathUri,
    file_path: &str,
) -> Result<PathUri, PathUriParseError> {
    match cwd.join(file_path) {
        Err(PathUriParseError::InvalidFileUriPath { path }) if path == cwd.to_string() => {
            let native_cwd = cwd
                .to_abs_path()
                .map_err(|_| PathUriParseError::InvalidFileUriPath { path: path.clone() })?;
            PathUri::from_host_native_path(native_cwd.as_path().join(file_path))
                .map_err(|_| PathUriParseError::InvalidFileUriPath { path })
        }
        result => result,
    }
}

fn upload_file_name(
    path_uri: &PathUri,
    file_path: &str,
    convention: PathConvention,
) -> Result<String, String> {
    let unusable_name = || "the path does not end in a usable file name".to_string();
    let source = path_uri.basename().unwrap_or_else(|| {
        // Opaque path URIs intentionally have no lexical basename. The tool argument is the
        // exact Unicode spelling that was interpreted with this convention, so its final native
        // component is the only lossless name available for upload metadata.
        file_path.to_string()
    });
    let file_name = convention
        .path_segments(&source)
        .rfind(|segment| !segment.is_empty())
        .ok_or_else(unusable_name)?;

    if matches!(file_name, "." | "..") || file_name.contains('\0') {
        Err(unusable_name())
    } else {
        Ok(file_name.to_string())
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
    let path_uri = resolve_environment_file_path(turn_environment.cwd(), file_path)
        .map_err(|error| contextualize_error(error.to_string()))?;
    let path_convention = turn_environment
        .cwd()
        .infer_path_convention()
        .or_else(|| path_uri.infer_path_convention())
        .ok_or_else(|| {
            contextualize_error(
                "could not determine the selected environment's path convention".to_string(),
            )
        })?;
    let file_name =
        upload_file_name(&path_uri, file_path, path_convention).map_err(&contextualize_error)?;
    let display_path = path_uri.inferred_native_path_string();
    let fs = turn_environment.environment.get_filesystem();
    let metadata = fs
        .get_metadata(&path_uri, /*sandbox*/ None)
        .await
        .map_err(|error| contextualize_error(error.to_string()))?;
    if !metadata.is_file {
        return Err(contextualize_error(format!(
            "path `{display_path}` is not a file"
        )));
    }
    if metadata.size > OPENAI_FILE_UPLOAD_LIMIT_BYTES {
        return Err(contextualize_error(format!(
            "file `{display_path}` is too large: {} bytes exceeds the limit of {} bytes",
            metadata.size, OPENAI_FILE_UPLOAD_LIMIT_BYTES,
        )));
    }
    let contents = fs
        .read_file_stream(&path_uri, /*sandbox*/ None)
        .await
        .map_err(|error| contextualize_error(error.to_string()))?;
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
    use codex_utils_absolute_path::AbsolutePathBuf;
    use codex_utils_path_uri::PathUri;
    use pretty_assertions::assert_eq;
    use std::path::Path;
    use std::sync::Arc;
    use tempfile::tempdir;

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

    #[test]
    fn upload_file_name_uses_exact_target_native_component_for_opaque_uri() {
        let cwd = PathUri::parse("file:///C:/workspace").expect("valid Windows cwd URI");

        for file_path in [r"\\?\C:\reports\report.pdf", "//?/C:/reports/report.pdf"] {
            let path_uri = cwd.join(file_path).expect("valid Windows namespace path");

            assert_eq!(
                (
                    path_uri.basename(),
                    upload_file_name(&path_uri, file_path, PathConvention::Windows,)
                ),
                (None, Ok("report.pdf".to_string())),
                "upload name for {file_path}"
            );
        }
    }

    #[test]
    fn upload_file_name_rejects_opaque_uri_without_a_usable_component() {
        let path_uri =
            PathUri::parse("file:///%00/bad/path/YQ").expect("structurally valid opaque path URI");

        assert_eq!(
            upload_file_name(&path_uri, "..", PathConvention::Posix),
            Err("the path does not end in a usable file name".to_string())
        );
    }

    #[test]
    fn upload_file_name_applies_target_native_separators_to_uri_basename() {
        for (uri, convention, expected) in [
            ("file:///tmp/a%2Fb", PathConvention::Posix, "b"),
            ("file:///C:/a%5Cb", PathConvention::Windows, "b"),
            ("file:///tmp/a%252Fb", PathConvention::Posix, "a%2Fb"),
        ] {
            let path_uri = PathUri::parse(uri).expect("valid path URI");

            assert_eq!(
                upload_file_name(&path_uri, "unused", convention),
                Ok(expected.to_string()),
                "upload name for {uri}"
            );
        }
    }

    #[tokio::test]
    async fn build_uploaded_argument_value_rejects_unusable_name_before_file_access() {
        let (_, mut turn_context) = make_session_and_context().await;
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        let primary = turn_context
            .environments
            .turn_environments
            .first_mut()
            .expect("primary environment");
        *primary = TurnEnvironment::new(
            primary.environment_id.clone(),
            Arc::clone(&primary.environment),
            PathUri::parse("file:///C:/workspace").expect("valid Windows cwd URI"),
            primary.shell.clone(),
        );
        let file_path = r"\\?\C:\reports\..";

        let error = build_uploaded_argument_value(
            &turn_context,
            Some(&auth),
            "file",
            /*index*/ None,
            file_path,
        )
        .await
        .expect_err("unusable upload name should fail before file access");

        assert_eq!(
            error,
            format!(
                "failed to upload `{file_path}` for `file`: \
                 the path does not end in a usable file name"
            )
        );
    }

    #[cfg(unix)]
    #[test]
    fn resolve_environment_file_path_joins_opaque_native_cwd_without_expanding_tilde() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        use std::path::PathBuf;

        let native_cwd = AbsolutePathBuf::from_absolute_path_checked(PathBuf::from(
            OsString::from_vec(b"/tmp/codex-non-utf8-\xff".to_vec()),
        ))
        .expect("absolute non-UTF-8 cwd");
        let cwd = PathUri::from_abs_path(&native_cwd);
        let file_paths = ["report.txt", "~/report.txt"];
        let actual = file_paths.map(|file_path| {
            resolve_environment_file_path(&cwd, file_path)
                .expect("opaque native cwd should resolve relative paths")
                .to_abs_path()
                .expect("resolved URI should remain host-native")
                .into_path_buf()
        });
        let expected = file_paths.map(|file_path| native_cwd.as_path().join(file_path));

        assert_eq!(actual, expected);
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
