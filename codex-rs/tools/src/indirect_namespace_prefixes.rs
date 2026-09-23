//! Adds namespace guidance to indirect tool descriptions after selection.
//! Conflicting prefixes for the same final namespace are rejected before rendering.
//! Direct specifications, search indexes, and cached definitions remain unchanged.

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
}

impl<'a> IndirectNamespacePrefixes<'a> {
    /// Expands configured server names using registered tools' final namespaces.
    /// Overlapping selectors must agree after trimming, including explicit empty values.
    pub fn new<'s>(
        prefixes: Option<&'a IndirectDescriptionPrefixes>,
        namespaces: impl IntoIterator<Item = (&'s str, &'s str)>,
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
                    let prefix = prefix.trim();
                    match resolved.entry(namespace.to_owned()) {
                        Entry::Vacant(entry) => {
                            entry.insert((prefix, format!("mcp_servers.{server}")));
                        }
                        Entry::Occupied(entry) if entry.get().0 != prefix => {
                            return Err(IndirectNamespacePrefixConflict {
                                namespace: namespace.to_owned(),
                                first_source: entry.get().1.clone(),
                                second_source: format!("mcp_servers.{server}"),
                            });
                        }
                        Entry::Occupied(_) => {}
                    }
                }
            }
        }
        Ok(Self {
            prefixes: resolved
                .into_iter()
                .filter(|(_, (prefix, _))| !prefix.is_empty())
                .map(|(namespace, (prefix, _))| (namespace, prefix))
                .collect(),
        })
    }

    /// ALL_TOOLS is flat, so each entry carries its namespace's guidance.
    /// Its eventual model-visible output is subject to normal tool-output truncation.
    pub fn apply_code_mode(&self, tools: &mut [ToolDefinition]) {
        if self.prefixes.is_empty() {
            return;
        }
        for tool in tools {
            if let Some(prefix) = self.prefix(namespace_name(&tool.tool_name)) {
                prepend(prefix, &mut tool.description);
            }
        }
    }

    /// Adds one prefix per namespace actually embedded in the exec description.
    pub fn apply_exec_prompt(
        &self,
        tools: &mut [ToolDefinition],
        namespaces: &mut BTreeMap<String, ToolNamespaceDescription>,
    ) {
        if self.prefixes.is_empty() {
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
        if self.prefixes.is_empty() {
            return;
        }
        for spec in tools {
            if let LoadableToolSpec::Namespace(namespace) = spec
                && let Some(prefix) = self.prefix(&namespace.name)
            {
                prepend(prefix, &mut namespace.description);
            }
        }
    }

    fn prefix(&self, namespace: &str) -> Option<&'a str> {
        self.prefixes.get(namespace).copied()
    }
}

fn prepend(prefix: &str, description: &mut String) {
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
