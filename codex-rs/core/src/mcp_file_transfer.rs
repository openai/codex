//! Adapts model-visible local paths to MCP SEP-2356/2631 file values.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use codex_mcp::FileInputSpec;
use codex_mcp::PrepareUploadParams;
use codex_protocol::mcp::CallToolResult;
use futures::StreamExt;
use reqwest::Method;
use reqwest::header::ACCEPT;
use reqwest::header::CONTENT_LENGTH;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::header::USER_AGENT;
use reqwest::redirect::Policy;
use serde_json::Value as JsonValue;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use url::Url;

use crate::session::session::Session;
use crate::session::turn_context::TurnContext;

const DEFAULT_MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
const TRANSFER_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(5 * 60);

#[tracing::instrument(
    name = "mcp_file_transfer.adapt_input",
    skip_all,
    fields(file_count = specs.len())
)]
pub(crate) async fn rewrite_mcp_file_arguments(
    sess: &Session,
    turn_context: &TurnContext,
    server: &str,
    arguments: Option<JsonValue>,
    specs: &[FileInputSpec],
) -> Result<Option<JsonValue>, String> {
    let Some(arguments) = arguments else {
        return Ok(None);
    };
    let Some(argument_object) = arguments.as_object() else {
        return Ok(Some(arguments));
    };
    let mut rewritten = argument_object.clone();
    for spec in specs.iter().filter(|spec| spec.is_mcp()) {
        let Some(value) = argument_object.get(&spec.path) else {
            continue;
        };
        rewritten.insert(
            spec.path.clone(),
            rewrite_file_value(sess, turn_context, server, spec, value).await?,
        );
    }
    Ok(Some(JsonValue::Object(rewritten)))
}

#[tracing::instrument(
    name = "mcp_file_transfer.materialize_output",
    skip_all,
    fields(file_count = tracing::field::Empty)
)]
pub(crate) async fn materialize_mcp_file_outputs(
    sess: &Session,
    turn_context: &TurnContext,
    server: &str,
    call_id: &str,
    mut result: CallToolResult,
) -> Result<CallToolResult, String> {
    let mut files = HashMap::<String, McpOutputFile>::new();
    for content in &result.content {
        collect_output_files(content, &mut files);
    }
    if let Some(structured_content) = result.structured_content.as_ref() {
        collect_output_files(structured_content, &mut files);
    }
    if files.is_empty() {
        return Ok(result);
    }
    tracing::Span::current().record("file_count", files.len());

    let output_dir = turn_context
        .config
        .codex_home
        .join("mcp-files")
        .join(sess.thread_id.to_string())
        .join(sanitize_filename(call_id));
    tokio::fs::create_dir_all(&output_dir)
        .await
        .map_err(|error| format!("failed to create MCP file output directory: {error}"))?;
    let manager = sess.services.mcp_connection_manager.load_full();
    let mut replacements = HashMap::new();
    for (uri, file) in files {
        let download = manager
            .get_file_download(server, uri.clone())
            .await
            .map_err(|error| format!("failed to prepare MCP file download: {error:#}"))?;
        validate_mcp_file_uri(&download.file.uri)?;
        if download.file.uri != uri {
            return Err("MCP download response returned a different file URI".to_string());
        }
        let name = sanitize_filename(
            download
                .file
                .name
                .as_deref()
                .or(file.name.as_deref())
                .unwrap_or("download"),
        );
        let output_path = unique_output_path(&output_dir, &name).await;
        let size = download_transfer_file(
            &download.transfer,
            &output_path,
            download
                .file
                .size
                .filter(|size| *size > 0)
                .unwrap_or(DEFAULT_MAX_FILE_BYTES)
                .min(DEFAULT_MAX_FILE_BYTES),
        )
        .await?;
        let local_uri = Url::from_file_path(&output_path)
            .map_err(|()| "failed to build local MCP file URI".to_string())?
            .to_string();
        replacements.insert(
            uri,
            serde_json::json!({
                "uri": local_uri,
                "name": name,
                "mimeType": download.file.mime_type.or(file.mime_type),
                "size": size,
            }),
        );
    }
    for content in &mut result.content {
        replace_output_files(content, &replacements);
    }
    if let Some(structured_content) = result.structured_content.as_mut() {
        replace_output_files(structured_content, &replacements);
    }
    Ok(result)
}

#[derive(Debug, Clone)]
struct McpOutputFile {
    name: Option<String>,
    mime_type: Option<String>,
}

