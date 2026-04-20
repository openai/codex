use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::codex_apps_mcp_tools::should_materialize_codex_apps_file_download;
use codex_api::download_openai_file;
use codex_login::CodexAuth;
use codex_model_provider::AuthorizationHeaderAuthProvider;
use codex_model_provider::BearerAuthProvider;
use codex_protocol::mcp::CallToolResult;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Map as JsonMap;
use serde_json::Value as JsonValue;
use tracing::warn;

const CODEX_APPS_FILE_DOWNLOAD_ARTIFACTS_DIR: &str = ".tmp/codex_apps_downloads";

#[derive(Debug, Deserialize, Serialize)]
struct CodexAppsFileDownloadPayload {
    file_id: String,
    #[serde(default)]
    file_name: Option<String>,
    file_uri: CodexAppsFileUri,
}

#[derive(Debug, Deserialize, Serialize)]
struct CodexAppsFileUri {
    download_url: String,
    #[serde(default)]
    file_name: Option<String>,
}

pub(crate) async fn maybe_materialize_codex_apps_file_download_result(
    sess: &Session,
    turn_context: &TurnContext,
    server: &str,
    codex_apps_meta: Option<&JsonMap<String, JsonValue>>,
    result: CallToolResult,
) -> CallToolResult {
    let auth = sess.services.auth_manager.auth().await;
    let authorization_header_value = match sess.authorization_header_for_current_agent_task().await {
        Ok(value) => value,
        Err(error) => {
            warn!(error = %error, "failed to build agent assertion authorization for codex_apps file download materialization");
            None
        }
    };
    maybe_materialize_codex_apps_file_download_result_with_auth(
        turn_context,
        &sess.conversation_id.to_string(),
        auth.as_ref(),
        authorization_header_value,
        server,
        codex_apps_meta,
        result,
    )
    .await
}

async fn maybe_materialize_codex_apps_file_download_result_with_auth(
    turn_context: &TurnContext,
    session_id: &str,
    auth: Option<&CodexAuth>,
    authorization_header_value: Option<String>,
    server: &str,
    codex_apps_meta: Option<&JsonMap<String, JsonValue>>,
    mut result: CallToolResult,
) -> CallToolResult {
    if !should_materialize_codex_apps_file_download(server, codex_apps_meta)
        || result.is_error == Some(true)
    {
        return result;
    }

    let Some(payload) = extract_codex_apps_file_download_payload(&result) else {
        return result;
    };
    if result.structured_content.is_none()
        && let Ok(structured_content) = serde_json::to_value(&payload)
    {
        result.structured_content = Some(structured_content);
    }

    let Some(auth) = auth else {
        warn!(
            "skipping codex_apps file download materialization because ChatGPT auth is unavailable"
        );
        return result;
    };
    let downloaded = match if let Some(authorization_header_value) = authorization_header_value {
        let mut auth_provider = AuthorizationHeaderAuthProvider::new(
            Some(authorization_header_value),
            /*account_id*/ None,
        );
        if auth.is_fedramp_account() {
            auth_provider = auth_provider.with_fedramp_routing_header();
        }
        download_openai_file(
            turn_context.config.chatgpt_base_url.trim_end_matches('/'),
            &auth_provider,
            &payload.file_uri.download_url,
        )
        .await
    } else {
        let token_data = match auth.get_token_data() {
            Ok(token_data) => token_data,
            Err(error) => {
                warn!(error = %error, "failed to read ChatGPT auth for codex_apps file download materialization");
                return result;
            }
        };
        let auth_provider = BearerAuthProvider {
            token: Some(token_data.access_token),
            account_id: token_data.account_id,
            is_fedramp_account: auth.is_fedramp_account(),
        };
        download_openai_file(
            turn_context.config.chatgpt_base_url.trim_end_matches('/'),
            &auth_provider,
            &payload.file_uri.download_url,
        )
        .await
    } {
        Ok(downloaded) => downloaded,
        Err(error) => {
            warn!(
                error = %error,
                file_id = payload.file_id,
                "failed to materialize codex_apps file download via app-server",
            );
            return result;
        }
    };

    let artifact_path = codex_apps_file_download_artifact_path(
        &turn_context.config.codex_home,
        session_id,
        &payload.file_id,
        payload
            .file_name
            .as_deref()
            .or(payload.file_uri.file_name.as_deref())
            .unwrap_or("downloaded_file"),
    );
    if let Some(parent) = artifact_path.parent()
        && let Err(error) = tokio::fs::create_dir_all(parent.as_path()).await
    {
        warn!(
            error = %error,
            path = %parent.display(),
            "failed to create codex_apps file download artifact directory",
        );
        return result;
    }
    if let Err(error) = tokio::fs::write(artifact_path.as_path(), &downloaded).await {
        warn!(
            error = %error,
            path = %artifact_path.display(),
            "failed to write codex_apps file download artifact",
        );
        return result;
    }

    let local_path = artifact_path.to_string_lossy().to_string();
    if let Some(JsonValue::Object(map)) = result.structured_content.as_mut() {
        map.insert(
            "local_path".to_string(),
            JsonValue::String(local_path.clone()),
        );
    }
    result.content.push(serde_json::json!({
        "type": "text",
        "text": format!("Downloaded file to local path: {local_path}"),
    }));
    result
}

