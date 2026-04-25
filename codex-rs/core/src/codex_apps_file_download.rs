use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_api::OPENAI_FILE_UPLOAD_LIMIT_BYTES;
use codex_api::download_openai_file_to_path;
use codex_api::get_openai_file_download_info;
use codex_login::CodexAuth;
use codex_model_provider::BearerAuthProvider;
use codex_protocol::mcp::CallToolResult;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Map as JsonMap;
use serde_json::Value as JsonValue;
use tracing::warn;

const CODEX_APPS_FILE_DOWNLOAD_ARTIFACTS_DIR: &str = ".tmp/codex_apps_downloads";
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
        .get(CODEX_APPS_META_MATERIALIZE_FILE_DOWNLOAD_KEY)
        .and_then(JsonValue::as_bool)
        == Some(true)
}

pub(crate) async fn maybe_materialize_codex_apps_file_download_result(
    sess: &Session,
    turn_context: &TurnContext,
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

    let auth = sess.services.auth_manager.auth().await;
    let Some(auth) = auth.as_ref() else {
        warn!(
            "skipping codex_apps file download materialization because ChatGPT auth is unavailable"
        );
        redact_codex_apps_file_download_url(&mut result);
        append_materialization_failure_message(&mut result);
        return result;
    };
    materialize_codex_apps_file_download_result_with_auth(
        turn_context,
        download_base_url,
        &sess.conversation_id.to_string(),
        auth,
        payload,
        result,
    )
    .await
}

async fn materialize_codex_apps_file_download_result_with_auth(
    turn_context: &TurnContext,
    download_base_url: &str,
    session_id: &str,
    auth: &CodexAuth,
    payload: CodexAppsFileDownloadPayload,
    mut result: CallToolResult,
) -> CallToolResult {
    let token_data = match auth.get_token_data() {
        Ok(token_data) => token_data,
        Err(error) => {
            warn!(error = %error, "failed to read ChatGPT auth for codex_apps file download materialization");
            redact_codex_apps_file_download_url(&mut result);
            append_materialization_failure_message(&mut result);
            return result;
        }
    };
    let auth_provider = BearerAuthProvider {
        token: Some(token_data.access_token),
        account_id: token_data.account_id,
        is_fedramp_account: auth.is_fedramp_account(),
    };
    let download_info =
        match get_openai_file_download_info(download_base_url, &auth_provider, &payload.file_id)
            .await
        {
            Ok(download_info) => download_info,
            Err(error) => {
                warn!(
                    error = %error,
                    file_id = payload.file_id,
                    "failed to resolve trusted codex_apps file download link",
                );
                redact_codex_apps_file_download_url(&mut result);
                append_materialization_failure_message(&mut result);
                return result;
            }
        };
    let artifact_path = codex_apps_file_download_artifact_path(
        &turn_context.config.codex_home,
        session_id,
        &payload.file_id,
        download_info
            .file_name
            .as_deref()
            .or(payload.file_name.as_deref())
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
        redact_codex_apps_file_download_url(&mut result);
        append_materialization_failure_message(&mut result);
        return result;
    }

    if let Err(error) = download_openai_file_to_path(
        download_base_url,
        &auth_provider,
        &download_info.download_url,
        artifact_path.as_path(),
        OPENAI_FILE_UPLOAD_LIMIT_BYTES,
    )
    .await
    {
        warn!(
            error = %error,
            file_id = payload.file_id,
            path = %artifact_path.display(),
            "failed to materialize codex_apps file download via app-server",
        );
        redact_codex_apps_file_download_url(&mut result);
        append_materialization_failure_message(&mut result);
        return result;
    }

    let local_path = artifact_path.to_string_lossy().to_string();
    if let Some(JsonValue::Object(map)) = result.structured_content.as_mut() {
        map.insert(
            "local_path".to_string(),
            JsonValue::String(local_path.clone()),
        );
    }
    redact_codex_apps_file_download_url(&mut result);
    result.content.push(serde_json::json!({
        "type": "text",
        "text": format!("Downloaded file to local path: {local_path}"),
    }));
    result
}

fn redact_codex_apps_file_download_url(result: &mut CallToolResult) {
    if let Some(structured_content) = result.structured_content.as_mut() {
        redact_download_url_from_value(structured_content);
    }

    for item in &mut result.content {
        let Some(item_object) = item.as_object_mut() else {
            continue;
        };
        let Some(text) = item_object.get("text").and_then(JsonValue::as_str) else {
            continue;
        };
        let Ok(mut payload) = serde_json::from_str::<JsonValue>(text) else {
            continue;
        };
        redact_download_url_from_value(&mut payload);
        if let Ok(serialized) = serde_json::to_string(&payload) {
            item_object.insert("text".to_string(), JsonValue::String(serialized));
        }
    }
}

fn redact_download_url_from_value(value: &mut JsonValue) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let Some(file_uri) = object.get_mut("file_uri").and_then(JsonValue::as_object_mut) else {
        return;
    };
    file_uri.remove("download_url");
}

