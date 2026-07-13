//! MCP tool metadata, filtering, and name normalization.
//!
//! Raw MCP tool identities must be preserved for protocol calls, while
//! model-visible tool names must be sanitized, deduplicated, and kept within API
//! limits. This module owns that translation as well as the shared [`ToolInfo`]
//! type.

use std::collections::HashMap;
use std::collections::HashSet;

use codex_config::McpServerConfig;
use codex_protocol::ToolName;
use rmcp::model::Tool;
use serde::Deserialize;
use serde::Serialize;
use sha1::Digest;
use sha1::Sha1;
use tracing::warn;

use crate::mcp::sanitize_responses_api_tool_name;

const LEGACY_MCP_TOOL_NAME_PREFIX: &str = "mcp__";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    /// Raw MCP server name used for routing the tool call.
    pub server_name: String,
    /// Whether calls routed to this server may run in parallel.
    #[serde(default)]
    pub supports_parallel_tool_calls: bool,
    /// MCP server origin used for telemetry and diagnostics, when known.
    #[serde(default)]
    pub server_origin: Option<String>,
    /// Model-visible tool name used in Responses API tool declarations.
    #[serde(rename = "tool_name", alias = "callable_name")]
    pub callable_name: String,
    /// Model-visible namespace used for deferred tool loading.
    #[serde(rename = "tool_namespace", alias = "callable_namespace")]
    pub callable_namespace: String,
    /// Model-visible namespace description.
    // Keep the old serialized field name readable for cached ToolInfo values.
    #[serde(default, alias = "connector_description")]
    pub namespace_description: Option<String>,
    /// Raw MCP tool definition; `tool.name` is sent back to the MCP server.
    pub tool: Tool,
    /// Optional provided-file fields accepted by each declared `openai/fileParams`
    /// argument. This is derived from the raw MCP schema before file arguments are
    /// masked as local paths for the model.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub openai_file_input_optional_fields: HashMap<String, Vec<String>>,
    pub connector_id: Option<String>,
    pub connector_name: Option<String>,
    #[serde(default)]
    pub plugin_display_names: Vec<String>,
}

impl ToolInfo {
    pub fn canonical_tool_name(&self) -> ToolName {
        ToolName::namespaced(self.callable_namespace.clone(), self.callable_name.clone())
    }
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

pub(crate) fn filter_tools(tools: Vec<ToolInfo>, filter: &ToolFilter) -> Vec<ToolInfo> {
    tools
        .into_iter()
        .filter(|tool| filter.allows(&tool.tool.name))
        .collect()
}

/// Returns MCP tools with model-visible names normalized.
///
/// Raw MCP server/tool names are kept on each [`ToolInfo`] for protocol calls, while
/// `callable_namespace` / `callable_name` are sanitized and, when necessary, hashed so
/// every model-visible name is unique and <= 64 bytes.
///
/// When `prefix_mcp_tool_names` is true, the historical `mcp__` namespace
/// prefix is added without restoring the old trailing `__` namespace suffix.
pub(crate) fn normalize_tools_for_model_with_prefix<I>(
    tools: I,
    prefix_mcp_tool_names: bool,
) -> Vec<ToolInfo>
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

        let callable_namespace = callable_namespace_with_prefix(
            &sanitize_responses_api_tool_name(&tool.callable_namespace),
            prefix_mcp_tool_names,
        );

        candidates.push(CallableToolCandidate {
            callable_namespace,
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
    let mut model_tools = Vec::new();
    for mut candidate in candidates {
        let (callable_namespace, callable_name) = unique_callable_parts(
            &candidate.callable_namespace,
            &candidate.callable_name,
            &candidate.raw_tool_identity,
            &mut used_names,
            MCP_TOOL_NAME_DELIMITER.len(),
        );
        candidate.tool.callable_namespace = callable_namespace;
        candidate.tool.callable_name = callable_name;
        model_tools.push(candidate.tool);
    }
    model_tools
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
fn callable_namespace_with_prefix(namespace: &str, prefix_mcp_tool_names: bool) -> String {
    if !prefix_mcp_tool_names || namespace.starts_with(LEGACY_MCP_TOOL_NAME_PREFIX) {
        namespace.to_string()
    } else {
        format!("{LEGACY_MCP_TOOL_NAME_PREFIX}{namespace}")
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
    reserved_len: usize,
) -> (String, String) {
    let suffix = callable_name_hash_suffix(raw_identity);
    let max_tool_len = MAX_TOOL_NAME_LENGTH.saturating_sub(namespace.len() + reserved_len);
    if max_tool_len >= suffix.len() {
        let prefix_len = max_tool_len - suffix.len();
        return (
            namespace.to_string(),
            format!("{}{}", truncate_name(tool_name, prefix_len), suffix),
        );
    }

    let max_namespace_len = MAX_TOOL_NAME_LENGTH.saturating_sub(suffix.len() + reserved_len);
    (truncate_name(namespace, max_namespace_len), suffix)
}

fn unique_callable_parts(
    namespace: &str,
    tool_name: &str,
    raw_identity: &str,
    used_names: &mut HashSet<String>,
    reserved_len: usize,
) -> (String, String) {
    let model_name = format!("{namespace}{tool_name}");
    if model_name.len() + reserved_len <= MAX_TOOL_NAME_LENGTH && used_names.insert(model_name) {
        return (namespace.to_string(), tool_name.to_string());
    }

    let mut attempt = 0_u32;
    loop {
        let hash_input = if attempt == 0 {
            raw_identity.to_string()
        } else {
            format!("{raw_identity}\0{attempt}")
        };
        let (namespace, tool_name) =
            fit_callable_parts_with_hash(namespace, tool_name, &hash_input, reserved_len);
        let model_name = format!("{namespace}{tool_name}");
        if used_names.insert(model_name) {
            return (namespace, tool_name);
        }
        attempt = attempt.saturating_add(1);
    }
}
