//! Adds namespace guidance to indirect tool descriptions after selection.
//! Conflicting prefixes for the same final namespace are rejected before rendering.

use crate::LoadableToolSpec;
use crate::ToolName;
use codex_code_mode::ToolDefinition;
use codex_code_mode::ToolNamespaceDescription;
use codex_protocol::DEFAULT_FUNCTION_NAMESPACE;
use codex_protocol::openai_models::IndirectDescriptionPrefixes;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::btree_map::Entry;

/// Conflicting catalog selectors for the same namespace, without including their prompt text.
#[derive(Debug, thiserror::Error)]
#[error(
    "Conflicting indirect description prefixes for namespace `{namespace}`: `{first_source}` and `{second_source}`"
)]
pub struct IndirectNamespacePrefixConflict {
    namespace: String,
    first_source: String,
    second_source: String,
}

/// Borrowed model text applied only to fresh indirect presentation data.
pub struct IndirectNamespacePrefixes<'a> {
    prefixes: BTreeMap<String, &'a str>,
    // This selector annotates namespace headers only, including shared headers.
    builtin_headers: BTreeMap<&'a str, &'a str>,
}

impl<'a> IndirectNamespacePrefixes<'a> {
    /// Expands configured server names using registered tools' final namespaces.
    /// Overlapping selectors must agree after trimming, including explicit empty values.
    pub fn new<'s>(
        prefixes: Option<&'a IndirectDescriptionPrefixes>,
        namespaces: impl IntoIterator<Item = (&'s str, &'s str)>,
        builtin_tools: impl IntoIterator<Item = &'s ToolName>,
    ) -> Result<Self, IndirectNamespacePrefixConflict> {
        let mut resolved = BTreeMap::new();
        if let Some(namespaces) = prefixes.and_then(|prefixes| prefixes.namespaces.as_ref()) {
            for (namespace, prefix) in namespaces {
                resolved.insert(
                    namespace.clone(),
                    (prefix.trim(), format!("namespaces.{namespace}")),
                );
            }
        }
        if let Some(servers) = prefixes
            .and_then(|prefixes| prefixes.mcp_servers.as_ref())
            .filter(|servers| !servers.is_empty())
        {
            for (server, namespace) in namespaces {
                if let Some(prefix) = servers.get(server) {
                    insert_prefix(
                        &mut resolved,
                        namespace,
                        prefix,
                        format!("mcp_servers.{server}"),
                    )?;
                }
            }
        }
        let mut builtin_headers = BTreeMap::new();
        if let Some(configured) = prefixes.and_then(|p| p.builtin_tool_namespaces.as_ref())
            && !configured.is_empty()
        {
            let registered = builtin_tools
                .into_iter()
                .map(namespace_name)
                .collect::<BTreeSet<_>>();
            for (namespace, prefix) in configured {
                if !registered.contains(namespace.as_str()) {
                    continue;
                }
                if resolved.contains_key(namespace) {
                    insert_prefix(
                        &mut resolved,
                        namespace,
                        prefix,
                        format!("builtin_tool_namespaces.{namespace}"),
                    )?;
                } else if !prefix.trim().is_empty() {
                    builtin_headers.insert(namespace.as_str(), prefix.trim());
                }
            }
        }
        Ok(Self {
            builtin_headers,
            prefixes: resolved
                .into_iter()
                .filter(|(_, (prefix, _))| !prefix.is_empty())
                .map(|(namespace, (prefix, _))| (namespace, prefix))
                .collect(),
        })
    }

    pub fn is_empty(&self) -> bool {
        self.prefixes.is_empty() && self.builtin_headers.is_empty()
    }

    /// ALL_TOOLS is flat, so each entry carries its namespace's guidance.
    /// Its eventual model-visible output is subject to normal tool-output truncation.
    pub fn apply_code_mode(&self, tools: &mut [ToolDefinition]) {
        if self.prefixes.is_empty() {
            return;
        }
        for tool in tools {
            self.apply_code_mode_description(&tool.tool_name, &mut tool.description);
        }
    }

    /// Adds guidance to one owned description without copying the tool's schemas.
    pub fn apply_code_mode_description(&self, tool_name: &ToolName, description: &mut String) {
        if let Some(prefix) = self.flat_prefix(tool_name) {
            prepend(prefix, description);
        }
    }

    /// Apply guidance once to each namespace embedded in the Code Mode prompt.
    pub fn apply_exec_prompt(
        &self,
        tools: &mut [ToolDefinition],
        namespaces: &mut BTreeMap<String, ToolNamespaceDescription>,
    ) {
        if self.is_empty() {
            return;
        }
        let mut has_default_prefix = false;
        let rendered_namespaces = tools
            .iter()
            .map(|tool| namespace_name(&tool.tool_name))
            .collect::<BTreeSet<_>>();
        for name in rendered_namespaces {
            if let Some(prefix) = self.prefix(name) {
                let namespace = namespaces.entry(name.to_string()).or_insert_with(|| {
                    ToolNamespaceDescription {
                        name: name.to_string(),
                        description: String::new(),
                    }
                });
                prepend(prefix, &mut namespace.description);
                has_default_prefix |= name == DEFAULT_FUNCTION_NAMESPACE;
            }
        }
        if has_default_prefix {
            // Only temporary prompt definitions join this group; runtime names stay unchanged.
            for tool in tools {
                if tool
                    .tool_name
                    .namespace
                    .as_deref()
                    .is_none_or(str::is_empty)
                {
                    tool.tool_name.namespace = Some(DEFAULT_FUNCTION_NAMESPACE.to_string());
                }
            }
        }
    }

    /// Applies prefixes after search ranking and namespace coalescing.
    pub fn apply_search(&self, tools: &mut [LoadableToolSpec]) {
        if self.is_empty() {
            return;
        }
        for spec in tools {
            let LoadableToolSpec::Namespace(namespace) = spec else {
                continue;
            };
            if let Some(prefix) = self.prefix(&namespace.name) {
                prepend(prefix, &mut namespace.description);
            }
        }
    }

    /// Guidance attached to an individual entry in a flat Code Mode listing.
    fn flat_prefix(&self, tool_name: &ToolName) -> Option<&'a str> {
        self.prefixes.get(namespace_name(tool_name)).copied()
    }

    fn prefix(&self, namespace: &str) -> Option<&'a str> {
        self.prefixes
            .get(namespace)
            .or_else(|| self.builtin_headers.get(namespace))
            .copied()
    }
}

fn insert_prefix<'a>(
    resolved: &mut BTreeMap<String, (&'a str, String)>,
    namespace: &str,
    prefix: &'a str,
    source: String,
) -> Result<(), IndirectNamespacePrefixConflict> {
    let prefix = prefix.trim();
    match resolved.entry(namespace.to_owned()) {
        Entry::Vacant(entry) => {
            entry.insert((prefix, source));
        }
        Entry::Occupied(entry) if entry.get().0 != prefix => {
            return Err(IndirectNamespacePrefixConflict {
                namespace: namespace.to_owned(),
                first_source: entry.get().1.clone(),
                second_source: source,
            });
        }
        Entry::Occupied(_) => {}
    }
    Ok(())
}

pub(super) fn prepend(prefix: &str, description: &mut String) {
    *description = if description.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}\n\n{description}")
    };
}

fn namespace_name(name: &ToolName) -> &str {
    name.namespace
        .as_deref()
        .filter(|namespace| !namespace.is_empty())
        .unwrap_or(DEFAULT_FUNCTION_NAMESPACE)
}

#[cfg(test)]
#[path = "indirect_namespace_prefixes_tests.rs"]
mod tests;
