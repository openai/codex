use std::collections::BTreeMap;
use std::collections::HashSet;

use codex_protocol::models::ResponseItem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnavailableTool {
    pub(crate) qualified_name: String,
    pub(crate) namespace: Option<String>,
    pub(crate) name: String,
}

pub(crate) fn collect_unavailable_called_tools(
    input: &[ResponseItem],
    exposed_tool_names: &HashSet<&str>,
) -> Vec<UnavailableTool> {
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

        let qualified_name = qualified_tool_name(name, namespace.as_deref());
        if exposed_tool_names.contains(qualified_name.as_str()) {
            continue;
        }

        unavailable_tools
            .entry(qualified_name.clone())
            .or_insert_with(|| UnavailableTool {
                qualified_name,
                namespace: namespace.clone(),
                name: name.clone(),
            });
    }

    unavailable_tools.into_values().collect()
}

fn should_collect_unavailable_tool(name: &str, namespace: Option<&str>) -> bool {
    namespace.is_some_and(|namespace| namespace.starts_with("mcp__")) || name.starts_with("mcp__")
}

fn qualified_tool_name(name: &str, namespace: Option<&str>) -> String {
    match namespace {
        Some(namespace) if name.starts_with(namespace) => name.to_string(),
        Some(namespace) => format!("{namespace}{name}"),
        None => name.to_string(),
    }
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
                UnavailableTool {
                    qualified_name: "mcp__codex_apps__calendar_create_event".to_string(),
                    namespace: Some("mcp__codex_apps__calendar".to_string()),
                    name: "_create_event".to_string(),
                },
                UnavailableTool {
                    qualified_name: "mcp__server__lookup".to_string(),
                    namespace: None,
                    name: "mcp__server__lookup".to_string(),
                },
            ]
        );
    }

    #[test]
    fn collect_unavailable_called_tools_skips_currently_available_tools() {
        let exposed_tool_names = HashSet::from(["mcp__server__lookup", "mcp__server__search"]);
        let input = vec![
            function_call("mcp__server__lookup", /*namespace*/ None),
            function_call("mcp__server__search", /*namespace*/ None),
            function_call("mcp__server__missing", /*namespace*/ None),
        ];

        let tools = collect_unavailable_called_tools(&input, &exposed_tool_names);

        assert_eq!(
            tools,
            vec![UnavailableTool {
                qualified_name: "mcp__server__missing".to_string(),
                namespace: None,
                name: "mcp__server__missing".to_string(),
            }]
        );
    }
}
