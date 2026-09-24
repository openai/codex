use crate::JsonSchema;
use crate::LoadableToolSpec;
use crate::ResponsesApiNamespace;
use crate::ResponsesApiNamespaceTool;
use crate::ResponsesApiTool;
use crate::ToolSearchSourceInfo;
use crate::ToolSpec;
use crate::default_namespace_description;
use codex_protocol::DEFAULT_FUNCTION_NAMESPACE;
use std::sync::Arc;

#[derive(Clone, PartialEq)]
pub struct ToolSearchEntry {
    pub search_text: String,
    spec: Arc<ToolSpec>,
}

impl ToolSearchEntry {
    /// Materialize only selected results; output schemas remain shared until discarded.
    pub fn to_loadable_spec(&self) -> LoadableToolSpec {
        let Some(output) = normalize_search_spec(self.spec.as_ref().clone()) else {
            unreachable!("search entries contain only loadable tools");
        };
        output
    }
}

#[derive(Clone, PartialEq)]
pub struct ToolSearchInfo {
    pub entry: ToolSearchEntry,
    pub source_info: Option<ToolSearchSourceInfo>,
}

impl ToolSearchInfo {
    /// Keep the immutable catalog snapshot alive without duplicating its schemas.
    pub fn from_shared_spec(
        search_text: String,
        spec: Arc<ToolSpec>,
        source_info: Option<ToolSearchSourceInfo>,
    ) -> Option<Self> {
        match spec.as_ref() {
            ToolSpec::Function(_) | ToolSpec::Freeform(_) | ToolSpec::Namespace(_) => Some(Self {
                entry: ToolSearchEntry { search_text, spec },
                source_info,
            }),
            ToolSpec::ToolSearch { .. } | ToolSpec::WebSearch { .. } => None,
        }
    }

    pub fn from_tool_spec(
        spec: ToolSpec,
        source_info: Option<ToolSearchSourceInfo>,
    ) -> Option<Self> {
        let search_text = default_tool_search_text(&spec);
        Self::from_spec(search_text, spec, source_info)
    }

    pub fn from_spec(
        search_text: String,
        spec: ToolSpec,
        source_info: Option<ToolSearchSourceInfo>,
    ) -> Option<Self> {
        // Dynamic-tool cache entries compare normalized specs by value.
        // Preserve that behavior; shared MCP specs normalize only when selected.
        let output = normalize_search_spec(spec)?;
        Some(Self {
            entry: ToolSearchEntry {
                search_text,
                spec: Arc::new(output.into()),
            },
            source_info,
        })
    }
}

fn normalize_search_spec(spec: ToolSpec) -> Option<LoadableToolSpec> {
    let mut namespace = match spec {
        ToolSpec::Function(tool) => ResponsesApiNamespace {
            name: DEFAULT_FUNCTION_NAMESPACE.to_string(),
            description: default_namespace_description(DEFAULT_FUNCTION_NAMESPACE),
            tools: vec![ResponsesApiNamespaceTool::Function(tool)],
        },
        ToolSpec::Freeform(tool) => ResponsesApiNamespace {
            name: DEFAULT_FUNCTION_NAMESPACE.to_string(),
            description: default_namespace_description(DEFAULT_FUNCTION_NAMESPACE),
            tools: vec![ResponsesApiNamespaceTool::Custom(tool)],
        },
        ToolSpec::Namespace(namespace) => namespace,
        ToolSpec::ToolSearch { .. } | ToolSpec::WebSearch { .. } => {
            return None;
        }
    };

    if namespace.description.trim().is_empty() {
        namespace.description = default_namespace_description(&namespace.name);
    }
    for tool in &mut namespace.tools {
        match tool {
            ResponsesApiNamespaceTool::Function(tool) => {
                tool.defer_loading = Some(true);
                tool.output_schema = None;
                tool.parameters.mcp_input_schema_max_bytes = None;
            }
            ResponsesApiNamespaceTool::Custom(tool) => {
                tool.defer_loading = Some(true);
            }
        }
    }
    Some(LoadableToolSpec::Namespace(namespace))
}

fn default_tool_search_text(spec: &ToolSpec) -> String {
    let mut parts = Vec::new();

    match spec {
        ToolSpec::Function(tool) => append_function_search_text(tool, &mut parts),
        ToolSpec::Namespace(namespace) => {
            push_search_part(&mut parts, namespace.name.clone());
            push_search_part(&mut parts, namespace.description.clone());
            for tool in &namespace.tools {
                match tool {
                    ResponsesApiNamespaceTool::Function(tool) => {
                        append_function_search_text(tool, &mut parts);
                    }
                    ResponsesApiNamespaceTool::Custom(tool) => {
                        push_search_part(&mut parts, tool.name.clone());
                        push_search_part(&mut parts, tool.description.clone());
                        push_search_part(&mut parts, tool.format.syntax.clone());
                    }
                }
            }
        }
        ToolSpec::ToolSearch { description, .. } => {
            push_search_part(&mut parts, description.clone());
        }
        ToolSpec::WebSearch { .. } => {
            push_search_part(&mut parts, "web search".to_string());
        }
        ToolSpec::Freeform(tool) => {
            push_search_part(&mut parts, tool.name.clone());
            push_search_part(&mut parts, tool.description.clone());
            push_search_part(&mut parts, tool.format.syntax.clone());
        }
    }

    parts.join(" ")
}

fn append_function_search_text(tool: &ResponsesApiTool, parts: &mut Vec<String>) {
    push_search_part(parts, tool.name.clone());
    push_search_part(parts, tool.name.replace('_', " "));
    push_search_part(parts, tool.description.clone());
    append_schema_search_text(&tool.parameters, parts);
}

fn append_schema_search_text(schema: &JsonSchema, parts: &mut Vec<String>) {
    if let Some(description) = &schema.description {
        push_search_part(parts, description.clone());
    }
    if let Some(properties) = &schema.properties {
        for (name, schema) in properties {
            push_search_part(parts, name.clone());
            append_schema_search_text(schema, parts);
        }
    }
    if let Some(items) = &schema.items {
        append_schema_search_text(items, parts);
    }
    if let Some(variants) = &schema.any_of {
        for variant in variants {
            append_schema_search_text(variant, parts);
        }
    }
}

fn push_search_part(parts: &mut Vec<String>, part: String) {
    let part = part.trim();
    if !part.is_empty() {
        parts.push(part.to_string());
    }
}

#[cfg(test)]
#[path = "tool_search_tests.rs"]
mod tests;