fn collect_output_files(value: &JsonValue, files: &mut HashMap<String, McpOutputFile>) {
    match value {
        JsonValue::Array(values) => {
            for value in values {
                collect_output_files(value, files);
            }
        }
        JsonValue::Object(object) => {
            if let Some(uri) = object.get("uri").and_then(JsonValue::as_str)
                && uri.starts_with("mcp-file://")
            {
                files
                    .entry(uri.to_string())
                    .or_insert_with(|| McpOutputFile {
                        name: object
                            .get("name")
                            .and_then(JsonValue::as_str)
                            .map(str::to_string),
                        mime_type: object
                            .get("mimeType")
                            .or_else(|| object.get("mime_type"))
                            .and_then(JsonValue::as_str)
                            .map(str::to_string),
                    });
                return;
            }
            for value in object.values() {
                collect_output_files(value, files);
            }
        }
        _ => {}
    }
}

fn replace_output_files(value: &mut JsonValue, replacements: &HashMap<String, JsonValue>) {
    match value {
        JsonValue::Array(values) => {
            for value in values {
                replace_output_files(value, replacements);
            }
        }
        JsonValue::Object(object) => {
            if let Some(replacement) = object
                .get("uri")
                .and_then(JsonValue::as_str)
                .and_then(|uri| replacements.get(uri))
            {
                *value = replacement.clone();
                return;
            }
            for value in object.values_mut() {
                replace_output_files(value, replacements);
            }
        }
        _ => {}
    }
}

fn sanitize_filename(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .take(160)
        .collect::<String>();
    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        "download".to_string()
    } else {
        sanitized
    }
}

async fn unique_output_path(output_dir: &std::path::Path, name: &str) -> PathBuf {
    let initial = output_dir.join(name);
    if !tokio::fs::try_exists(&initial).await.unwrap_or(true) {
        return initial;
    }
    for index in 2..=10_000 {
        let candidate = output_dir.join(format!("{index}-{name}"));
        if !tokio::fs::try_exists(&candidate).await.unwrap_or(true) {
            return candidate;
        }
    }
    output_dir.join(format!("{}-{name}", uuid::Uuid::new_v4()))
}

async fn download_transfer_file(
    transfer: &codex_mcp::FileTransferDescriptor,
    output_path: &std::path::Path,
    max_size: u64,
) -> Result<u64, String> {
    let url = validated_transfer_descriptor(transfer, "GET")?;
    let response = transfer_client()?
        .get(url)
        .send()
        .await
        .map_err(|_| "MCP download transfer request failed".to_string())?;
    let status = response.status();
    let response = response
        .error_for_status()
        .map_err(|_| format!("MCP download transfer returned HTTP {status}"))?;
    if response
        .content_length()
        .is_some_and(|size| size > max_size)
    {
        return Err(format!("MCP download exceeds the {max_size}-byte limit"));
    }
    let temporary_path = output_path.with_extension("part");
    let result = async {
        let mut output = tokio::fs::File::create(&temporary_path)
            .await
            .map_err(|error| format!("failed to create MCP download: {error}"))?;
        let mut size = 0_u64;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| format!("failed to read MCP download: {error}"))?;
            size = size.saturating_add(chunk.len() as u64);
            if size > max_size {
                return Err(format!("MCP download exceeds the {max_size}-byte limit"));
            }
            output
                .write_all(&chunk)
                .await
                .map_err(|error| format!("failed to write MCP download: {error}"))?;
        }
        output
            .flush()
            .await
            .map_err(|error| format!("failed to flush MCP download: {error}"))?;
        drop(output);
        tokio::fs::rename(&temporary_path, output_path)
            .await
            .map_err(|error| format!("failed to finalize MCP download: {error}"))?;
        Ok(size)
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&temporary_path).await;
    }
    result
}

async fn rewrite_file_value(
    sess: &Session,
    turn_context: &TurnContext,
    server: &str,
    spec: &FileInputSpec,
    value: &JsonValue,
) -> Result<JsonValue, String> {
    if let Some(values) = value.as_array() {
        let mut rewritten = Vec::with_capacity(values.len());
        for value in values {
            rewritten.push(rewrite_single_file(sess, turn_context, server, spec, value).await?);
        }
        return Ok(JsonValue::Array(rewritten));
    }
    rewrite_single_file(sess, turn_context, server, spec, value).await
}

