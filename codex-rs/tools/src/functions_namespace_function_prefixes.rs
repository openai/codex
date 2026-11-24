//! Prepares function descriptions before direct, Code Mode, or search rendering.
//! Only the `functions` namespace is affected; runtime-owned specs stay unchanged.

use crate::ResponsesApiNamespaceTool;
use crate::ToolName;
use crate::ToolSpec;
use crate::indirect_namespace_prefixes::prepend;
use codex_protocol::openai_models::ToolDescriptionPrefixes;
use std::collections::BTreeMap;

/// Trimmed description prefixes for the available `functions.*` tools.
#[derive(Default)]
pub struct FunctionsNamespaceFunctionPrefixes(BTreeMap<String, String>);

impl FunctionsNamespaceFunctionPrefixes {
    pub fn new(
        prefixes: Option<&ToolDescriptionPrefixes>,
        available_tools: impl IntoIterator<Item = ToolName>,
    ) -> Self {
        let Some(prefixes) = prefixes.filter(|prefixes| !prefixes.is_empty()) else {
            return Self::default();
        };
        let mut normalized = BTreeMap::new();
        for tool in available_tools {
            if !tool.is_default_namespace() || normalized.contains_key(&tool.name) {
                continue;
            }
            let Some(prefix) = prefixes.get(tool.name.as_str()) else {
                continue;
            };
            let prefix = prefix.trim();
            if prefix.is_empty() {
                continue;
            }
            normalized.insert(tool.name, prefix.to_owned());
        }
        Self(normalized)
    }

    /// Returns an owned replacement only for a configured function. Unaffected tools
    /// keep their original specs and derived caches, including large MCP schemas.
    pub fn prepare(&self, name: &ToolName, spec: impl FnOnce() -> ToolSpec) -> Option<ToolSpec> {
        if !name.is_default_namespace() || !self.0.contains_key(&name.name) {
            return None;
        }
        let mut spec = spec();
        match &mut spec {
            ToolSpec::Function(tool) => self.prepend(&tool.name, &mut tool.description),
            ToolSpec::Freeform(tool) => self.prepend(&tool.name, &mut tool.description),
            ToolSpec::Namespace(namespace) if namespace.name == "functions" => {
                for tool in &mut namespace.tools {
                    match tool {
                        ResponsesApiNamespaceTool::Function(tool) => {
                            self.prepend(&tool.name, &mut tool.description);
                        }
                        ResponsesApiNamespaceTool::Custom(tool) => {
                            self.prepend(&tool.name, &mut tool.description);
                        }
                    }
                }
            }
            ToolSpec::Namespace(_) | ToolSpec::ToolSearch { .. } | ToolSpec::WebSearch { .. } => {
                return None;
            }
        }
        Some(spec)
    }

    fn prepend(&self, name: &str, description: &mut String) {
        if let Some(prefix) = self.0.get(name) {
            prepend(prefix, description);
        }
    }
}
