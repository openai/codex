//! Allocates model-visible MCP tool names while preserving raw MCP identities.

use std::collections::HashMap;
use std::collections::HashSet;

use sha1::Digest;
use sha1::Sha1;
use tracing::warn;

use crate::mcp::sanitize_responses_api_tool_name;
use crate::mcp_connection_manager::ToolInfo;
use codex_protocol::ToolName;

const MCP_TOOL_NAME_DELIMITER: &str = "__";
const MAX_TOOL_NAME_LENGTH: usize = 64;
const CALLABLE_NAME_HASH_LEN: usize = 12;

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
) -> (String, String) {
    let display_name = ToolName::namespaced(namespace, tool_name).display();
    if display_name.len() <= MAX_TOOL_NAME_LENGTH && used_names.insert(display_name) {
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
            fit_callable_parts_with_hash(namespace, tool_name, &hash_input);
        let display_name = ToolName::namespaced(&namespace, &tool_name).display();
        if used_names.insert(display_name) {
            return (namespace, tool_name);
        }
        attempt = attempt.saturating_add(1);
    }
}

#[derive(Debug)]
struct CallableToolCandidate {
    tool: ToolInfo,
    raw_namespace_identity: String,
    raw_tool_identity: String,
    callable_namespace: String,
    callable_name: String,
}

/// Returns a callable-name lookup for MCP tools.
///
/// Raw MCP server/tool names are kept on each [`ToolInfo`] for protocol calls, while
/// `callable_namespace` / `callable_name` are sanitized and, when necessary, hashed so
/// every model-visible `mcp__namespace__tool` name is unique and <= 64 bytes.
pub(crate) fn qualify_tools<I>(tools: I) -> HashMap<ToolName, ToolInfo>
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
        let (callable_namespace, callable_name) = unique_callable_parts(
            &candidate.callable_namespace,
            &candidate.callable_name,
            &candidate.raw_tool_identity,
            &mut used_names,
        );
        candidate.tool.callable_namespace = callable_namespace;
        candidate.tool.callable_name = callable_name;
        qualified_tools.insert(candidate.tool.canonical_tool_name(), candidate.tool);
    }
    qualified_tools
}