fn extract_codex_apps_file_download_payload(
    result: &CallToolResult,
) -> Option<CodexAppsFileDownloadPayload> {
    if let Some(structured_content) = result.structured_content.clone()
        && let Ok(payload) =
            serde_json::from_value::<CodexAppsFileDownloadPayload>(structured_content)
    {
        return Some(payload);
    }

    result
        .content
        .iter()
        .filter_map(|item| item.as_object())
        .find_map(|item| {
            let text = item.get("text")?.as_str()?;
            serde_json::from_str::<CodexAppsFileDownloadPayload>(text).ok()
        })
}

fn codex_apps_file_download_artifact_path(
    codex_home: &codex_utils_absolute_path::AbsolutePathBuf,
    session_id: &str,
    file_id: &str,
    file_name: &str,
) -> codex_utils_absolute_path::AbsolutePathBuf {
    codex_home
        .join(CODEX_APPS_FILE_DOWNLOAD_ARTIFACTS_DIR)
        .join(sanitize_path_component(session_id, "session"))
        .join(sanitize_path_component(file_id, "file"))
        .join(sanitize_file_name(file_name))
}

fn sanitize_path_component(value: &str, fallback: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        fallback.to_string()
    } else {
        sanitized
    }
}

fn sanitize_file_name(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "downloaded_file".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::tests::make_session_and_context;
    use codex_login::CodexAuth;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    fn download_materialization_meta() -> JsonMap<String, JsonValue> {
        serde_json::json!({
            "provider": "builtin",
            "materialize_file_download": true,
        })
        .as_object()
        .cloned()
        .expect("_codex_apps metadata object")
    }

    #[tokio::test]
    async fn codex_apps_file_download_materialization_ignores_results_without_metadata_flag() {
        let (_, turn_context) = make_session_and_context().await;
        let original = CallToolResult {
            content: vec![serde_json::json!({"type": "text", "text": "hello"})],
            structured_content: Some(serde_json::json!({"x": 1})),
            is_error: Some(false),
            meta: None,
        };

        let result = maybe_materialize_codex_apps_file_download_result_with_auth(
            &turn_context,
            "session-1",
            Some(&CodexAuth::create_dummy_chatgpt_auth_for_testing()),
            None,
            "custom_server",
            /*codex_apps_meta*/ None,
            original.clone(),
        )
        .await;

        assert_eq!(result, original);
    }

    #[tokio::test]
    async fn codex_apps_file_download_materialization_adds_local_path_for_marked_tools() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/codex/files/file_123/content"))
            .and(header("authorization", "Bearer Access Token"))
            .and(header("chatgpt-account-id", "account_id"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/plain")
                    .set_body_bytes(b"downloaded contents".to_vec()),
            )
            .mount(&server)
            .await;

        let (_, mut turn_context) = make_session_and_context().await;
        let mut config = (*turn_context.config).clone();
        config.chatgpt_base_url = format!("{}/backend-api/codex", server.uri());
        turn_context.config = Arc::new(config);
        let original = CallToolResult {
            content: vec![serde_json::json!({
                "type": "text",
                "text": "{\"file_id\":\"file_123\"}",
            })],
            structured_content: Some(serde_json::json!({
                "file_id": "file_123",
                "file_name": "testing-file.txt",
                "file_uri": {
                    "download_url": "/api/codex/files/file_123/content",
                    "file_id": "file_123",
                    "file_name": "testing-file.txt",
                    "mime_type": "text/plain",
                }
            })),
            is_error: Some(false),
            meta: None,
        };

        let result = maybe_materialize_codex_apps_file_download_result_with_auth(
            &turn_context,
            "session-1",
            Some(&CodexAuth::create_dummy_chatgpt_auth_for_testing()),
            None,
            codex_mcp::CODEX_APPS_MCP_SERVER_NAME,
            Some(&download_materialization_meta()),
            original,
        )
        .await;

        let local_path = result
            .structured_content
            .as_ref()
            .and_then(|value| value.get("local_path"))
            .and_then(JsonValue::as_str)
            .expect("local_path in structured content");
        assert!(local_path.contains("codex_apps_downloads"));
        let saved = tokio::fs::read(local_path)
            .await
            .expect("saved local file should exist");
        assert_eq!(saved, b"downloaded contents".to_vec());
        assert!(result.content.iter().any(|block| {
            block.get("type").and_then(JsonValue::as_str) == Some("text")
                && block
                    .get("text")
                    .and_then(JsonValue::as_str)
                    .is_some_and(|text| text.contains("Downloaded file to local path:"))
        }));
    }

    #[tokio::test]
    async fn codex_apps_file_download_materialization_uses_json_text_when_structured_content_is_missing()
     {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/codex/files/file_123/content"))
            .and(header("authorization", "Bearer Access Token"))
            .and(header("chatgpt-account-id", "account_id"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/plain")
                    .set_body_bytes(b"downloaded contents".to_vec()),
            )
            .mount(&server)
            .await;

        let (_, mut turn_context) = make_session_and_context().await;
        let mut config = (*turn_context.config).clone();
        config.chatgpt_base_url = format!("{}/backend-api/codex", server.uri());
        turn_context.config = Arc::new(config);
        let original = CallToolResult {
            content: vec![serde_json::json!({
                "type": "text",
                "text": serde_json::json!({
                    "file_id": "file_123",
                    "file_name": "testing-file.txt",
                    "file_uri": {
                        "download_url": "/api/codex/files/file_123/content",
                        "file_name": "testing-file.txt",
                    }
                })
                .to_string(),
            })],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        };

        let result = maybe_materialize_codex_apps_file_download_result_with_auth(
            &turn_context,
            "session-1",
            Some(&CodexAuth::create_dummy_chatgpt_auth_for_testing()),
            None,
            codex_mcp::CODEX_APPS_MCP_SERVER_NAME,
            Some(&download_materialization_meta()),
            original,
        )
        .await;

        let local_path = result
            .content
            .iter()
            .find_map(|item| {
                item.get("text")
                    .and_then(|text| text.as_str())
                    .and_then(|text| text.strip_prefix("Downloaded file to local path: "))
            })
            .expect("expected local path announcement");
        assert_eq!(
            result.structured_content,
            Some(serde_json::json!({
                "file_id": "file_123",
                "file_name": "testing-file.txt",
                "file_uri": {
                    "download_url": "/api/codex/files/file_123/content",
                    "file_name": "testing-file.txt",
                },
                "local_path": local_path,
            }))
        );
        assert_eq!(
            tokio::fs::read(local_path).await.expect("downloaded file"),
            b"downloaded contents"
        );
    }
}
