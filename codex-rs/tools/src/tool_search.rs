use crate::JsonSchema;
use crate::LoadableToolSpec;
use crate::ResponsesApiNamespaceTool;
use crate::ResponsesApiTool;
use crate::ToolSearchSourceInfo;
use crate::ToolSpec;
use crate::default_namespace_description;

#[derive(Clone, PartialEq)]
pub struct ToolSearchEntry {
    pub search_text: String,
    pub identity: ToolSearchIdentity,
    pub output: LoadableToolSpec,
}

#[derive(Clone, PartialEq)]
pub struct ToolSearchInfo {
    pub entry: ToolSearchEntry,
    pub source_info: Option<ToolSearchSourceInfo>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ToolSearchIdentity {
    pub canonical_aliases: Vec<String>,
    pub tool_aliases: Vec<String>,
    pub source_aliases: Vec<String>,
}

impl ToolSearchInfo {
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
        Self::from_spec_with_identity(
            search_text,
            spec,
            source_info,
            ToolSearchIdentity::default(),
        )
    }

    pub fn from_spec_with_identity(
        search_text: String,
        spec: ToolSpec,
        source_info: Option<ToolSearchSourceInfo>,
        identity: ToolSearchIdentity,
    ) -> Option<Self> {
        let output = match spec {
            ToolSpec::Function(mut tool) => {
                tool.defer_loading = Some(true);
                tool.output_schema = None;
                LoadableToolSpec::Function(tool)
            }
            ToolSpec::Namespace(mut namespace) => {
                if namespace.description.trim().is_empty() {
                    namespace.description = default_namespace_description(&namespace.name);
                }
                for tool in &mut namespace.tools {
                    let ResponsesApiNamespaceTool::Function(tool) = tool;
                    tool.defer_loading = Some(true);
                    tool.output_schema = None;
                }
                LoadableToolSpec::Namespace(namespace)
            }
            ToolSpec::ToolSearch { .. }
            | ToolSpec::ImageGeneration { .. }
            | ToolSpec::WebSearch { .. }
            | ToolSpec::Freeform(_) => return None,
        };
        let identity = build_identity(&output, source_info.as_ref(), identity);

        Some(Self {
            entry: ToolSearchEntry {
                search_text,
                identity,
                output,
            },
            source_info,
        })
    }
}

fn default_tool_search_text(spec: &ToolSpec) -> String {
    let mut parts = Vec::new();

    match spec {
        ToolSpec::Function(tool) => append_function_search_text(tool, &mut parts),
        ToolSpec::Namespace(namespace) => {
            push_search_part(&mut parts, namespace.description.clone());
            for tool in &namespace.tools {
                let ResponsesApiNamespaceTool::Function(tool) = tool;
                append_function_search_text(tool, &mut parts);
            }
        }
        ToolSpec::ToolSearch { description, .. } => {
            push_search_part(&mut parts, description.clone());
        }
        ToolSpec::ImageGeneration { .. } => {
            push_search_part(&mut parts, "image generation".to_string());
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
    push_search_part(parts, tool.description.clone());
    append_schema_search_text(&tool.parameters, parts);
}

fn append_schema_search_text(schema: &JsonSchema, parts: &mut Vec<String>) {
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

fn build_identity(
    output: &LoadableToolSpec,
    source_info: Option<&ToolSearchSourceInfo>,
    mut identity: ToolSearchIdentity,
) -> ToolSearchIdentity {
    append_output_identity(output, &mut identity);
    if let Some(source_info) = source_info {
        push_unique_alias(&mut identity.source_aliases, &source_info.name);
    }
    identity
}

fn append_output_identity(output: &LoadableToolSpec, identity: &mut ToolSearchIdentity) {
    match output {
        LoadableToolSpec::Function(tool) => {
            push_unique_alias(&mut identity.canonical_aliases, &tool.name);
            push_unique_alias(&mut identity.tool_aliases, &tool.name);
        }
        LoadableToolSpec::Namespace(namespace) => {
            push_unique_alias(&mut identity.source_aliases, &namespace.name);
            for tool in &namespace.tools {
                let ResponsesApiNamespaceTool::Function(tool) = tool;
                push_namespaced_aliases(
                    &mut identity.canonical_aliases,
                    &namespace.name,
                    &tool.name,
                );
                push_unique_alias(&mut identity.tool_aliases, &tool.name);
            }
        }
    }
}

fn push_namespaced_aliases(aliases: &mut Vec<String>, namespace: &str, name: &str) {
    push_unique_alias(aliases, &format!("{namespace}.{name}"));
    push_unique_alias(aliases, &format!("{namespace}__{name}"));
    push_unique_alias(aliases, &format!("{namespace}{name}"));
}

fn push_unique_alias(aliases: &mut Vec<String>, alias: &str) {
    let alias = alias.trim();
    if !alias.is_empty() && !aliases.iter().any(|existing| existing == alias) {
        aliases.push(alias.to_string());
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