fn append_materialization_failure_message(result: &mut CallToolResult) {
    result.content.push(serde_json::json!({
        "type": "text",
        "text": "Failed to materialize downloaded file to a local path.",
    }));
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
    use wiremock::Request;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    #[tokio::test]
    async fn codex_apps_file_download_materialization_adds_local_path_for_marked_tools() {
        let server = MockServer::start().await;
        let attacker_server = MockServer::start().await;
        let attacker_hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let attacker_hits_responder = Arc::clone(&attacker_hits);
        Mock::given(method("GET"))
            .and(path("/backend-api/files/file_123/download"))
            .and(header("authorization", "Bearer Access Token"))
            .and(header("chatgpt-account-id", "account_id"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "download_url": format!("{}/download/file_123", server.uri()),
                "file_name": "trusted-name.txt",
                "mime_type": "text/plain",
            })))
            .mount(&server)
            .await;
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
        Mock::given(method("GET"))
            .respond_with(move |_request: &Request| {
                attacker_hits_responder.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                ResponseTemplate::new(200).set_body_bytes(b"attacker controlled".to_vec())
            })
            .mount(&attacker_server)
            .await;

        let (_, mut turn_context) = make_session_and_context().await;
        let mut config = (*turn_context.config).clone();
        config.chatgpt_base_url = format!("{}/backend-api", server.uri());
        turn_context.config = Arc::new(config);
        let original_payload = serde_json::json!({
            "file_id": "file_123",
            "file_name": "testing-file.txt",
            "file_uri": {
                "download_url": format!("{}/download/file_123", attacker_server.uri()),
                "file_id": "file_123",
                "file_name": "testing-file.txt",
                "mime_type": "text/plain",
            }
        });
        let original = CallToolResult {
            content: vec![serde_json::json!({
                "type": "text",
                "text": original_payload.to_string(),
            })],
            structured_content: Some(original_payload),
            is_error: Some(false),
            meta: None,
        };

        let result = materialize_codex_apps_file_download_result_with_auth(
            &turn_context,
            turn_context.config.chatgpt_base_url.as_str(),
            "session-1",
            &CodexAuth::create_dummy_chatgpt_auth_for_testing(),
            serde_json::from_value(
                original
                    .structured_content
                    .clone()
                    .expect("structured content should exist"),
            )
            .expect("download payload"),
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
        assert!(local_path.ends_with("trusted-name.txt"));
        let structured_download_url = result
            .structured_content
            .as_ref()
            .and_then(|value| value.get("file_uri"))
            .and_then(|value| value.get("download_url"));
        assert_eq!(structured_download_url, None);
        let saved = tokio::fs::read(local_path)
            .await
            .expect("saved local file should exist");
        assert_eq!(saved, b"downloaded contents".to_vec());
        assert_eq!(attacker_hits.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(!result.content.iter().any(|block| {
            block.get("text")
                .and_then(JsonValue::as_str)
                .is_some_and(|text| text.contains(&attacker_server.uri()))
        }));
        assert!(result.content.iter().any(|block| {
            block.get("type").and_then(JsonValue::as_str) == Some("text")
                && block
                    .get("text")
                    .and_then(JsonValue::as_str)
                    .is_some_and(|text| text.contains("Downloaded file to local path:"))
        }));
    }

    #[tokio::test]
    async fn codex_apps_file_download_materialization_failure_redacts_download_url() {
        let server = MockServer::start().await;
        let attacker_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/backend-api/files/file_123/download"))
            .and(header("authorization", "Bearer Access Token"))
            .and(header("chatgpt-account-id", "account_id"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&server)
            .await;

        let (_, mut turn_context) = make_session_and_context().await;
        let mut config = (*turn_context.config).clone();
        config.chatgpt_base_url = format!("{}/backend-api", server.uri());
        turn_context.config = Arc::new(config);
        let original_payload = serde_json::json!({
            "file_id": "file_123",
            "file_name": "testing-file.txt",
            "file_uri": {
                "download_url": format!("{}/download/file_123", attacker_server.uri()),
                "file_name": "testing-file.txt",
                "mime_type": "text/plain",
            }
        });
        let original = CallToolResult {
            content: vec![serde_json::json!({
                "type": "text",
                "text": original_payload.to_string(),
            })],
            structured_content: Some(original_payload),
            is_error: Some(false),
            meta: None,
        };

        let result = materialize_codex_apps_file_download_result_with_auth(
            &turn_context,
            turn_context.config.chatgpt_base_url.as_str(),
            "session-1",
            &CodexAuth::create_dummy_chatgpt_auth_for_testing(),
            serde_json::from_value(
                original
                    .structured_content
                    .clone()
                    .expect("structured content should exist"),
            )
            .expect("download payload"),
            original,
        )
        .await;

        assert!(
            result
                .structured_content
                .as_ref()
                .and_then(|value| value.get("local_path"))
                .is_none()
        );
        assert_eq!(
            result
                .structured_content
                .as_ref()
                .and_then(|value| value.get("file_uri"))
                .and_then(|value| value.get("download_url")),
            None,
        );
        assert!(!result.content.iter().any(|block| {
            block.get("text")
                .and_then(JsonValue::as_str)
                .is_some_and(|text| text.contains(&attacker_server.uri()))
        }));
        assert!(result.content.iter().any(|block| {
            block.get("type").and_then(JsonValue::as_str) == Some("text")
                && block
                    .get("text")
                    .and_then(JsonValue::as_str)
                    .is_some_and(|text| {
                        text.contains("Failed to materialize downloaded file to a local path.")
                    })
        }));
    }
}
