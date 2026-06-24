use codex_protocol::models::ResponseItem;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolSpec;

/// Function names that uniquely identify the complete MultiAgentV2 tool family.
const MULTI_AGENT_V2_FUNCTION_NAMES: [&str; 6] = [
    "spawn_agent",
    "send_message",
    "followup_task",
    "wait_agent",
    "interrupt_agent",
    "list_agents",
];

/// Repairs function calls persisted before MultiAgentV2 tools moved into a namespace.
///
/// The prompt's typed tool specs are the source of truth. The repair deliberately fails closed
/// unless one namespace exposes the complete MultiAgentV2 family and no flat function or second
/// namespace makes the legacy call ambiguous.
pub(crate) fn repair_legacy_multi_agent_v2_function_call_namespaces(
    input: &mut [ResponseItem],
    tools: &[ToolSpec],
) {
    for item in input {
        let ResponseItem::FunctionCall {
            name, namespace, ..
        } = item
        else {
            continue;
        };
        if namespace.is_some()
            || !MULTI_AGENT_V2_FUNCTION_NAMES.contains(&name.as_str())
            || has_flat_function(tools, name)
        {
            continue;
        }

        let mut matching_namespaces = tools
            .iter()
            .filter_map(|tool| namespace_containing_function(tool, name));
        let Some(candidate_namespace) = matching_namespaces.next() else {
            continue;
        };
        if matching_namespaces.any(|namespace| namespace != candidate_namespace)
            || !has_multi_agent_v2_fingerprint(tools, candidate_namespace)
        {
            continue;
        }

        *namespace = Some(candidate_namespace.to_string());
    }
}

fn has_flat_function(tools: &[ToolSpec], function_name: &str) -> bool {
    tools
        .iter()
        .any(|tool| matches!(tool, ToolSpec::Function(function) if function.name == function_name))
}

fn namespace_containing_function<'a>(tool: &'a ToolSpec, function_name: &str) -> Option<&'a str> {
    let ToolSpec::Namespace(namespace) = tool else {
        return None;
    };
    namespace
        .tools
        .iter()
        .any(|tool| {
            matches!(tool, ResponsesApiNamespaceTool::Function(function) if function.name == function_name)
        })
        .then_some(namespace.name.as_str())
}

fn has_multi_agent_v2_fingerprint(tools: &[ToolSpec], namespace_name: &str) -> bool {
    !namespace_name.is_empty()
        && MULTI_AGENT_V2_FUNCTION_NAMES
            .iter()
            .all(|function_name| {
                tools
                    .iter()
                    .filter_map(|tool| {
                        let ToolSpec::Namespace(namespace) = tool else {
                            return None;
                        };
                        (namespace.name == namespace_name).then_some(&namespace.tools)
                    })
                    .flatten()
                    .filter(|tool| {
                        matches!(tool, ResponsesApiNamespaceTool::Function(function) if function.name == *function_name)
                    })
                    .count()
                    == 1
            })
}
