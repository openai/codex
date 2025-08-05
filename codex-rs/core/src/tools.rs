use std::collections::{BTreeMap, HashMap};

use mcp_types::Tool;
use serde::Serialize;

use crate::model_family;
use crate::openai_tools::{JsonSchema, OpenAiTool, ResponsesApiTool};
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
    pub fn build(model_family: &model_family::ModelFamily, include_plan_tool: bool) -> Self {
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
    config: ToolsConfig,
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
