use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use std::collections::BTreeMap;

pub const RELOAD_PLUGINS_TOOL_NAME: &str = "reload_plugins";

pub fn create_reload_plugins_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: RELOAD_PLUGINS_TOOL_NAME.to_string(),
        description: "Reload local plugin configuration, clear plugin and skill caches, and rebuild MCP tools from the refreshed plugin state for the current turn. Plugin instructions or skills already injected into an older thread remain unchanged; use a new thread to pick up newly loaded plugin context.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(BTreeMap::new(), Some(Vec::new()), Some(false.into())),
        output_schema: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reload_plugins_tool_has_no_arguments() {
        let ToolSpec::Function(tool) = create_reload_plugins_tool() else {
            panic!("reload_plugins should be a function tool");
        };

        assert_eq!(tool.name, RELOAD_PLUGINS_TOOL_NAME);
        assert_eq!(tool.parameters.required, Some(Vec::new()));
        assert_eq!(tool.parameters.properties, Some(BTreeMap::new()));
    }
}
