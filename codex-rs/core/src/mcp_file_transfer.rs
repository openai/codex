//! Adapts model-visible local paths to MCP SEP-2356/2631 file values.

use std::collections::HashMap;
use std::path::PathBuf;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use codex_mcp::FileInputSpec;
use codex_mcp::PrepareUploadParams;
use codex_protocol::mcp::CallToolResult;
use serde_json::Value as JsonValue;
use url::Url;

use crate::session::session::Session;
use crate::session::turn_context::TurnContext;

const DEFAULT_MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;

use self::http::download_transfer_file;
#[cfg(test)]
use self::http::is_disallowed_transfer_address;
#[cfg(test)]
use self::http::is_trusted_direct_transfer_host;
use self::http::put_transfer_file;
#[cfg(test)]
use self::http::validated_transfer_descriptor;

mod http;

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
            sess,
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
    let Some(turn_environment) = turn_context.environments.primary() else {
        return Err("no primary turn environment is available".to_string());
    };
    let path = resolve_file_path(turn_environment.cwd_uri(), file_ref)?;
    let path_display = path.to_url().to_string();
    let sandbox = turn_context.file_system_sandbox_context(
        /*additional_permissions*/ None,
        turn_environment.cwd_uri(),
    );
    let fs = turn_environment.environment.get_filesystem();
    let metadata = fs
        .get_metadata(&path, Some(&sandbox))
        .await
        .map_err(|error| format!("failed to read `{path_display}`: {error}"))?;
    if !metadata.is_file {
        return Err(format!("`{path_display}` is not a regular file"));
    }
    let max_size = spec
        .max_size
        .unwrap_or(DEFAULT_MAX_FILE_BYTES)
        .min(DEFAULT_MAX_FILE_BYTES);
    if metadata.size > max_size {
        return Err(format!(
            "file `{path_display}` is {} bytes, exceeding the {max_size}-byte limit",
            metadata.size
        ));
    }
    let bytes = fs
        .read_file(&path, Some(&sandbox))
        .await
        .map_err(|error| format!("failed to read `{path_display}`: {error}"))?;
    let size = bytes.len() as u64;
    if size > max_size {
        return Err(format!(
            "file `{path_display}` exceeds the {max_size}-byte limit"
        ));
    }
    let name = path.basename().unwrap_or_else(|| "upload".to_string());
    let mime_type = mime_guess::from_path(&name)
        .first_raw()
        .unwrap_or("application/octet-stream");
    if !spec.accepts.is_empty()
        && !spec
            .accepts
            .iter()
            .any(|accept| mime_matches(accept, mime_type))
    {
        return Err(format!(
            "file `{path_display}` has MIME type `{mime_type}`, which is not accepted by `{}`",
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
            size_bucket = file_size_bucket(size),
            "adapting MCP file input"
        );
        let prepared = manager
            .prepare_file_upload(
                server,
                PrepareUploadParams {
                    name,
                    mime_type: mime_type.to_string(),
                    size,
                },
            )
            .await
            .map_err(|error| format!("failed to prepare MCP file upload: {error:#}"))?;
        validate_mcp_file_uri(&prepared.file.uri)?;
        put_transfer_file(sess, &prepared.transfer, bytes, max_size).await?;
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
            size_bucket = file_size_bucket(size),
            "adapting MCP file input"
        );
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

fn resolve_file_path(
    cwd: &codex_utils_path_uri::PathUri,
    file_ref: &str,
) -> Result<codex_utils_path_uri::PathUri, String> {
    if file_ref.starts_with("file:") {
        return codex_utils_path_uri::PathUri::parse(file_ref)
            .map_err(|error| format!("invalid file URI: {error}"));
    }
    if std::path::Path::new(file_ref).is_absolute() {
        return codex_utils_path_uri::PathUri::from_path(file_ref)
            .map_err(|error| format!("invalid absolute file path: {error}"));
    }
    cwd.join(file_ref)
        .map_err(|error| format!("invalid file path `{file_ref}`: {error}"))
}

#[cfg(test)]
#[path = "mcp_file_transfer_tests.rs"]
mod tests;
