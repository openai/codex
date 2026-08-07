use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::responses_metadata::TurnToolFunctionInfo;
use crate::responses_metadata::TurnToolNamespaceInfo;
use crate::responses_metadata::TurnToolNamespacesInfo;
use crate::responses_metadata::TurnToolSource;
use crate::tools::registry::ToolExposure;
use crate::tools::registry::ToolRegistry;
use codex_protocol::DEFAULT_FUNCTION_NAMESPACE;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::TOOL_SEARCH_TOOL_NAME;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

const TOOL_SEARCH_FUNCTION_NAME: &str = "tool_search_tool";

pub(super) fn collect_tool_namespaces_info(
    registry: &ToolRegistry,
    code_mode_tool_names: &BTreeMap<String, ToolName>,
    model_visible_specs: &[ToolSpec],
) -> TurnToolNamespacesInfo {
    let mut direct_functions = BTreeSet::new();
    let mut native_tool_search_visible = false;
    for spec in model_visible_specs {
        match spec {
            ToolSpec::Function(_) | ToolSpec::Freeform(_) => {
                direct_functions.insert((DEFAULT_FUNCTION_NAMESPACE, spec.name()));
            }
            ToolSpec::Namespace(namespace) => {
                for tool in &namespace.tools {
                    let function_name = match tool {
                        ResponsesApiNamespaceTool::Function(tool) => tool.name.as_str(),
                        ResponsesApiNamespaceTool::Custom(tool) => tool.name.as_str(),
                    };
                    direct_functions.insert((namespace.name.as_str(), function_name));
                }
            }
            ToolSpec::ToolSearch { .. } => {
                native_tool_search_visible = true;
                direct_functions.insert((TOOL_SEARCH_TOOL_NAME, TOOL_SEARCH_FUNCTION_NAME));
            }
            ToolSpec::WebSearch { .. } => {}
        }
    }

    let code_mode_names_by_tool = code_mode_tool_names
        .iter()
        .map(|(code_mode_name, tool_name)| {
            (tool_name.clone().with_default_namespace(), code_mode_name)
        })
        .collect::<BTreeMap<_, _>>();
    let mut namespaces = TurnToolNamespacesInfo::default();

    for tool in registry.entries() {
        let runtime = &tool.runtime;
        let exposure = tool.exposure;
        if exposure == ToolExposure::Hidden {
            continue;
        }

        let tool_name = runtime.tool_name().with_default_namespace();
        let is_tool_search = tool_name.name == TOOL_SEARCH_TOOL_NAME
            && matches!(runtime.spec(), ToolSpec::ToolSearch { .. });
        let (namespace_name, function_name) = if is_tool_search {
            (TOOL_SEARCH_TOOL_NAME, TOOL_SEARCH_FUNCTION_NAME)
        } else {
            (
                tool_name
                    .namespace
                    .as_deref()
                    .unwrap_or(DEFAULT_FUNCTION_NAMESPACE),
                tool_name.name.as_str(),
            )
        };
        let direct = direct_functions.contains(&(namespace_name, function_name));
        let code_mode_name = code_mode_names_by_tool
            .get(&tool_name)
            .map(|name| (*name).clone());
        let deferred = exposure.is_deferred()
            && native_tool_search_visible
            && (runtime.immutable_spec().is_some() || runtime.search_info().is_some());
        if !direct && !deferred && code_mode_name.is_none() {
            continue;
        }

        let namespace = namespaces
            .entry(namespace_name.to_string())
            .or_insert_with(|| TurnToolNamespaceInfo {
                name: namespace_name.to_string(),
                functions: BTreeMap::new(),
            });
        namespace
            .functions
            .entry(function_name.to_string())
            .or_insert_with(|| TurnToolFunctionInfo {
                name: function_name.to_string(),
                direct,
                code_mode_name,
                deferred,
                source: match runtime.mcp_server_name() {
                    Some(server_name) => TurnToolSource::Mcp {
                        server_name: server_name.to_string(),
                    },
                    None => TurnToolSource::Harness,
                },
            });
    }

    namespaces
}
