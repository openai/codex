//! MCP tool metadata, filtering, schema shaping, and name qualification.
//!
//! Raw MCP tool identities must be preserved for protocol calls, while
//! model-visible tool names must be sanitized, deduplicated, and kept within API
//! limits. This module owns that translation as well as the shared [`ToolInfo`]
//! type and helpers that adjust tool schemas before exposing them to the model.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use codex_config::McpServerConfig;
use codex_protocol::ToolName;
use rmcp::model::Tool;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value as JsonValue;
use sha1::Digest;
use sha1::Sha1;
use tracing::warn;

use crate::mcp::sanitize_responses_api_tool_name;

pub(crate) const MCP_TOOLS_CACHE_WRITE_DURATION_METRIC: &str =
    "codex.mcp.tools.cache_write.duration_ms";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    /// Raw MCP server name used for routing the tool call.
    pub server_name: String,
    /// Model-visible tool name used in Responses API tool declarations.
    #[serde(rename = "tool_name", alias = "callable_name")]
    pub callable_name: String,
    /// Model-visible namespace used for deferred tool loading.
    #[serde(rename = "tool_namespace", alias = "callable_namespace")]
    pub callable_namespace: String,
    /// Instructions from the MCP server initialize result.
    #[serde(default)]
    pub server_instructions: Option<String>,
    /// Raw MCP tool definition; `tool.name` is sent back to the MCP server.
    pub tool: Tool,
    pub connector_id: Option<String>,
    pub connector_name: Option<String>,
    #[serde(default)]
    pub plugin_display_names: Vec<String>,
    pub connector_description: Option<String>,
}

impl ToolInfo {
    pub fn canonical_tool_name(&self) -> ToolName {
        ToolName::namespaced(self.callable_namespace.clone(), self.callable_name.clone())
    }
}

