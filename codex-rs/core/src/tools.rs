use std::collections::BTreeMap;
use std::collections::HashMap;

use mcp_types::Tool;
use serde::Serialize;

use crate::model_family;
use crate::openai_tools::JsonSchema;
use crate::openai_tools::OpenAiTool;
use crate::openai_tools::ResponsesApiTool;
use crate::plan_tool::PLAN_TOOL;

#[derive(Debug, Clone, Serialize)]
pub(crate) enum CodexTool {
    McpTool {
        fully_qualified_name: String,
        tool: Box<Tool>,
    },
    OpenAiTool(OpenAiTool),
}

#[derive(Debug, Clone)]
pub enum ShellToolType {
    DefaultShell,
    LocalShell,
}

#[derive(Debug, Clone)]
pub struct ToolsConfig {
    pub shell_type: ShellToolType,
    pub plan_tool: bool,
}

impl ToolsConfig {
    pub fn new(model_family: &model_family::ModelFamily, include_plan_tool: bool) -> Self {
        let shell_type = if model_family.uses_local_shell_tool {
            ShellToolType::LocalShell
        } else {
            ShellToolType::DefaultShell
        };

        Self {
            shell_type,
            plan_tool: include_plan_tool,
        }
    }
}

/// Returns a list of CodexTools based on the provided config and MCP tools.
/// Note that the keys of mcp_tools should be fully qualified names. See
/// [`McpConnectionManager`] for more details.
pub(crate) fn get_codex_tools(
    config: &ToolsConfig,
    mcp_tools: Option<HashMap<String, Tool>>,
) -> Vec<CodexTool> {
    let mut tools: Vec<CodexTool> = Vec::new();

    match config.shell_type {
        ShellToolType::DefaultShell => {
            tools.push(CodexTool::OpenAiTool(create_shell_tool()));
        }
        ShellToolType::LocalShell => {
            tools.push(CodexTool::OpenAiTool(OpenAiTool::LocalShell {}));
        }
    }

    if config.plan_tool {
        tools.push(CodexTool::OpenAiTool(PLAN_TOOL.clone()));
    }

    if let Some(mcp_tools) = mcp_tools {
        tools.extend(
            mcp_tools
                .into_iter()
                .map(|(name, tool)| CodexTool::McpTool {
                    fully_qualified_name: name,
                    tool: Box::new(tool),
                }),
        );
    }

    tools
}

pub(crate) fn create_shell_tool() -> OpenAiTool {
    let mut properties = BTreeMap::new();
    properties.insert(
        "command".to_string(),
        JsonSchema::Array {
            items: Box::new(JsonSchema::String),
        },
    );
    properties.insert("workdir".to_string(), JsonSchema::String);
    properties.insert("timeout".to_string(), JsonSchema::Number);

    OpenAiTool::Function(ResponsesApiTool {
        name: "shell",
        description: "Runs a shell command and returns its output",
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: &["command"],
            additional_properties: false,
        },
    })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use mcp_types::ToolInputSchema;

    use super::*;

    #[test]
    fn test_get_codex_tools() {
        let model_family = model_family::find_family_for_model("codex-mini-latest")
            .expect("codex-mini-latest should be a valid model family");
        let config = ToolsConfig::new(&model_family, true);
        let tools = get_codex_tools(&config, Some(HashMap::new()));

        assert_eq!(tools.len(), 2);
        assert!(matches!(
            tools[0],
            CodexTool::OpenAiTool(OpenAiTool::LocalShell {})
        ));
        assert!(matches!(
            tools[1],
            CodexTool::OpenAiTool(OpenAiTool::Function(ResponsesApiTool {
                name: "update_plan",
                ..
            }))
        ));
    }

    #[test]
    fn test_get_codex_tools_default_shell() {
        let model_family =
            model_family::find_family_for_model("o3").expect("o3 should be a valid model family");
        let config = ToolsConfig::new(&model_family, true);
        let tools = get_codex_tools(&config, Some(HashMap::new()));

        assert_eq!(tools.len(), 2);
        assert!(matches!(
            tools[0],
            CodexTool::OpenAiTool(OpenAiTool::Function(ResponsesApiTool { name: "shell", .. }))
        ));
        assert!(matches!(
            tools[1],
            CodexTool::OpenAiTool(OpenAiTool::Function(ResponsesApiTool {
                name: "update_plan",
                ..
            }))
        ));
    }

    #[test]
    fn test_get_codex_tools_mcp_tools() {
        let model_family =
            model_family::find_family_for_model("o3").expect("o3 should be a valid model family");
        let config = ToolsConfig::new(&model_family, false);
        let tools = get_codex_tools(
            &config,
            Some(HashMap::from([(
                "test_server/do_something_cool".to_string(),
                Tool {
                    name: "do_something_cool".to_string(),
                    input_schema: ToolInputSchema {
                        properties: Some(serde_json::json!({})),
                        required: None,
                        r#type: "object".to_string(),
                    },
                    output_schema: None,
                    title: None,
                    annotations: None,
                    description: None,
                },
            )])),
        );

        assert_eq!(tools.len(), 2);
        assert!(matches!(
            tools[0],
            CodexTool::OpenAiTool(OpenAiTool::Function(ResponsesApiTool { name: "shell", .. }))
        ));
        assert!(matches!(
            tools[1],
            CodexTool::McpTool { ref fully_qualified_name, .. }
            if fully_qualified_name == "test_server/do_something_cool"
        ));
    }
}
