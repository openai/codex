use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::unified_exec::ExecCommandArgs;
use codex_protocol::models::ShellCommandToolCallParams;
use codex_protocol::models::ShellToolCallParams;
use codex_protocol::parse_command::ParsedCommand;
use codex_shell_command::is_safe_command::is_known_safe_command;
use codex_shell_command::parse_command::parse_command;
use std::path::PathBuf;

const MEMORIES_USAGE_METRIC: &str = "codex.memories.usage";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum MemoriesUsageKind {
    MemoryMd,
    MemorySummary,
    RawMemories,
    RolloutSummaries,
    Skills,
}

impl MemoriesUsageKind {
    fn as_tag(self) -> &'static str {
        match self {
            Self::MemoryMd => "memory_md",
            Self::MemorySummary => "memory_summary",
            Self::RawMemories => "raw_memories",
            Self::RolloutSummaries => "rollout_summaries",
            Self::Skills => "skills",
        }
    }
}

pub(crate) async fn emit_metric_for_tool_read(invocation: &ToolInvocation, success: bool) {
    let kinds = memories_usage_kinds_from_invocation(invocation).await;
    if kinds.is_empty() {
        return;
    }

    let success = if success { "true" } else { "false" };
    let tool_name = invocation.tool_name.display();
    for kind in kinds {
        invocation.turn.session_telemetry.counter(
            MEMORIES_USAGE_METRIC,
            /*inc*/ 1,
            &[
                ("kind", kind.as_tag()),
                ("tool", &tool_name),
                ("success", success),
            ],
        );
    }
}

async fn memories_usage_kinds_from_invocation(
    invocation: &ToolInvocation,
) -> Vec<MemoriesUsageKind> {
    let Some((command, _)) = shell_command_for_invocation(invocation) else {
        return Vec::new();
    };
    if !is_known_safe_command(&command) {
        return Vec::new();
    }

    let parsed_commands = parse_command(&command);
    parsed_commands
        .into_iter()
        .filter_map(|command| match command {
            ParsedCommand::Read { path, .. } => get_memory_kind(path.display().to_string()),
            ParsedCommand::Search { path, .. } => path.and_then(get_memory_kind),
            ParsedCommand::ListFiles { .. } | ParsedCommand::Unknown { .. } => None,
        })
        .collect()
}

fn shell_command_for_invocation(invocation: &ToolInvocation) -> Option<(Vec<String>, PathBuf)> {
    let ToolPayload::Function { arguments } = &invocation.payload else {
        return None;
    };

    match (
        invocation.tool_name.namespace.as_deref(),
        invocation.tool_name.name.as_str(),
    ) {
        (None, "shell") => serde_json::from_str::<ShellToolCallParams>(arguments)
            .ok()
            .and_then(|params| {
                let workdir = resolve_tool_workdir(
                    invocation,
                    params.environment_id.as_deref(),
                    params.workdir,
                )?;
                Some((params.command, workdir))
            }),
        (None, "shell_command") => serde_json::from_str::<ShellCommandToolCallParams>(arguments)
            .ok()
            .and_then(|params| {
                let workdir = resolve_tool_workdir(
                    invocation,
                    params.environment_id.as_deref(),
                    params.workdir,
                )?;
                if !invocation.turn.tools_config.allow_login_shell && params.login == Some(true) {
                    return Some((Vec::new(), workdir));
                }
                let use_login_shell = params
                    .login
                    .unwrap_or(invocation.turn.tools_config.allow_login_shell);
                let command = invocation
                    .session
                    .user_shell()
                    .derive_exec_args(&params.command, use_login_shell);
                Some((command, workdir))
            }),
        (None, "exec_command") => serde_json::from_str::<ExecCommandArgs>(arguments)
            .ok()
            .and_then(|params| {
                let environment_id = tool_environment_id(arguments);
                let workdir = resolve_tool_workdir(
                    invocation,
                    environment_id.as_deref(),
                    params.workdir.clone(),
                )?;
                let command = crate::tools::handlers::unified_exec::get_command(
                    &params,
                    invocation.session.user_shell(),
                    &invocation.turn.tools_config.unified_exec_shell_mode,
                    invocation.turn.tools_config.allow_login_shell,
                )
                .ok()?;
                Some((command, workdir))
            }),
        (Some(_), _) | (None, _) => None,
    }
}

fn tool_environment_id(arguments: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(arguments).ok()?;
    value
        .get("environment_id")
        .or_else(|| value.get("environmentId"))
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
}

fn resolve_tool_workdir(
    invocation: &ToolInvocation,
    environment_id: Option<&str>,
    workdir: Option<String>,
) -> Option<PathBuf> {
    let requested_environment_id = invocation
        .turn
        .tools_config
        .multi_environment_tools
        .then_some(environment_id)
        .flatten();
    let environment = invocation
        .turn
        .environment_for_tool(requested_environment_id)?;
    Some(
        invocation
            .turn
            .resolve_path_for_environment(&environment, workdir)
            .to_path_buf(),
    )
}

fn get_memory_kind(path: String) -> Option<MemoriesUsageKind> {
    if path.contains("memories/MEMORY.md") {
        Some(MemoriesUsageKind::MemoryMd)
    } else if path.contains("memories/memory_summary.md") {
        Some(MemoriesUsageKind::MemorySummary)
    } else if path.contains("memories/raw_memories.md") {
        Some(MemoriesUsageKind::RawMemories)
    } else if path.contains("memories/rollout_summaries/") {
        Some(MemoriesUsageKind::RolloutSummaries)
    } else if path.contains("memories/skills/") {
        Some(MemoriesUsageKind::Skills)
    } else {
        None
    }
}
