//! MCP file input schema parsing and model-visible schema adaptation.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;

use rmcp::model::ServerCapabilities;
use rmcp::model::Tool;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value as JsonValue;

const MCP_FILE_SCHEMA_EXTENSION: &str = "x-mcp-file";
const META_OPENAI_FILE_PARAMS: &str = "openai/fileParams";
const MCP_FILE_HANDLE_GUIDANCE: &str = "Pass an absolute local file path. Do not construct data: URIs, mcp-file:// handles, signed URLs, or file-service payloads.";
const OPENAI_FILE_HANDLE_GUIDANCE: &str = "This parameter expects an absolute local file path. If you want to upload a file, provide the absolute path to that file here.";

pub(crate) const METHOD_FILES_PREPARE_UPLOAD: &str = "files/prepareUpload";
pub(crate) const METHOD_FILES_COMPLETE_UPLOAD: &str = "files/completeUpload";
pub(crate) const METHOD_FILES_GET_DOWNLOAD: &str = "files/getDownload";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpFileValue {
    pub uri: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTransferDescriptor {
    #[serde(default)]
    pub transport: Option<String>,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareUploadParams {
    pub name: String,
    pub mime_type: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileUriParams {
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PrepareUploadResult {
    pub file: McpFileValue,
    pub transfer: FileTransferDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CompleteUploadResult {
    pub file: McpFileValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GetDownloadResult {
    pub file: McpFileValue,
    pub transfer: FileTransferDescriptor,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct McpFileCapabilities {
    pub prepare_upload: bool,
    pub complete_upload: bool,
    pub get_download: bool,
}

impl McpFileCapabilities {
    pub(crate) fn from_server_and_tools(
        capabilities: &ServerCapabilities,
        tools: &[crate::ToolInfo],
    ) -> Self {
        let extension = capabilities.extensions.as_ref().and_then(|extensions| {
            extensions
                .get("io.modelcontextprotocol/files")
                .or_else(|| extensions.get("files"))
        });
        if let Some(extension) = extension {
            return Self {
                prepare_upload: extension
                    .get("prepareUpload")
                    .and_then(JsonValue::as_bool)
                    .unwrap_or(true),
                complete_upload: extension
                    .get("completeUpload")
                    .and_then(JsonValue::as_bool)
                    .unwrap_or(true),
                get_download: extension
                    .get("getDownload")
                    .and_then(JsonValue::as_bool)
                    .unwrap_or(true),
            };
        }

        let exposes_upload = tools.iter().any(|tool| {
            file_input_specs(&tool.tool)
                .iter()
                .any(FileInputSpec::accepts_upload)
        });
        Self {
            prepare_upload: exposes_upload,
            complete_upload: exposes_upload,
            get_download: exposes_upload,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileInputSource {
    Mcp,
    OpenAiFileParams,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileTransferMode {
    Inline,
    Upload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileInputSpec {
    pub path: String,
    pub accepts: Vec<String>,
    pub max_size: Option<u64>,
    pub transfer_modes: BTreeSet<FileTransferMode>,
    pub sources: BTreeSet<FileInputSource>,
    pub is_array: bool,
}

impl FileInputSpec {
    pub fn accepts_inline(&self) -> bool {
        self.transfer_modes.contains(&FileTransferMode::Inline)
    }

    pub fn accepts_upload(&self) -> bool {
        self.transfer_modes.contains(&FileTransferMode::Upload)
    }

    pub fn is_mcp(&self) -> bool {
        self.sources.contains(&FileInputSource::Mcp)
    }

    pub fn is_openai_file_param(&self) -> bool {
        self.sources.contains(&FileInputSource::OpenAiFileParams)
    }
}

pub fn file_input_specs(tool: &Tool) -> Vec<FileInputSpec> {
    let mut specs = BTreeMap::<String, FileInputSpec>::new();
    if let Some(properties) = tool
        .input_schema
        .get("properties")
        .and_then(JsonValue::as_object)
    {
        for (path, property) in properties {
            let Some(extension) = property
                .get(MCP_FILE_SCHEMA_EXTENSION)
                .and_then(JsonValue::as_object)
            else {
                continue;
            };
            let transfer_modes = parse_transfer_modes(extension.get("transferModes"));
            specs.insert(
                path.clone(),
                FileInputSpec {
                    path: path.clone(),
                    accepts: string_array(extension.get("accept")),
                    max_size: extension.get("maxSize").and_then(JsonValue::as_u64),
                    transfer_modes,
                    sources: BTreeSet::from([FileInputSource::Mcp]),
                    is_array: property.get("type").and_then(JsonValue::as_str) == Some("array")
                        || property.get("items").is_some(),
                },
            );
        }
    }

    for path in declared_openai_file_params(tool.meta.as_deref()) {
        if let Some(spec) = specs.get_mut(&path) {
            spec.sources.insert(FileInputSource::OpenAiFileParams);
        } else {
            specs.insert(
                path.clone(),
                FileInputSpec {
                    path: path.clone(),
                    accepts: Vec::new(),
                    max_size: None,
                    transfer_modes: BTreeSet::new(),
                    sources: BTreeSet::from([FileInputSource::OpenAiFileParams]),
                    is_array: tool
                        .input_schema
                        .get("properties")
                        .and_then(JsonValue::as_object)
                        .and_then(|properties| properties.get(&path))
                        .is_some_and(|property| {
                            property.get("type").and_then(JsonValue::as_str) == Some("array")
                                || property.get("items").is_some()
                        }),
                },
            );
        }
    }
    specs.into_values().collect()
}

pub(crate) fn tool_with_file_input_schema(
    tool: &Tool,
    honor_openai_file_params: bool,
    mcp_file_transfer_enabled: bool,
) -> Tool {
    let specs = file_input_specs(tool);
    let visible_specs = specs
        .into_iter()
        .filter(|spec| {
            honor_openai_file_params && spec.is_openai_file_param()
                || mcp_file_transfer_enabled && spec.is_mcp()
        })
        .collect::<Vec<_>>();
    if visible_specs.is_empty() {
        return tool.clone();
    }

    let mut tool = tool.clone();
    let mut input_schema = JsonValue::Object(tool.input_schema.as_ref().clone());
    mask_file_input_schemas(&mut input_schema, &visible_specs);
    if let JsonValue::Object(input_schema) = input_schema {
        tool.input_schema = Arc::new(input_schema);
    }
    tool
}

fn parse_transfer_modes(value: Option<&JsonValue>) -> BTreeSet<FileTransferMode> {
    let values = match value {
        Some(JsonValue::Array(values)) => values.as_slice(),
        Some(_) => return BTreeSet::new(),
        None => return BTreeSet::from([FileTransferMode::Inline]),
    };
    values
        .iter()
        .filter_map(JsonValue::as_str)
        .filter_map(|value| match value {
            "inline" => Some(FileTransferMode::Inline),
            "upload" => Some(FileTransferMode::Upload),
            _ => None,
        })
        .collect()
}

fn string_array(value: Option<&JsonValue>) -> Vec<String> {
    value
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter_map(JsonValue::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn declared_openai_file_params(meta: Option<&Map<String, JsonValue>>) -> Vec<String> {
    meta.and_then(|meta| meta.get(META_OPENAI_FILE_PARAMS))
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter_map(JsonValue::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn mask_file_input_schemas(input_schema: &mut JsonValue, specs: &[FileInputSpec]) {
    let Some(properties) = input_schema
        .as_object_mut()
        .and_then(|schema| schema.get_mut("properties"))
        .and_then(JsonValue::as_object_mut)
    else {
        return;
    };
    for spec in specs {
        let Some(property) = properties.get_mut(&spec.path) else {
            continue;
        };
        let guidance = if spec.is_mcp() {
            MCP_FILE_HANDLE_GUIDANCE
        } else {
            OPENAI_FILE_HANDLE_GUIDANCE
        };
        let description = property
            .get("description")
            .and_then(JsonValue::as_str)
            .map(str::trim)
            .filter(|description| !description.is_empty())
            .map_or_else(
                || guidance.to_string(),
                |description| format!("{description} {guidance}"),
            );
        let mut masked = Map::new();
        masked.insert("description".to_string(), JsonValue::String(description));
        if spec.is_array {
            masked.insert("type".to_string(), JsonValue::String("array".to_string()));
            masked.insert("items".to_string(), serde_json::json!({ "type": "string" }));
        } else {
            masked.insert("type".to_string(), JsonValue::String("string".to_string()));
        }
        *property = JsonValue::Object(masked);
    }
}

#[cfg(test)]
#[path = "file_transfer_tests.rs"]
mod tests;
