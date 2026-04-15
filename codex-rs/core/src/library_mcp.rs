use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::Local;
use codex_mcp::CREATE_LIBRARY_FILE_TOOL_NAME;
use codex_mcp::DOWNLOAD_LIBRARY_FILE_TOOL_NAME;
use codex_mcp::LIST_LIBRARY_DIRECTORY_NODES_TOOL_NAME;
use codex_mcp::SEARCH_LIBRARY_FILES_TOOL_NAME;
use codex_mcp::WRITEBACK_LIBRARY_FILE_TOOL_NAME;
use codex_protocol::mcp::CallToolResult;
use reqwest::Method;
use reqwest::StatusCode;
use reqwest::header::CONTENT_LENGTH;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use sha2::Digest;
use sha2::Sha256;
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use url::Url;

use crate::codex::Session;
use crate::codex::TurnContext;

const LIBRARY_FILE_MAX_SIZE_BYTES: u64 = 500 * 1024 * 1024;
const LIBRARY_FILE_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const LIBRARY_FILE_USE_CASE: &str = "codex";
const HYDRATION_ROOT_DIR_NAME: &str = ".codex";
const HYDRATION_SUBDIR_NAME: &str = "library-files";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchLibraryFilesArgs {
    q: Option<String>,
    limit: Option<u32>,
    cursor: Option<String>,
    category: Option<String>,
    state: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ListLibraryDirectoryNodesArgs {
    parent_directory_id: Option<String>,
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateLibraryFileArgs {
    file_name: String,
    content: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DownloadLibraryFileArgs {
    file_id: String,
    file_name: String,
    library_file_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WritebackLibraryFileArgs {
    local_path: String,
    file_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateFileEntryResponse {
    file_id: String,
    upload_url: String,
}

#[derive(Debug, Deserialize)]
struct DownloadLinkResponse {
    status: String,
    download_url: Option<String>,
    error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProcessUploadStreamEvent {
    event: Option<String>,
    message: Option<String>,
    extra: Option<ProcessUploadStreamExtra>,
}

#[derive(Debug, Deserialize)]
struct ProcessUploadStreamExtra {
    metadata_object_id: Option<String>,
    library_file_name: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct ProcessUploadStreamMetadata {
    library_file_id: Option<String>,
    library_file_name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct HydratedLibraryFileMetadata {
    thread_id: String,
    file_id: String,
    file_name: String,
    library_file_id: Option<String>,
    content_type: Option<String>,
    hydrated_at: String,
    source_sha256: String,
    source_size_bytes: u64,
}

#[derive(Debug, Clone)]
struct ChatGptAuthContext {
    access_token: String,
    account_id: Option<String>,
}

pub(crate) async fn handle_library_mcp_tool_call(
    sess: &Session,
    turn_context: &TurnContext,
    tool_name: &str,
    arguments_value: Option<JsonValue>,
) -> Result<CallToolResult, String> {
    let result = match tool_name {
        SEARCH_LIBRARY_FILES_TOOL_NAME => {
            let arguments = parse_arguments::<SearchLibraryFilesArgs>(tool_name, arguments_value)?;
            handle_search_library_files(sess, turn_context, arguments).await?
        }
        LIST_LIBRARY_DIRECTORY_NODES_TOOL_NAME => {
            let arguments =
                parse_arguments::<ListLibraryDirectoryNodesArgs>(tool_name, arguments_value)?;
            handle_list_library_directory_nodes(sess, turn_context, arguments).await?
        }
        CREATE_LIBRARY_FILE_TOOL_NAME => {
            let arguments = parse_arguments::<CreateLibraryFileArgs>(tool_name, arguments_value)?;
            handle_create_library_file(sess, turn_context, arguments).await?
        }
        DOWNLOAD_LIBRARY_FILE_TOOL_NAME => {
            let arguments = parse_arguments::<DownloadLibraryFileArgs>(tool_name, arguments_value)?;
            handle_download_library_file(sess, turn_context, arguments).await?
        }
        WRITEBACK_LIBRARY_FILE_TOOL_NAME => {
            let arguments =
                parse_arguments::<WritebackLibraryFileArgs>(tool_name, arguments_value)?;
            handle_writeback_library_file(sess, turn_context, arguments).await?
        }
        _ => {
            return Err(format!("Unsupported library MCP tool: {tool_name}"));
        }
    };
    build_json_tool_result(result)
}

fn parse_arguments<T>(tool_name: &str, arguments_value: Option<JsonValue>) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    let arguments = arguments_value.unwrap_or_else(|| serde_json::json!({}));
    serde_json::from_value(arguments)
        .map_err(|_| format!("{tool_name} received invalid arguments."))
}

async fn handle_search_library_files(
    sess: &Session,
    turn_context: &TurnContext,
    arguments: SearchLibraryFilesArgs,
) -> Result<JsonValue, String> {
    if let Some(limit) = arguments.limit
        && !(1..=200).contains(&limit)
    {
        return Err(format!(
            "{SEARCH_LIBRARY_FILES_TOOL_NAME} received invalid arguments."
        ));
    }

    let url = format!(
        "{}/files/library",
        turn_context.config.chatgpt_base_url.trim_end_matches('/')
    );
    send_chatgpt_json_request(
        sess,
        &url,
        Method::POST,
        Some(serde_json::json!({
            "limit": arguments.limit.unwrap_or(10),
            "cursor": arguments.cursor,
            "category": arguments.category,
            "state": arguments.state,
            "q": arguments.q,
        })),
    )
    .await
}

async fn handle_list_library_directory_nodes(
    sess: &Session,
    turn_context: &TurnContext,
    arguments: ListLibraryDirectoryNodesArgs,
) -> Result<JsonValue, String> {
    let mut url = Url::parse(&format!(
        "{}/files/library/nodes",
        turn_context.config.chatgpt_base_url.trim_end_matches('/')
    ))
    .map_err(|error| format!("invalid library nodes URL: {error}"))?;
    {
        let mut query_pairs = url.query_pairs_mut();
        if let Some(parent_directory_id) = arguments.parent_directory_id {
            query_pairs.append_pair("parent_directory_id", &parent_directory_id);
        }
        if let Some(cursor) = arguments.cursor {
            query_pairs.append_pair("cursor", &cursor);
        }
    }

    send_chatgpt_json_request(sess, url.as_str(), Method::GET, None).await
}

async fn handle_create_library_file(
    sess: &Session,
    turn_context: &TurnContext,
    arguments: CreateLibraryFileArgs,
) -> Result<JsonValue, String> {
    let file_size_bytes = arguments.content.as_bytes().len() as u64;
    if file_size_bytes > LIBRARY_FILE_MAX_SIZE_BYTES {
        return Err(format!(
            "File content is {file_size_bytes} bytes, exceeding the v1 maximum of {LIBRARY_FILE_MAX_SIZE_BYTES} bytes."
        ));
    }

    let file_entry =
        create_library_file_entry(sess, turn_context, &arguments.file_name, file_size_bytes)
            .await?;
    upload_text_to_library(file_entry.upload_url.as_str(), &arguments.content).await?;
    let processed = finalize_library_file_upload(
        sess,
        turn_context,
        &file_entry.file_id,
        &arguments.file_name,
    )
    .await?;

    Ok(serde_json::json!({
        "file_id": file_entry.file_id,
        "library_file_id": processed.library_file_id,
        "file_name": processed
            .library_file_name
            .unwrap_or(arguments.file_name),
        "file_size_bytes": file_size_bytes,
        "writeback_mode": "create_new",
    }))
}

async fn handle_download_library_file(
    sess: &Session,
    turn_context: &TurnContext,
    arguments: DownloadLibraryFileArgs,
) -> Result<JsonValue, String> {
    ensure_non_empty(&arguments.file_id, DOWNLOAD_LIBRARY_FILE_TOOL_NAME)?;
    ensure_non_empty(&arguments.file_name, DOWNLOAD_LIBRARY_FILE_TOOL_NAME)?;

    let download_url = fetch_library_download_url(sess, turn_context, &arguments.file_id).await?;
    let download_response = send_unsigned_request(Method::GET, &download_url, None, None).await?;
    let content_type = download_response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);

    let (local_path, sidecar_path) =
        hydrated_library_file_paths(turn_context, sess, &arguments.file_id, &arguments.file_name);
    hydrate_library_download(
        download_response,
        &local_path,
        &sidecar_path,
        HydratedLibraryFileMetadata {
            thread_id: sess.conversation_id.to_string(),
            file_id: arguments.file_id.clone(),
            file_name: arguments.file_name.clone(),
            library_file_id: arguments.library_file_id.clone(),
            content_type: content_type.clone(),
            hydrated_at: chrono::Utc::now().to_rfc3339(),
            source_sha256: String::new(),
            source_size_bytes: 0,
        },
    )
    .await?;

    Ok(serde_json::json!({
        "file_id": arguments.file_id,
        "library_file_id": arguments.library_file_id,
        "file_name": arguments.file_name,
        "local_path": local_path.display().to_string(),
        "sidecar_path": sidecar_path.display().to_string(),
        "content_type": content_type,
        "cache_scope": "per_thread",
    }))
}

async fn handle_writeback_library_file(
    sess: &Session,
    turn_context: &TurnContext,
    arguments: WritebackLibraryFileArgs,
) -> Result<JsonValue, String> {
    ensure_non_empty(&arguments.local_path, WRITEBACK_LIBRARY_FILE_TOOL_NAME)?;

    let local_path = turn_context
        .resolve_path(Some(arguments.local_path.clone()))
        .to_path_buf();
    let sidecar_path = sidecar_path_for_local_path(&local_path);
    let metadata = read_hydrated_library_metadata(&sidecar_path).await?;
    let (current_sha256, file_size_bytes) = sha256_for_file(&local_path).await?;

    if file_size_bytes > LIBRARY_FILE_MAX_SIZE_BYTES {
        return Err(format!(
            "Hydrated file is {file_size_bytes} bytes, exceeding the v1 maximum of {LIBRARY_FILE_MAX_SIZE_BYTES} bytes."
        ));
    }

    if current_sha256 == metadata.source_sha256 {
        return Ok(serde_json::json!({
            "writeback_mode": "create_new",
            "skipped": true,
            "reason": "unchanged",
            "local_path": local_path.display().to_string(),
            "source_file_id": metadata.file_id,
            "source_library_file_id": metadata.library_file_id,
            "source_sha256": metadata.source_sha256,
        }));
    }

    let target_file_name = arguments
        .file_name
        .unwrap_or_else(|| metadata.file_name.clone());
    let file_entry =
        create_library_file_entry(sess, turn_context, &target_file_name, file_size_bytes).await?;
    upload_local_file_to_library(
        file_entry.upload_url.as_str(),
        &local_path,
        file_size_bytes,
        metadata
            .content_type
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or("application/octet-stream"),
    )
    .await?;
    let processed =
        finalize_library_file_upload(sess, turn_context, &file_entry.file_id, &target_file_name)
            .await?;

    Ok(serde_json::json!({
        "writeback_mode": "create_new",
        "skipped": false,
        "source_file_id": metadata.file_id,
        "source_library_file_id": metadata.library_file_id,
        "new_file_id": file_entry.file_id,
        "new_library_file_id": processed.library_file_id,
        "file_name": processed
            .library_file_name
            .unwrap_or(target_file_name),
        "file_size_bytes": file_size_bytes,
        "local_path": local_path.display().to_string(),
        "source_sha256": metadata.source_sha256,
        "current_sha256": current_sha256,
    }))
}

async fn create_library_file_entry(
    sess: &Session,
    turn_context: &TurnContext,
    file_name: &str,
    file_size: u64,
) -> Result<CreateFileEntryResponse, String> {
    let timezone_offset_min = -(Local::now().offset().local_minus_utc() / 60);
    let url = format!(
        "{}/files",
        turn_context.config.chatgpt_base_url.trim_end_matches('/')
    );
    let body = send_chatgpt_json_request(
        sess,
        &url,
        Method::POST,
        Some(serde_json::json!({
            "file_name": file_name,
            "file_size": file_size,
            "use_case": LIBRARY_FILE_USE_CASE,
            "timezone_offset_min": timezone_offset_min,
            "reset_rate_limits": false,
            "store_in_library": true,
        })),
    )
    .await?;
    serde_json::from_value(body)
        .map_err(|error| format!("invalid create file entry response: {error}"))
}

async fn fetch_library_download_url(
    sess: &Session,
    turn_context: &TurnContext,
    file_id: &str,
) -> Result<String, String> {
    let url = format!(
        "{}/files/download/{file_id}",
        turn_context.config.chatgpt_base_url.trim_end_matches('/')
    );
    let body = send_chatgpt_json_request(sess, &url, Method::GET, None).await?;
    let response: DownloadLinkResponse = serde_json::from_value(body)
        .map_err(|error| format!("invalid library download response: {error}"))?;
    if response.status != "success" {
        return Err(response
            .error_message
            .unwrap_or_else(|| "Failed to get download URL for library file.".to_string()));
    }
    response
        .download_url
        .ok_or_else(|| "Failed to get download URL for library file.".to_string())
}

async fn upload_text_to_library(upload_url: &str, content: &str) -> Result<(), String> {
    let upload_url = resolve_library_upload_url(upload_url)?;
    send_unsigned_request(
        Method::PUT,
        upload_url.as_ref(),
        Some(vec![
            (
                "content-type".to_string(),
                "text/plain; charset=utf-8".to_string(),
            ),
            ("x-ms-blob-type".to_string(), "BlockBlob".to_string()),
            ("x-ms-version".to_string(), "2020-04-08".to_string()),
            (
                "x-ms-blob-content-type".to_string(),
                "text/plain; charset=utf-8".to_string(),
            ),
            (
                CONTENT_LENGTH.to_string(),
                content.as_bytes().len().to_string(),
            ),
        ]),
        Some(reqwest::Body::from(content.to_owned())),
    )
    .await?;
    Ok(())
}

async fn upload_local_file_to_library(
    upload_url: &str,
    path: &Path,
    file_size: u64,
    content_type: &str,
) -> Result<(), String> {
    let upload_url = resolve_library_upload_url(upload_url)?;
    let file = File::open(path)
        .await
        .map_err(|error| format!("failed to read `{}`: {error}", path.display()))?;
    send_unsigned_request(
        Method::PUT,
        upload_url.as_ref(),
        Some(vec![
            ("content-type".to_string(), content_type.to_string()),
            ("x-ms-blob-type".to_string(), "BlockBlob".to_string()),
            ("x-ms-version".to_string(), "2020-04-08".to_string()),
            (
                "x-ms-blob-content-type".to_string(),
                content_type.to_string(),
            ),
            (CONTENT_LENGTH.to_string(), file_size.to_string()),
        ]),
        Some(reqwest::Body::wrap_stream(ReaderStream::new(file))),
    )
    .await?;
    Ok(())
}

async fn finalize_library_file_upload(
    sess: &Session,
    turn_context: &TurnContext,
    file_id: &str,
    file_name: &str,
) -> Result<ProcessUploadStreamMetadata, String> {
    let url = format!(
        "{}/files/process_upload_stream",
        turn_context.config.chatgpt_base_url.trim_end_matches('/')
    );
    let response_text = send_chatgpt_text_request(
        sess,
        &url,
        Method::POST,
        Some(serde_json::json!({
            "file_id": file_id,
            "file_name": file_name,
            "use_case": LIBRARY_FILE_USE_CASE,
            "index_for_retrieval": false,
            "metadata": {
                "store_in_library": true,
            },
        })),
    )
    .await?;
    parse_process_upload_stream_response(&response_text)
}

async fn send_chatgpt_json_request(
    sess: &Session,
    url: &str,
    method: Method,
    body: Option<JsonValue>,
) -> Result<JsonValue, String> {
    let response_text = send_chatgpt_text_request(sess, url, method, body).await?;
    serde_json::from_str(&response_text)
        .map_err(|error| format!("invalid library response from {url}: {error}"))
}

async fn send_chatgpt_text_request(
    sess: &Session,
    url: &str,
    method: Method,
    body: Option<JsonValue>,
) -> Result<String, String> {
    let response = send_chatgpt_request(sess, url, method, body).await?;
    let status = response.status();
    let response_text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        let body = if response_text.is_empty() {
            "<empty body>".to_string()
        } else {
            response_text
        };
        return Err(format!(
            "library request to {url} failed with status {status}: {body}"
        ));
    }
    Ok(response_text)
}

async fn send_chatgpt_request(
    sess: &Session,
    url: &str,
    method: Method,
    body: Option<JsonValue>,
) -> Result<reqwest::Response, String> {
    let client = reqwest::Client::new();
    let mut refreshed_auth = false;
    let mut include_account_header = true;
    loop {
        let auth = current_chatgpt_auth(sess).await?;
        let account_id = auth.account_id.clone();
        let mut request = client
            .request(method.clone(), url)
            .timeout(LIBRARY_FILE_REQUEST_TIMEOUT)
            .bearer_auth(auth.access_token);
        if include_account_header && let Some(account_id) = account_id.as_deref() {
            request = request.header("chatgpt-account-id", account_id);
        }
        if let Some(ref body) = body {
            request = request.json(body);
        }
        let response = request
            .send()
            .await
            .map_err(|error| format!("failed to send library request to {url}: {error}"))?;
        match response.status() {
            StatusCode::UNAUTHORIZED if !refreshed_auth => {
                sess.services
                    .auth_manager
                    .refresh_token()
                    .await
                    .map_err(|error| {
                        format!("failed to refresh ChatGPT auth after 401 from {url}: {error}")
                    })?;
                refreshed_auth = true;
            }
            StatusCode::FORBIDDEN
                if include_account_header
                    && account_id.is_some()
                    && should_retry_without_account_header_after_403(url) =>
            {
                tracing::warn!(
                    "library request to {} returned 403 with chatgpt-account-id; retrying without account header for local dev",
                    url
                );
                include_account_header = false;
            }
            _ => return Ok(response),
        }
    }
}

async fn send_unsigned_request(
    method: Method,
    url: &str,
    headers: Option<Vec<(String, String)>>,
    body: Option<reqwest::Body>,
) -> Result<reqwest::Response, String> {
    let client = reqwest::Client::new();
    let mut request = client
        .request(method, url)
        .timeout(LIBRARY_FILE_REQUEST_TIMEOUT);
    if let Some(headers) = headers {
        for (key, value) in headers {
            request = request.header(&key, value);
        }
    }
    if let Some(body) = body {
        request = request.body(body);
    }
    let response = request
        .send()
        .await
        .map_err(|error| format!("failed to send library request to {url}: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        let response_text = response.text().await.unwrap_or_default();
        let body = if response_text.is_empty() {
            "<empty body>".to_string()
        } else {
            response_text
        };
        return Err(format!(
            "library request to {url} failed with status {status}: {body}"
        ));
    }
    Ok(response)
}

async fn current_chatgpt_auth(sess: &Session) -> Result<ChatGptAuthContext, String> {
    let auth = sess
        .services
        .auth_manager
        .auth()
        .await
        .ok_or_else(|| "ChatGPT auth is required for library file tools".to_string())?;
    let token_data = auth
        .get_token_data()
        .map_err(|error| format!("failed to read ChatGPT auth for library file tools: {error}"))?;
    Ok(ChatGptAuthContext {
        access_token: token_data.access_token,
        account_id: token_data.account_id,
    })
}

fn should_retry_without_account_header_after_403(url: &str) -> bool {
    Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_owned))
        .is_some_and(|host| matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1"))
}

fn resolve_library_upload_url(upload_url: &str) -> Result<String, String> {
    if !upload_url.to_ascii_lowercase().contains("estuary") {
        return Ok(upload_url.to_string());
    }
    let parsed = Url::parse(upload_url)
        .map_err(|error| format!("invalid combined upload URL `{upload_url}`: {error}"))?;
    parsed
        .query_pairs()
        .find_map(|(key, value)| (key == "upload_url").then(|| value.into_owned()))
        .ok_or_else(|| "Combined upload URL missing embedded Azure upload_url.".to_string())
}

fn parse_process_upload_stream_response(body: &str) -> Result<ProcessUploadStreamMetadata, String> {
    let trimmed_body = body.trim();
    if trimmed_body.is_empty() {
        return Ok(ProcessUploadStreamMetadata::default());
    }

    if let Ok(value) = serde_json::from_str::<JsonValue>(trimmed_body) {
        return parse_process_upload_stream_value(value);
    }

    let mut metadata = ProcessUploadStreamMetadata::default();
    for raw_line in trimmed_body.lines() {
        let Some(line) = normalize_process_upload_stream_line(raw_line) else {
            continue;
        };
        let value: JsonValue = serde_json::from_str(line)
            .map_err(|error| format!("invalid process_upload_stream response line: {error}"))?;
        metadata.merge(parse_process_upload_stream_value(value)?);
    }
    Ok(metadata)
}

fn parse_process_upload_stream_value(
    value: JsonValue,
) -> Result<ProcessUploadStreamMetadata, String> {
    if let Some(base64) = value.get("base64") {
        let Some(base64) = base64.as_str() else {
            return Err(
                "invalid process_upload_stream response: `base64` must be a string".to_string(),
            );
        };
        return parse_process_upload_stream_base64_payload(base64);
    }

    let event: ProcessUploadStreamEvent = serde_json::from_value(value)
        .map_err(|error| format!("invalid process_upload_stream event: {error}"))?;
    parse_process_upload_stream_event(event)
}

fn parse_process_upload_stream_base64_payload(
    base64: &str,
) -> Result<ProcessUploadStreamMetadata, String> {
    let bytes = BASE64_STANDARD
        .decode(base64.as_bytes())
        .map_err(|error| format!("invalid process_upload_stream base64 payload: {error}"))?;
    let text = String::from_utf8(bytes)
        .map_err(|error| format!("invalid process_upload_stream utf8 payload: {error}"))?;
    let mut metadata = ProcessUploadStreamMetadata::default();

    for raw_line in text.lines() {
        let Some(line) = normalize_process_upload_stream_line(raw_line) else {
            continue;
        };
        let event: ProcessUploadStreamEvent = serde_json::from_str(line)
            .map_err(|error| format!("invalid process_upload_stream event: {error}"))?;
        metadata.merge(parse_process_upload_stream_event(event)?);
    }

    Ok(metadata)
}

fn normalize_process_upload_stream_line(raw_line: &str) -> Option<&str> {
    let line = raw_line.trim();
    if line.is_empty() || line.starts_with("event:") {
        return None;
    }
    Some(line.strip_prefix("data:").map(str::trim).unwrap_or(line))
}

fn parse_process_upload_stream_event(
    event: ProcessUploadStreamEvent,
) -> Result<ProcessUploadStreamMetadata, String> {
    if is_process_upload_stream_error_event(event.event.as_deref()) {
        return Err(event
            .message
            .unwrap_or_else(|| "Library upload processing failed.".to_string()));
    }

    let mut metadata = ProcessUploadStreamMetadata::default();
    if let Some(extra) = event.extra {
        if let Some(library_file_id) = extra.metadata_object_id {
            metadata.library_file_id = Some(library_file_id);
        }
        if let Some(library_file_name) = extra.library_file_name {
            metadata.library_file_name = Some(library_file_name);
        }
    }
    Ok(metadata)
}

fn is_process_upload_stream_error_event(event: Option<&str>) -> bool {
    matches!(
        event,
        Some(
            "file.processing.error"
                | "file.processing.cancelled"
                | "file.unknown"
                | "file.indexing.error"
                | "file.indexing.cancelled"
                | "error"
                | "cancelled"
        )
    ) || event.is_some_and(|value| {
        value.ends_with("_error")
            || value.ends_with("_cancelled")
            || value.ends_with(".error")
            || value.ends_with(".cancelled")
    })
}

impl ProcessUploadStreamMetadata {
    fn merge(&mut self, other: ProcessUploadStreamMetadata) {
        if let Some(library_file_id) = other.library_file_id {
            self.library_file_id = Some(library_file_id);
        }
        if let Some(library_file_name) = other.library_file_name {
            self.library_file_name = Some(library_file_name);
        }
    }
}

async fn hydrate_library_download(
    mut response: reqwest::Response,
    local_path: &Path,
    sidecar_path: &Path,
    mut metadata: HydratedLibraryFileMetadata,
) -> Result<(), String> {
    if let Some(parent) = local_path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("failed to create `{}`: {error}", parent.display()))?;
    }

    let mut output = File::create(local_path)
        .await
        .map_err(|error| format!("failed to create `{}`: {error}", local_path.display()))?;
    let mut hasher = Sha256::new();
    let mut size_bytes = 0_u64;

    loop {
        let next_chunk = response
            .chunk()
            .await
            .map_err(|error| format!("failed to read library download body: {error}"))?;
        let Some(chunk) = next_chunk else {
            break;
        };
        size_bytes += chunk.len() as u64;
        hasher.update(&chunk);
        output
            .write_all(&chunk)
            .await
            .map_err(|error| format!("failed to write `{}`: {error}", local_path.display()))?;
    }
    output
        .flush()
        .await
        .map_err(|error| format!("failed to flush `{}`: {error}", local_path.display()))?;

    metadata.source_sha256 = format!("{:x}", hasher.finalize());
    metadata.source_size_bytes = size_bytes;
    let sidecar_bytes = serde_json::to_vec_pretty(&metadata)
        .map_err(|error| format!("failed to serialize library sidecar metadata: {error}"))?;
    fs::write(sidecar_path, sidecar_bytes)
        .await
        .map_err(|error| format!("failed to write `{}`: {error}", sidecar_path.display()))?;

    Ok(())
}

async fn read_hydrated_library_metadata(
    path: &Path,
) -> Result<HydratedLibraryFileMetadata, String> {
    let contents = fs::read(path)
        .await
        .map_err(|error| format!("failed to read `{}`: {error}", path.display()))?;
    serde_json::from_slice(&contents)
        .map_err(|error| format!("failed to parse `{}`: {error}", path.display()))
}

async fn sha256_for_file(path: &Path) -> Result<(String, u64), String> {
    let mut file = File::open(path)
        .await
        .map_err(|error| format!("failed to read `{}`: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut size_bytes = 0_u64;

    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|error| format!("failed to read `{}`: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size_bytes += read as u64;
    }

    Ok((format!("{:x}", hasher.finalize()), size_bytes))
}

fn hydrated_library_file_paths(
    turn_context: &TurnContext,
    sess: &Session,
    file_id: &str,
    file_name: &str,
) -> (PathBuf, PathBuf) {
    let root = turn_context
        .cwd
        .to_path_buf()
        .join(HYDRATION_ROOT_DIR_NAME)
        .join(HYDRATION_SUBDIR_NAME)
        .join(sanitize_library_file_path_segment(
            &sess.conversation_id.to_string(),
        ));
    let local_path = root.join(format!(
        "{}-{}",
        sanitize_library_file_path_segment(file_id),
        sanitize_library_file_path_segment(file_name),
    ));
    let sidecar_path = sidecar_path_for_local_path(&local_path);
    (local_path, sidecar_path)
}

fn sidecar_path_for_local_path(path: &Path) -> PathBuf {
    let mut sidecar_name = path
        .file_name()
        .map(|value| value.to_os_string())
        .unwrap_or_default();
    sidecar_name.push(".codex-library.json");
    path.with_file_name(sidecar_name)
}

fn sanitize_library_file_path_segment(value: &str) -> String {
    let trimmed = value.trim();
    let mut normalized = String::with_capacity(trimmed.len());
    for character in trimmed.chars() {
        if matches!(
            character,
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
        ) || character <= '\u{1f}'
        {
            normalized.push('_');
        } else {
            normalized.push(character);
        }
    }

    let normalized = if normalized.chars().all(|character| character == '.') {
        "_".to_string()
    } else {
        normalized
    };

    if normalized.is_empty() {
        "library-file".to_string()
    } else {
        normalized
    }
}

fn ensure_non_empty(value: &str, tool_name: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{tool_name} received invalid arguments."));
    }
    Ok(())
}

fn build_json_tool_result(data: JsonValue) -> Result<CallToolResult, String> {
    let text = serde_json::to_string_pretty(&data)
        .map_err(|error| format!("failed to serialize library tool result: {error}"))?;
    Ok(CallToolResult {
        content: vec![serde_json::json!({
            "type": "text",
            "text": text,
        })],
        structured_content: Some(data),
        is_error: None,
        meta: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::make_session_and_context;
    use crate::mcp_tool_call::handle_mcp_tool_call;
    use crate::test_support::auth_manager_from_auth;
    use codex_login::CodexAuth;
    use codex_protocol::protocol::AskForApproval;
    use codex_protocol::protocol::SandboxPolicy;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;
    use tempfile::tempdir;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::Request;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::body_json;
    use wiremock::matchers::header;
    use wiremock::matchers::header_exists;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    #[tokio::test]
    async fn library_search_tool_requires_chatgpt_auth() {
        let (session, mut turn_context) = make_session_and_context().await;
        turn_context
            .approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn_context
            .sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let result = handle_mcp_tool_call(
            Arc::new(session),
            &Arc::new(turn_context),
            "call-1".to_string(),
            codex_mcp::CODEX_APPS_MCP_SERVER_NAME.to_string(),
            SEARCH_LIBRARY_FILES_TOOL_NAME.to_string(),
            "{}".to_string(),
        )
        .await;

        assert_eq!(result.is_error, Some(true));
        assert!(
            result
                .content
                .first()
                .and_then(|value| value.get("text"))
                .and_then(JsonValue::as_str)
                .is_some_and(|text| {
                    text.contains("ChatGPT auth") || text.contains("Token data is not available")
                }),
            "expected auth error result: {result:?}"
        );
    }

    #[tokio::test]
    async fn library_search_tool_uses_chatgpt_auth_headers() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/backend-api/files/library"))
            .and(header("authorization", "Bearer Access Token"))
            .and(header("chatgpt-account-id", "account_id"))
            .and(body_json(serde_json::json!({
                "limit": 5,
                "cursor": null,
                "category": null,
                "state": null,
                "q": "alpha",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{
                    "id": "library_1",
                    "file_id": "file_1",
                    "file_name": "alpha.txt",
                }],
                "cursor": null,
            })))
            .expect(1)
            .mount(&server)
            .await;

        let (mut session, mut turn_context) = make_session_and_context().await;
        session.services.auth_manager =
            auth_manager_from_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing());
        turn_context
            .approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn_context
            .sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");
        let mut config = (*turn_context.config).clone();
        config.chatgpt_base_url = format!("{}/backend-api", server.uri());
        turn_context.config = Arc::new(config);

        let result = handle_mcp_tool_call(
            Arc::new(session),
            &Arc::new(turn_context),
            "call-2".to_string(),
            codex_mcp::CODEX_APPS_MCP_SERVER_NAME.to_string(),
            SEARCH_LIBRARY_FILES_TOOL_NAME.to_string(),
            serde_json::json!({
                "q": "alpha",
                "limit": 5,
            })
            .to_string(),
        )
        .await;

        assert_eq!(result.is_error, None);
        assert_eq!(
            result.structured_content,
            Some(serde_json::json!({
                "items": [{
                    "id": "library_1",
                    "file_id": "file_1",
                    "file_name": "alpha.txt",
                }],
                "cursor": null,
            }))
        );
    }

    #[tokio::test]
    async fn library_search_tool_retries_without_account_header_after_local_403() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/backend-api/files/library"))
            .and(header("authorization", "Bearer Access Token"))
            .and(header_exists("chatgpt-account-id"))
            .and(body_json(serde_json::json!({
                "limit": 5,
                "cursor": null,
                "category": null,
                "state": null,
                "q": "alpha",
            })))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "detail": "Forbidden",
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/backend-api/files/library"))
            .and(header("authorization", "Bearer Access Token"))
            .and(|request: &Request| !request.headers.contains_key("chatgpt-account-id"))
            .and(body_json(serde_json::json!({
                "limit": 5,
                "cursor": null,
                "category": null,
                "state": null,
                "q": "alpha",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{
                    "id": "library_1",
                    "file_id": "file_1",
                    "file_name": "alpha.txt",
                }],
                "cursor": null,
            })))
            .expect(1)
            .mount(&server)
            .await;

        let (mut session, mut turn_context) = make_session_and_context().await;
        session.services.auth_manager =
            auth_manager_from_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing());
        turn_context
            .approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn_context
            .sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");
        let mut config = (*turn_context.config).clone();
        config.chatgpt_base_url = format!("{}/backend-api", server.uri());
        turn_context.config = Arc::new(config);

        let result = handle_mcp_tool_call(
            Arc::new(session),
            &Arc::new(turn_context),
            "call-3".to_string(),
            codex_mcp::CODEX_APPS_MCP_SERVER_NAME.to_string(),
            SEARCH_LIBRARY_FILES_TOOL_NAME.to_string(),
            serde_json::json!({
                "q": "alpha",
                "limit": 5,
            })
            .to_string(),
        )
        .await;

        assert_eq!(result.is_error, None);
        assert_eq!(
            result.structured_content,
            Some(serde_json::json!({
                "items": [{
                    "id": "library_1",
                    "file_id": "file_1",
                    "file_name": "alpha.txt",
                }],
                "cursor": null,
            }))
        );
    }

    #[test]
    fn library_search_local_403_retry_is_limited_to_local_hosts() {
        assert!(should_retry_without_account_header_after_403(
            "http://localhost:8000/api/files/library"
        ));
        assert!(should_retry_without_account_header_after_403(
            "http://127.0.0.1:8000/api/files/library"
        ));
        assert!(!should_retry_without_account_header_after_403(
            "https://chatgpt.com/backend-api/files/library"
        ));
        assert!(!should_retry_without_account_header_after_403("not-a-url"));
    }

    #[test]
    fn process_upload_stream_parser_accepts_single_base64_envelope() {
        let payload = BASE64_STANDARD.encode(
            r#"{"event":"file.processing.completed","message":"done","extra":{"metadata_object_id":"library_new","library_file_name":"edited.txt"}}"#,
        );
        let response_text = serde_json::json!({
            "base64": payload,
        })
        .to_string();

        let metadata =
            parse_process_upload_stream_response(&response_text).expect("parse should succeed");

        assert_eq!(metadata.library_file_id.as_deref(), Some("library_new"));
        assert_eq!(metadata.library_file_name.as_deref(), Some("edited.txt"));
    }

    #[test]
    fn process_upload_stream_parser_accepts_ndjson_status_lines() {
        let response_text = concat!(
            r#"{"file_id":"file_new","event":"file.processing.started","message":"started"}"#,
            "\n",
            r#"{"file_id":"file_new","event":"file.processing.completed","message":"done","extra":{"metadata_object_id":"library_new","library_file_name":"edited.txt"}}"#,
            "\n"
        );

        let metadata =
            parse_process_upload_stream_response(response_text).expect("parse should succeed");

        assert_eq!(metadata.library_file_id.as_deref(), Some("library_new"));
        assert_eq!(metadata.library_file_name.as_deref(), Some("edited.txt"));
    }

    #[test]
    fn process_upload_stream_parser_accepts_multiple_base64_envelope_lines() {
        let started =
            BASE64_STANDARD.encode(r#"{"event":"file.processing.started","message":"started"}"#);
        let completed = BASE64_STANDARD.encode(
            r#"{"event":"file.processing.completed","message":"done","extra":{"metadata_object_id":"library_new","library_file_name":"edited.txt"}}"#,
        );
        let response_text = format!(r#"{{"base64":"{started}"}}"#,)
            + "\n"
            + &format!(r#"{{"base64":"{completed}"}}"#,);

        let metadata =
            parse_process_upload_stream_response(&response_text).expect("parse should succeed");

        assert_eq!(metadata.library_file_id.as_deref(), Some("library_new"));
        assert_eq!(metadata.library_file_name.as_deref(), Some("edited.txt"));
    }

    #[tokio::test]
    async fn library_download_and_writeback_manage_hydrated_local_files() {
        let server = MockServer::start().await;
        let download_url = format!("{}/signed/file_src", server.uri());
        Mock::given(method("GET"))
            .and(path("/backend-api/files/download/file_src"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "download_url": download_url,
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/signed/file_src"))
            .respond_with(
                ResponseTemplate::new(200)
                    .append_header("content-type", "text/plain")
                    .set_body_bytes(b"hello world".to_vec()),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/backend-api/files"))
            .and(body_json(serde_json::json!({
                "file_name": "edited.txt",
                "file_size": 12,
                "use_case": "codex",
                "timezone_offset_min": -(Local::now().offset().local_minus_utc() / 60),
                "reset_rate_limits": false,
                "store_in_library": true,
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "file_id": "file_new",
                "upload_url": format!("{}/upload/file_new", server.uri()),
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/upload/file_new"))
            .and(header("content-type", "text/plain"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;
        let finalize_payload = BASE64_STANDARD.encode(
            r#"{"event":"file.processed","extra":{"metadata_object_id":"library_new","library_file_name":"edited.txt"}}"#,
        );
        Mock::given(method("POST"))
            .and(path("/backend-api/files/process_upload_stream"))
            .and(body_json(serde_json::json!({
                "file_id": "file_new",
                "file_name": "edited.txt",
                "use_case": "codex",
                "index_for_retrieval": false,
                "metadata": {
                    "store_in_library": true,
                },
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "base64": finalize_payload,
            })))
            .expect(1)
            .mount(&server)
            .await;

        let (mut session, mut turn_context) = make_session_and_context().await;
        session.services.auth_manager =
            auth_manager_from_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing());
        turn_context
            .approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn_context
            .sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");
        let cwd = tempdir().expect("tempdir");
        turn_context.cwd =
            codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(cwd.path())
                .expect("absolute cwd");
        let mut config = (*turn_context.config).clone();
        config.chatgpt_base_url = format!("{}/backend-api", server.uri());
        turn_context.config = Arc::new(config);

        let session = Arc::new(session);
        let turn_context = Arc::new(turn_context);

        let download_result = handle_mcp_tool_call(
            Arc::clone(&session),
            &turn_context,
            "call-download".to_string(),
            codex_mcp::CODEX_APPS_MCP_SERVER_NAME.to_string(),
            DOWNLOAD_LIBRARY_FILE_TOOL_NAME.to_string(),
            serde_json::json!({
                "file_id": "file_src",
                "file_name": "report.txt",
                "library_file_id": "library_src",
            })
            .to_string(),
        )
        .await;
        let local_path = download_result
            .structured_content
            .as_ref()
            .and_then(|value| value.get("local_path"))
            .and_then(JsonValue::as_str)
            .expect("download result should include local_path")
            .to_string();
        let sidecar_path = download_result
            .structured_content
            .as_ref()
            .and_then(|value| value.get("sidecar_path"))
            .and_then(JsonValue::as_str)
            .expect("download result should include sidecar_path")
            .to_string();
        assert!(Path::new(&local_path).exists());
        assert!(Path::new(&sidecar_path).exists());

        let unchanged_result = handle_mcp_tool_call(
            Arc::clone(&session),
            &turn_context,
            "call-writeback-unchanged".to_string(),
            codex_mcp::CODEX_APPS_MCP_SERVER_NAME.to_string(),
            WRITEBACK_LIBRARY_FILE_TOOL_NAME.to_string(),
            serde_json::json!({
                "local_path": local_path,
            })
            .to_string(),
        )
        .await;
        assert_eq!(
            unchanged_result
                .structured_content
                .as_ref()
                .and_then(|value| value.get("skipped")),
            Some(&JsonValue::Bool(true))
        );

        fs::write(Path::new(&local_path), b"hello world!")
            .await
            .expect("update hydrated file");

        let changed_result = handle_mcp_tool_call(
            Arc::clone(&session),
            &turn_context,
            "call-writeback-changed".to_string(),
            codex_mcp::CODEX_APPS_MCP_SERVER_NAME.to_string(),
            WRITEBACK_LIBRARY_FILE_TOOL_NAME.to_string(),
            serde_json::json!({
                "local_path": local_path,
                "file_name": "edited.txt",
            })
            .to_string(),
        )
        .await;
        assert_eq!(
            changed_result
                .structured_content
                .as_ref()
                .and_then(|value| value.get("new_file_id")),
            Some(&JsonValue::String("file_new".to_string()))
        );
        assert_eq!(
            changed_result
                .structured_content
                .as_ref()
                .and_then(|value| value.get("new_library_file_id")),
            Some(&JsonValue::String("library_new".to_string()))
        );
    }
}