async fn rewrite_single_file(
    sess: &Session,
    turn_context: &TurnContext,
    server: &str,
    spec: &FileInputSpec,
    value: &JsonValue,
) -> Result<JsonValue, String> {
    let file_ref = model_file_ref(value).ok_or_else(|| {
        format!(
            "MCP file argument `{}` must be a local file path or file URI",
            spec.path
        )
    })?;
    if file_ref.starts_with("data:") || file_ref.starts_with("mcp-file://") {
        return Err(format!(
            "MCP file argument `{}` must reference a local file",
            spec.path
        ));
    }
    let path = resolve_file_path(turn_context, file_ref)?;
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|error| format!("failed to read `{}`: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("`{}` is not a regular file", path.display()));
    }
    let max_size = spec
        .max_size
        .unwrap_or(DEFAULT_MAX_FILE_BYTES)
        .min(DEFAULT_MAX_FILE_BYTES);
    if metadata.len() > max_size {
        return Err(format!(
            "file `{}` is {} bytes, exceeding the {max_size}-byte limit",
            path.display(),
            metadata.len()
        ));
    }
    let mime_type = mime_guess::from_path(&path)
        .first_raw()
        .unwrap_or("application/octet-stream");
    if !spec.accepts.is_empty()
        && !spec
            .accepts
            .iter()
            .any(|accept| mime_matches(accept, mime_type))
    {
        return Err(format!(
            "file `{}` has MIME type `{mime_type}`, which is not accepted by `{}`",
            path.display(),
            spec.path
        ));
    }
    let manager = sess.services.mcp_connection_manager.load_full();
    let capabilities = manager
        .file_capabilities(server)
        .await
        .map_err(|error| format!("failed to inspect MCP file capabilities: {error:#}"))?;
    if spec.accepts_upload() && capabilities.prepare_upload && capabilities.complete_upload {
        tracing::debug!(
            transfer_mode = "upload",
            size_bucket = file_size_bucket(metadata.len()),
            "adapting MCP file input"
        );
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("upload")
            .to_string();
        let prepared = manager
            .prepare_file_upload(
                server,
                PrepareUploadParams {
                    name,
                    mime_type: mime_type.to_string(),
                    size: metadata.len(),
                },
            )
            .await
            .map_err(|error| format!("failed to prepare MCP file upload: {error:#}"))?;
        validate_mcp_file_uri(&prepared.file.uri)?;
        put_transfer_file(&prepared.transfer, &path, max_size).await?;
        let completed = manager
            .complete_file_upload(server, prepared.file.uri)
            .await
            .map_err(|error| format!("failed to complete MCP file upload: {error:#}"))?;
        validate_mcp_file_uri(&completed.file.uri)?;
        return Ok(JsonValue::String(completed.file.uri));
    }
    if spec.accepts_inline() {
        tracing::debug!(
            transfer_mode = "inline",
            size_bucket = file_size_bucket(metadata.len()),
            "adapting MCP file input"
        );
        let file = tokio::fs::File::open(&path)
            .await
            .map_err(|error| format!("failed to open `{}`: {error}", path.display()))?;
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.take(max_size.saturating_add(1))
            .read_to_end(&mut bytes)
            .await
            .map_err(|error| format!("failed to read `{}`: {error}", path.display()))?;
        if bytes.len() as u64 > max_size {
            return Err(format!(
                "file `{}` exceeds the {max_size}-byte limit",
                path.display()
            ));
        }
        return Ok(JsonValue::String(format!(
            "data:{mime_type};base64,{}",
            STANDARD.encode(bytes)
        )));
    }
    Err(format!(
        "MCP file argument `{}` has no supported transfer mode",
        spec.path
    ))
}

fn model_file_ref(value: &JsonValue) -> Option<&str> {
    value.as_str().or_else(|| {
        value
            .as_object()
            .and_then(|value| value.get("uri"))
            .and_then(JsonValue::as_str)
    })
}

fn validate_mcp_file_uri(uri: &str) -> Result<(), String> {
    if uri.starts_with("mcp-file://") {
        Ok(())
    } else {
        Err("MCP file response returned an invalid file URI".to_string())
    }
}

fn mime_matches(accept: &str, mime_type: &str) -> bool {
    accept == "*/*"
        || accept == mime_type
        || accept
            .strip_suffix("/*")
            .is_some_and(|prefix| mime_type.starts_with(&format!("{prefix}/")))
}

fn file_size_bucket(size: u64) -> &'static str {
    match size {
        0..=65_535 => "lt_64_kib",
        65_536..=1_048_575 => "lt_1_mib",
        1_048_576..=10_485_759 => "lt_10_mib",
        _ => "gte_10_mib",
    }
}

fn resolve_file_path(turn_context: &TurnContext, file_ref: &str) -> Result<PathBuf, String> {
    if file_ref.starts_with("file:") {
        return Url::parse(file_ref)
            .map_err(|error| format!("invalid file URI: {error}"))?
            .to_file_path()
            .map_err(|()| "file URI does not identify a local path".to_string());
    }
    #[allow(deprecated)]
    Ok(turn_context
        .resolve_path(Some(file_ref.to_string()))
        .to_path_buf())
}

