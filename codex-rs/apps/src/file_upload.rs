use std::sync::Arc;

use codex_api::OPENAI_FILE_UPLOAD_LIMIT_BYTES;
use codex_api::SharedAuthProvider;
use codex_api::upload_openai_file;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::FileSystemReadStream;
use codex_exec_server::FileSystemSandboxContext;
use codex_mcp::SandboxState;
use codex_utils_path_uri::PathUri;
use rmcp::model::Tool;
use serde_json::Value as JsonValue;

pub(super) const META_OPENAI_FILE_PARAMS: &str = "openai/fileParams";

pub(super) struct AppsFileSupport {
    pub(super) chatgpt_base_url: String,
    pub(super) auth_provider: SharedAuthProvider,
    pub(super) environment_manager: Arc<EnvironmentManager>,
}

pub(super) fn declared_openai_file_input_param_names(tool: &Tool) -> Vec<String> {
    tool.meta
        .as_deref()
        .and_then(|meta| meta.get(META_OPENAI_FILE_PARAMS))
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter_map(JsonValue::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

pub(super) fn rewrite_tool_schema_for_local_file_paths(tool: &mut Tool, file_params: &[String]) {
    let mut input_schema = JsonValue::Object(tool.input_schema.as_ref().clone());
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
        rewrite_input_property_schema_as_local_file_path(property_schema);
    }
    if let JsonValue::Object(input_schema) = input_schema {
        tool.input_schema = Arc::new(input_schema);
    }
}

fn rewrite_input_property_schema_as_local_file_path(schema: &mut JsonValue) {
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
    let array_constraints = is_array.then(|| {
        ["minItems", "maxItems", "uniqueItems"]
            .into_iter()
            .filter_map(|key| {
                object
                    .get(key)
                    .cloned()
                    .map(|value| (key.to_string(), value))
            })
            .collect::<Vec<_>>()
    });
    object.clear();
    object.insert("description".to_string(), JsonValue::String(description));
    if is_array {
        object.insert("type".to_string(), JsonValue::String("array".to_string()));
        object.insert("items".to_string(), serde_json::json!({ "type": "string" }));
        object.extend(array_constraints.into_iter().flatten());
    } else {
        object.insert("type".to_string(), JsonValue::String("string".to_string()));
    }
}

pub(super) async fn rewrite_arguments_for_openai_files(
    file_support: &AppsFileSupport,
    sandbox_state: Option<&SandboxState>,
    arguments: Option<JsonValue>,
    file_params: &[String],
) -> Result<Option<JsonValue>, String> {
    let Some(arguments) = arguments else {
        return Ok(None);
    };
    let Some(argument_object) = arguments.as_object() else {
        return Ok(Some(arguments));
    };
    validate_file_argument_arrays(argument_object, file_params)?;
    let mut rewritten = argument_object.clone();
    for field_name in file_params {
        let Some(value) = argument_object.get(field_name) else {
            continue;
        };
        let Some(sandbox_state) = sandbox_state else {
            return Err(format!(
                "cannot upload `{field_name}` because the MCP caller did not provide sandbox state"
            ));
        };
        let Some(uploaded) =
            rewrite_file_argument_value(file_support, sandbox_state, field_name, value).await?
        else {
            continue;
        };
        rewritten.insert(field_name.clone(), uploaded);
    }

    if rewritten == *argument_object {
        Ok(Some(arguments))
    } else {
        Ok(Some(JsonValue::Object(rewritten)))
    }
}

fn validate_file_argument_arrays(
    arguments: &serde_json::Map<String, JsonValue>,
    file_params: &[String],
) -> Result<(), String> {
    for field_name in file_params {
        let Some(JsonValue::Array(values)) = arguments.get(field_name) else {
            continue;
        };
        if let Some((index, _)) = values
            .iter()
            .enumerate()
            .find(|(_, value)| !value.is_string())
        {
            return Err(format!(
                "cannot upload `{field_name}[{index}]`: expected a local file path string"
            ));
        }
    }
    Ok(())
}

async fn rewrite_file_argument_value(
    file_support: &AppsFileSupport,
    sandbox_state: &SandboxState,
    field_name: &str,
    value: &JsonValue,
) -> Result<Option<JsonValue>, String> {
    match value {
        JsonValue::String(file_path) => {
            let file = prepare_environment_file(
                file_support,
                sandbox_state,
                field_name,
                /*index*/ None,
                file_path,
            )
            .await?;
            Ok(Some(upload_environment_file(file_support, file).await?))
        }
        JsonValue::Array(values) => {
            let mut prepared = Vec::with_capacity(values.len());
            for (index, value) in values.iter().enumerate() {
                let Some(file_path) = value.as_str() else {
                    return Err(format!(
                        "cannot upload `{field_name}[{index}]`: expected a local file path string"
                    ));
                };
                prepared.push(
                    prepare_environment_file(
                        file_support,
                        sandbox_state,
                        field_name,
                        Some(index),
                        file_path,
                    )
                    .await?,
                );
            }
            let mut rewritten = Vec::with_capacity(prepared.len());
            for file in prepared {
                rewritten.push(upload_environment_file(file_support, file).await?);
            }
            Ok(Some(JsonValue::Array(rewritten)))
        }
        _ => Ok(None),
    }
}

struct PreparedEnvironmentFile {
    filesystem: Arc<dyn ExecutorFileSystem>,
    sandbox: FileSystemSandboxContext,
    path: PathUri,
    file_name: String,
    metadata_size: u64,
    field_name: String,
    index: Option<usize>,
    supplied_path: String,
}

async fn prepare_environment_file(
    file_support: &AppsFileSupport,
    sandbox_state: &SandboxState,
    field_name: &str,
    index: Option<usize>,
    file_path: &str,
) -> Result<PreparedEnvironmentFile, String> {
    let contextualize = |error: String| contextualize(field_name, index, file_path, error);
    let expected_instance_id = sandbox_state
        .environment_instance_id
        .as_deref()
        .ok_or_else(|| {
            contextualize("sandbox state is missing an environment instance id".into())
        })?;
    let environment = file_support
        .environment_manager
        .get_environment_instance(&sandbox_state.environment_id, expected_instance_id)
        .ok_or_else(|| {
            contextualize(format!(
                "environment `{}` was replaced after the sandbox state was captured",
                sandbox_state.environment_id
            ))
        })?;
    let path: PathUri = sandbox_state
        .sandbox_cwd
        .join(file_path)
        .map_err(|error| contextualize(error.to_string()))?;
    let mut sandbox = FileSystemSandboxContext::from_permission_profile_with_cwd(
        sandbox_state.permission_profile.clone(),
        sandbox_state.sandbox_cwd.clone(),
    );
    sandbox.use_legacy_landlock = sandbox_state.use_legacy_landlock;
    let filesystem = environment.get_filesystem();
    let metadata = filesystem
        .get_metadata(&path, Some(&sandbox))
        .await
        .map_err(|error| contextualize(error.to_string()))?;
    if !metadata.is_file {
        return Err(contextualize(format!(
            "path `{}` is not a file",
            path.inferred_native_path_string()
        )));
    }
    if metadata.size > OPENAI_FILE_UPLOAD_LIMIT_BYTES {
        return Err(contextualize(format!(
            "file is too large: {} bytes exceeds the limit of {} bytes",
            metadata.size, OPENAI_FILE_UPLOAD_LIMIT_BYTES
        )));
    }
    let file_name = path.basename().unwrap_or_else(|| "file".to_string());
    Ok(PreparedEnvironmentFile {
        filesystem,
        sandbox,
        path,
        file_name,
        metadata_size: metadata.size,
        field_name: field_name.to_string(),
        index,
        supplied_path: file_path.to_string(),
    })
}

async fn upload_environment_file(
    file_support: &AppsFileSupport,
    file: PreparedEnvironmentFile,
) -> Result<JsonValue, String> {
    let PreparedEnvironmentFile {
        filesystem,
        sandbox,
        path,
        file_name,
        metadata_size,
        field_name,
        index,
        supplied_path,
    } = file;
    let contextualize = |error: String| contextualize(&field_name, index, &supplied_path, error);
    let (file_size, contents) = if sandbox.should_run_in_sandbox() {
        // Platform-sandboxed filesystem reads are bounded but cannot stream. Keep the permission
        // check attached to the read, then upload the validated bytes as one body chunk.
        let contents = filesystem
            .read_file(&path, Some(&sandbox))
            .await
            .map_err(|error| contextualize(error.to_string()))?;
        let file_size = u64::try_from(contents.len()).unwrap_or(u64::MAX);
        if file_size > OPENAI_FILE_UPLOAD_LIMIT_BYTES {
            return Err(contextualize(format!(
                "file is too large: {file_size} bytes exceeds the limit of {OPENAI_FILE_UPLOAD_LIMIT_BYTES} bytes"
            )));
        }
        if file_size != metadata_size {
            tracing::debug!(
                path = %path,
                metadata_size,
                file_size,
                "file size changed between upload validation and read"
            );
        }
        let contents = FileSystemReadStream::new(futures::stream::once(async move {
            Ok::<_, std::io::Error>(contents.into())
        }));
        (file_size, contents)
    } else {
        let contents = filesystem
            .read_file_stream(&path, Some(&sandbox))
            .await
            .map_err(|error| contextualize(error.to_string()))?;
        (metadata_size, contents)
    };
    let uploaded = upload_openai_file(
        file_support.chatgpt_base_url.trim_end_matches('/'),
        file_support.auth_provider.as_ref(),
        file_name,
        file_size,
        contents,
    )
    .await
    .map_err(|error| contextualize(error.to_string()))?;
    Ok(serde_json::json!({
        "download_url": uploaded.download_url,
        "file_id": uploaded.file_id,
        "mime_type": uploaded.mime_type,
        "file_name": uploaded.file_name,
        "uri": uploaded.uri,
        "file_size_bytes": uploaded.file_size_bytes,
    }))
}

fn contextualize(field_name: &str, index: Option<usize>, file_path: &str, error: String) -> String {
    match index {
        Some(index) => {
            format!("failed to upload `{file_path}` for `{field_name}[{index}]`: {error}")
        }
        None => format!("failed to upload `{file_path}` for `{field_name}`: {error}"),
    }
}
