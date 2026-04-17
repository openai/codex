use std::collections::BTreeMap;
use std::collections::HashSet;

use codex_protocol::models::ResponseItem;
use codex_tools::ToolName;

pub(crate) fn collect_unavailable_called_tools(
    input: &[ResponseItem],
    exposed_tool_names: &HashSet<String>,
) -> Vec<ToolName> {
    let mut unavailable_tools = BTreeMap::new();

    for item in input {
        let ResponseItem::FunctionCall {
            name, namespace, ..
        } = item
        else {
            continue;
        };
        if !should_collect_unavailable_tool(name, namespace.as_deref()) {
            continue;
        }

        let tool_name = match namespace {
            Some(namespace) => ToolName::namespaced(namespace.clone(), name.clone()),
            None => ToolName::plain(name.clone()),
        };
        let display_name = tool_name.display();
        if exposed_tool_names.contains(display_name.as_str()) {
            continue;
        }

        unavailable_tools
            .entry(display_name)
            .or_insert_with(|| tool_name);
    }

    unavailable_tools.into_values().collect()
}

fn should_collect_unavailable_tool(name: &str, namespace: Option<&str>) -> bool {
    namespace.is_some_and(|namespace| namespace.starts_with("mcp__")) || name.starts_with("mcp__")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn function_call(name: &str, namespace: Option<&str>) -> ResponseItem {
        ResponseItem::FunctionCall {
            id: None,
            name: name.to_string(),
            namespace: namespace.map(str::to_string),
            arguments: "{}".to_string(),
            call_id: format!("call-{name}"),
        }
    }

    #[test]
    fn collect_unavailable_called_tools_detects_mcp_function_calls() {
        let input = vec![
            function_call("shell", /*namespace*/ None),
            function_call("mcp__server__lookup", /*namespace*/ None),
            function_call("_create_event", Some("mcp__codex_apps__calendar")),
        ];

        let tools = collect_unavailable_called_tools(&input, &HashSet::new());

        assert_eq!(
            tools,
            vec![
                ToolName::namespaced("mcp__codex_apps__calendar", "_create_event"),
                ToolName::plain("mcp__server__lookup"),
            ]
        );
    }

    #[test]
    fn collect_unavailable_called_tools_skips_currently_available_tools() {
        let exposed_tool_names = HashSet::from([
            "mcp__server__lookup".to_string(),
            "mcp__server__search".to_string(),
        ]);
        let input = vec![
            function_call("mcp__server__lookup", /*namespace*/ None),
            function_call("mcp__server__search", /*namespace*/ None),
            function_call("mcp__server__missing", /*namespace*/ None),
        ];

        let tools = collect_unavailable_called_tools(&input, &exposed_tool_names);

        assert_eq!(tools, vec![ToolName::plain("mcp__server__missing")]);
    }
}