async fn put_transfer_file(
    transfer: &codex_mcp::FileTransferDescriptor,
    path: &std::path::Path,
    max_size: u64,
) -> Result<(), String> {
    let url = validated_upload_transfer_descriptor(transfer)?;
    let method = Method::from_bytes(transfer.method.as_bytes())
        .map_err(|error| format!("invalid MCP transfer method: {error}"))?;
    let size = tokio::fs::metadata(path)
        .await
        .map_err(|error| format!("failed to inspect MCP upload: {error}"))?
        .len();
    if size > max_size {
        return Err(format!("MCP upload exceeds the {max_size}-byte limit"));
    }
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|error| format!("failed to open MCP upload: {error}"))?;
    let stream = futures::stream::try_unfold((file, 0_u64), move |(mut file, sent)| async move {
        let mut buffer = vec![0_u8; 64 * 1024];
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            return Ok(None);
        }
        buffer.truncate(read);
        let sent = sent.saturating_add(read as u64);
        if sent > max_size {
            return Err(std::io::Error::other("MCP upload exceeded its size limit"));
        }
        Ok(Some((buffer, (file, sent))))
    });
    let azure_blob_upload = url.host_str().is_some_and(|host| {
        host.ends_with(".blob.core.windows.net") || host.ends_with(".oaiusercontent.com")
    });
    let mut request = transfer_client()?
        .request(method, url)
        .header(CONTENT_LENGTH, size)
        .body(reqwest::Body::wrap_stream(stream));
    if azure_blob_upload {
        request = request.header("x-ms-blob-type", "BlockBlob");
    }
    let response = request
        .send()
        .await
        .map_err(|_| "MCP upload transfer request failed".to_string())?;
    let status = response.status();
    response
        .error_for_status()
        .map_err(|_| format!("MCP upload transfer returned HTTP {status}"))?;
    Ok(())
}

fn validated_transfer_url(url: &str) -> Result<Url, String> {
    let url = Url::parse(url).map_err(|error| format!("invalid MCP transfer URL: {error}"))?;
    let local_http = cfg!(test)
        && url.scheme() == "http"
        && url
            .host_str()
            .is_some_and(|host| host == "localhost" || host == "127.0.0.1" || host == "::1");
    if url.scheme() != "https" && !local_http {
        return Err("MCP transfer URL must use HTTPS".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("MCP transfer URL must not contain credentials".to_string());
    }
    if let Some(url::Host::Ipv4(address)) = url.host()
        && (address.is_private()
            || address.is_link_local()
            || address.is_loopback()
            || address.is_unspecified())
        && !local_http
    {
        return Err("MCP transfer URL must not target a private address".to_string());
    }
    if let Some(url::Host::Ipv6(address)) = url.host()
        && (address.is_loopback()
            || address.is_unspecified()
            || (address.segments()[0] & 0xffc0) == 0xfe80)
        && !local_http
    {
        return Err("MCP transfer URL must not target a private address".to_string());
    }
    Ok(url)
}

fn validated_transfer_descriptor(
    transfer: &codex_mcp::FileTransferDescriptor,
    expected_method: &str,
) -> Result<Url, String> {
    if transfer
        .transport
        .as_deref()
        .is_some_and(|value| value != "https")
    {
        return Err("MCP transfer transport must be HTTPS".to_string());
    }
    if transfer.method != expected_method {
        return Err(format!("MCP transfer method must be {expected_method}"));
    }
    if let Some(expires_at) = transfer.expires_at.as_deref() {
        let expires_at = chrono::DateTime::parse_from_rfc3339(expires_at)
            .map_err(|error| format!("invalid MCP transfer expiry: {error}"))?;
        if expires_at <= chrono::Utc::now() {
            return Err("MCP transfer descriptor has expired".to_string());
        }
    }
    validated_transfer_url(&transfer.url)
}

fn validated_upload_transfer_descriptor(
    transfer: &codex_mcp::FileTransferDescriptor,
) -> Result<Url, String> {
    if !matches!(transfer.method.as_str(), "PUT" | "POST") {
        return Err("MCP upload transfer method must be PUT or POST".to_string());
    }
    validated_transfer_descriptor(transfer, &transfer.method)
}

fn transfer_client() -> Result<reqwest::Client, String> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Mozilla/5.0 Codex MCP File Transfer"),
    );
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .connect_timeout(TRANSFER_CONNECT_TIMEOUT)
        .timeout(TRANSFER_TIMEOUT)
        .no_proxy()
        .redirect(Policy::none())
        .build()
        .map_err(|error| format!("failed to build MCP transfer client: {error}"))?;
    Ok(client)
}

#[cfg(test)]
#[path = "mcp_file_transfer_tests.rs"]
mod tests;
