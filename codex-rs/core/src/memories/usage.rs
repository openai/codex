use crate::is_safe_command::is_known_safe_command;
use crate::parse_command::parse_command;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::unified_exec::ExecCommandArgs;
use codex_protocol::models::ShellCommandToolCallParams;
use codex_protocol::models::ShellToolCallParams;
use codex_protocol::parse_command::ParsedCommand;
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
    for kind in kinds {
        invocation.turn.otel_manager.counter(
            MEMORIES_USAGE_METRIC,
            1,
            &[
                ("kind", kind.as_tag()),
                ("tool", invocation.tool_name.as_str()),
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

    match invocation.tool_name.as_str() {
        "shell" => serde_json::from_str::<ShellToolCallParams>(arguments)
            .ok()
            .map(|params| (params.command, invocation.turn.resolve_path(params.workdir))),
        "shell_command" => serde_json::from_str::<ShellCommandToolCallParams>(arguments)
            .ok()
            .map(|params| {
                let command = invocation
                    .session
                    .user_shell()
                    .derive_exec_args(&params.command, params.login.unwrap_or(true));
                (command, invocation.turn.resolve_path(params.workdir))
            }),
        "exec_command" => serde_json::from_str::<ExecCommandArgs>(arguments)
            .ok()
            .map(|params| {
                (
                    crate::tools::handlers::unified_exec::get_command(
                        &params,
                        invocation.session.user_shell(),
                    ),
                    invocation.turn.resolve_path(params.workdir),
                )
            }),
        _ => None,
    }
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