pub fn declared_openai_file_input_param_names(
    meta: Option<&Map<String, JsonValue>>,
) -> Vec<String> {
    let Some(meta) = meta else {
        return Vec::new();
    };

    meta.get(META_OPENAI_FILE_PARAMS)
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter_map(JsonValue::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

/// A tool is allowed to be used if both are true:
/// 1. enabled is None (no allowlist is set) or the tool is explicitly enabled.
/// 2. The tool is not explicitly disabled.
#[derive(Default, Clone)]
pub(crate) struct ToolFilter {
    pub(crate) enabled: Option<HashSet<String>>,
    pub(crate) disabled: HashSet<String>,
}

impl ToolFilter {
    pub(crate) fn from_config(cfg: &McpServerConfig) -> Self {
        let enabled = cfg
            .enabled_tools
            .as_ref()
            .map(|tools| tools.iter().cloned().collect::<HashSet<_>>());
        let disabled = cfg
            .disabled_tools
            .as_ref()
            .map(|tools| tools.iter().cloned().collect::<HashSet<_>>())
            .unwrap_or_default();

        Self { enabled, disabled }
    }

    pub(crate) fn allows(&self, tool_name: &str) -> bool {
        if let Some(enabled) = &self.enabled
            && !enabled.contains(tool_name)
        {
            return false;
        }

        !self.disabled.contains(tool_name)
    }
}

/// Returns the model-visible view of a tool while preserving the raw metadata
/// used by execution. Keep cache entries raw and call this at manager return
/// boundaries.
pub(crate) fn tool_with_model_visible_input_schema(tool: &Tool) -> Tool {
    let file_params = declared_openai_file_input_param_names(tool.meta.as_deref());
    if file_params.is_empty() {
        return tool.clone();
    }

    let mut tool = tool.clone();
    let mut input_schema = JsonValue::Object(tool.input_schema.as_ref().clone());
    mask_input_schema_for_file_path_params(&mut input_schema, &file_params);
    if let JsonValue::Object(input_schema) = input_schema {
        tool.input_schema = Arc::new(input_schema);
    }
    tool
}

pub(crate) fn filter_tools(tools: Vec<ToolInfo>, filter: &ToolFilter) -> Vec<ToolInfo> {
    tools
        .into_iter()
        .filter(|tool| filter.allows(&tool.tool.name))
        .collect()
}

/// Returns a qualified-name lookup for MCP tools.
///
/// Raw MCP server/tool names are kept on each [`ToolInfo`] for protocol calls, while
/// `callable_namespace` / `callable_name` are sanitized and, when necessary, hashed so
/// every model-visible `mcp__namespace__tool` name is unique and <= 64 bytes.
pub(crate) fn qualify_tools<I>(tools: I) -> HashMap<String, ToolInfo>
where
    I: IntoIterator<Item = ToolInfo>,
{
    let mut seen_raw_names = HashSet::new();
    let mut candidates = Vec::new();
    for tool in tools {
        let raw_namespace_identity = format!(
            "{}\0{}\0{}",
            tool.server_name,
            tool.callable_namespace,
            tool.connector_id.as_deref().unwrap_or_default()
        );
        let raw_tool_identity = format!(
            "{}\0{}\0{}",
            raw_namespace_identity, tool.callable_name, tool.tool.name
        );
        if !seen_raw_names.insert(raw_tool_identity.clone()) {
            warn!("skipping duplicated tool {}", tool.tool.name);
            continue;
        }

        candidates.push(CallableToolCandidate {
            callable_namespace: sanitize_responses_api_tool_name(&tool.callable_namespace),
            callable_name: sanitize_responses_api_tool_name(&tool.callable_name),
            raw_namespace_identity,
            raw_tool_identity,
            tool,
        });
    }

    let mut namespace_identities_by_base = HashMap::<String, HashSet<String>>::new();
    for candidate in &candidates {
        namespace_identities_by_base
            .entry(candidate.callable_namespace.clone())
            .or_default()
            .insert(candidate.raw_namespace_identity.clone());
    }
    let colliding_namespaces = namespace_identities_by_base
        .into_iter()
        .filter_map(|(namespace, identities)| (identities.len() > 1).then_some(namespace))
        .collect::<HashSet<_>>();
    for candidate in &mut candidates {
        if colliding_namespaces.contains(&candidate.callable_namespace) {
            candidate.callable_namespace = append_namespace_hash_suffix(
                &candidate.callable_namespace,
                &candidate.raw_namespace_identity,
            );
        }
    }

    let mut tool_identities_by_base = HashMap::<(String, String), HashSet<String>>::new();
    for candidate in &candidates {
        tool_identities_by_base
            .entry((
                candidate.callable_namespace.clone(),
                candidate.callable_name.clone(),
            ))
            .or_default()
            .insert(candidate.raw_tool_identity.clone());
    }
    let colliding_tools = tool_identities_by_base
        .into_iter()
        .filter_map(|(key, identities)| (identities.len() > 1).then_some(key))
        .collect::<HashSet<_>>();
    for candidate in &mut candidates {
        if colliding_tools.contains(&(
            candidate.callable_namespace.clone(),
            candidate.callable_name.clone(),
        )) {
            candidate.callable_name =
                append_hash_suffix(&candidate.callable_name, &candidate.raw_tool_identity);
        }
    }

    candidates.sort_by(|left, right| left.raw_tool_identity.cmp(&right.raw_tool_identity));

    let mut used_names = HashSet::new();
    let mut qualified_tools = HashMap::new();
    for mut candidate in candidates {
        let (callable_namespace, callable_name, qualified_name) = unique_callable_parts(
            &candidate.callable_namespace,
            &candidate.callable_name,
            &candidate.raw_tool_identity,
            &mut used_names,
        );
        candidate.tool.callable_namespace = callable_namespace;
        candidate.tool.callable_name = callable_name;
        qualified_tools.insert(qualified_name, candidate.tool);
    }
    qualified_tools
}

#[derive(Debug)]
struct CallableToolCandidate {
    tool: ToolInfo,
    raw_namespace_identity: String,
    raw_tool_identity: String,
    callable_namespace: String,
    callable_name: String,
}

const MCP_TOOL_NAME_DELIMITER: &str = "__";
const MAX_TOOL_NAME_LENGTH: usize = 64;
const CALLABLE_NAME_HASH_LEN: usize = 12;
const META_OPENAI_FILE_PARAMS: &str = "openai/fileParams";

fn mask_input_schema_for_file_path_params(input_schema: &mut JsonValue, file_params: &[String]) {
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
        mask_input_property_schema(property_schema);
    }
}

fn mask_input_property_schema(schema: &mut JsonValue) {
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
    object.clear();
    object.insert("description".to_string(), JsonValue::String(description));
    if is_array {
        object.insert("type".to_string(), JsonValue::String("array".to_string()));
        object.insert("items".to_string(), serde_json::json!({ "type": "string" }));
    } else {
        object.insert("type".to_string(), JsonValue::String("string".to_string()));
    }
}

fn sha1_hex(s: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(s.as_bytes());
    let sha1 = hasher.finalize();
    format!("{sha1:x}")
}

fn callable_name_hash_suffix(raw_identity: &str) -> String {
    let hash = sha1_hex(raw_identity);
    format!("_{}", &hash[..CALLABLE_NAME_HASH_LEN])
}

fn append_hash_suffix(value: &str, raw_identity: &str) -> String {
    format!("{value}{}", callable_name_hash_suffix(raw_identity))
}

fn append_namespace_hash_suffix(namespace: &str, raw_identity: &str) -> String {
    if let Some(namespace) = namespace.strip_suffix(MCP_TOOL_NAME_DELIMITER) {
        format!(
            "{}{}{}",
            namespace,
            callable_name_hash_suffix(raw_identity),
            MCP_TOOL_NAME_DELIMITER
        )
    } else {
        append_hash_suffix(namespace, raw_identity)
    }
}

fn truncate_name(value: &str, max_len: usize) -> String {
    value.chars().take(max_len).collect()
}

fn fit_callable_parts_with_hash(
    namespace: &str,
    tool_name: &str,
    raw_identity: &str,
) -> (String, String) {
    let suffix = callable_name_hash_suffix(raw_identity);
    let max_tool_len = MAX_TOOL_NAME_LENGTH.saturating_sub(namespace.len());
    if max_tool_len >= suffix.len() {
        let prefix_len = max_tool_len - suffix.len();
        return (
            namespace.to_string(),
            format!("{}{}", truncate_name(tool_name, prefix_len), suffix),
        );
    }

    let max_namespace_len = MAX_TOOL_NAME_LENGTH - suffix.len();
    (truncate_name(namespace, max_namespace_len), suffix)
}

fn unique_callable_parts(
    namespace: &str,
    tool_name: &str,
    raw_identity: &str,
    used_names: &mut HashSet<String>,
) -> (String, String, String) {
    let qualified_name = format!("{namespace}{tool_name}");
    if qualified_name.len() <= MAX_TOOL_NAME_LENGTH && used_names.insert(qualified_name.clone()) {
        return (namespace.to_string(), tool_name.to_string(), qualified_name);
    }

    let mut attempt = 0_u32;
    loop {
        let hash_input = if attempt == 0 {
            raw_identity.to_string()
        } else {
            format!("{raw_identity}\0{attempt}")
        };
        let (namespace, tool_name) =
            fit_callable_parts_with_hash(namespace, tool_name, &hash_input);
        let qualified_name = format!("{namespace}{tool_name}");
        if used_names.insert(qualified_name.clone()) {
            return (namespace, tool_name, qualified_name);
        }
        attempt = attempt.saturating_add(1);
    }
}
