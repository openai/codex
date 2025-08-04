use mcp_types::Tool;
use serde::Serialize;

use crate::openai_tools::DEFAULT_SHELL_TOOL;
use crate::openai_tools::OpenAiTool;
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
pub struct ToolFlags {
    pub local_shell: bool,
    pub plan_tool: bool,
    pub shell: bool,
}

impl ToolFlags {
    pub fn default(model: &str, include_plan_tool: bool) -> Self {
        let local_shell = model.starts_with("codex");

        Self {
            local_shell,
            plan_tool: include_plan_tool,
            shell: !local_shell,
        }
    }
}

pub(crate) fn get_codex_tools(flags: ToolFlags, extra_tools: Vec<CodexTool>) -> Vec<CodexTool> {
    let mut tools: Vec<CodexTool> = Vec::new();

    if !flags.local_shell && !flags.shell {
        tracing::warn!("No shell tools enabled");
    } else if flags.local_shell && flags.shell {
        tracing::warn!("Multiple shell tools enabled");
    }

    if flags.local_shell {
        tools.push(CodexTool::OpenAiTool(OpenAiTool::LocalShell {}));
    }

    if flags.plan_tool {
        tools.push(CodexTool::OpenAiTool(PLAN_TOOL.clone()));
    }

    if flags.shell {
        tools.push(CodexTool::OpenAiTool(DEFAULT_SHELL_TOOL.clone()));
    }

    tools.extend(extra_tools);

    tools
}
