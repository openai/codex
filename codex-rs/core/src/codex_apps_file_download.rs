use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_api::download_openai_file;
use codex_login::CodexAuth;
use codex_model_provider::BearerAuthProvider;
use codex_protocol::mcp::CallToolResult;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Map as JsonMap;
use serde_json::Value as JsonValue;
use tracing::warn;

const CODEX_APPS_FILE_DOWNLOAD_ARTIFACTS_DIR: &str = ".tmp/codex_apps_downloads";
const CODEX_APPS_PROVIDER_BUILTIN: &str = "builtin";
const CODEX_APPS_META_PROVIDER_KEY: &str = "provider";
const CODEX_APPS_META_MATERIALIZE_FILE_DOWNLOAD_KEY: &str = "materialize_file_download";

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

fn codex_apps_download_base_url(turn_context: &TurnContext) -> &str {
    turn_context.config.chatgpt_base_url.as_str()
}

fn should_materialize_codex_apps_file_download(
    server: &str,
    codex_apps_meta: Option<&JsonMap<String, JsonValue>>,
) -> bool {
    if server != codex_mcp::CODEX_APPS_MCP_SERVER_NAME {
        return false;
    }

    let Some(codex_apps_meta) = codex_apps_meta else {
        return false;
    };

    codex_apps_meta
        .get(CODEX_APPS_META_PROVIDER_KEY)
        .and_then(JsonValue::as_str)
        == Some(CODEX_APPS_PROVIDER_BUILTIN)
        && codex_apps_meta
            .get(CODEX_APPS_META_MATERIALIZE_FILE_DOWNLOAD_KEY)
            .and_then(JsonValue::as_bool)
            == Some(true)
}

pub(crate) async fn maybe_materialize_codex_apps_file_download_result(
    sess: &Session,
    turn_context: &TurnContext,
    server: &str,
    codex_apps_meta: Option<&JsonMap<String, JsonValue>>,
    result: CallToolResult,
) -> CallToolResult {
    let auth = sess.services.auth_manager.auth().await;
    maybe_materialize_codex_apps_file_download_result_with_auth(
        turn_context,
        &sess.conversation_id.to_string(),
        auth.as_ref(),
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
    let download_base_url = codex_apps_download_base_url(turn_context);
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
    let downloaded = match download_openai_file(
        download_base_url,
        &auth_provider,
        &payload.file_uri.download_url,
    )
    .await
    {
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
    async fn codex_apps_file_download_materialization_adds_local_path_for_marked_tools() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/download/file_123"))
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
                    "download_url": format!("{}/download/file_123", server.uri()),
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
}
