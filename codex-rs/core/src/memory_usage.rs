use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::flat_tool_name;
use crate::tools::handlers::unified_exec::ExecCommandArgs;
use codex_memories_read::usage::MEMORIES_USAGE_METRIC;
use codex_memories_read::usage::memories_usage_kinds_from_command;
use codex_protocol::models::ShellCommandToolCallParams;
use codex_shell_command::bash::parse_plain_shell_script;
use codex_shell_command::is_safe_command::is_known_safe_command;

pub(crate) fn emit_metric_for_tool_read(invocation: &ToolInvocation, success: bool) {
    let Some(commands) = shell_commands_for_invocation(invocation) else {
        return;
    };
    if !commands
        .iter()
        .all(|command| is_known_safe_command(command))
    {
        return;
    }

    let success = if success { "true" } else { "false" };
    let tool_name = flat_tool_name(&invocation.tool_name);
    for command in commands {
        for kind in memories_usage_kinds_from_command(&command) {
            invocation.turn.session_telemetry.counter(
                MEMORIES_USAGE_METRIC,
                /*inc*/ 1,
                &[
                    ("kind", kind.as_tag()),
                    ("tool", tool_name.as_ref()),
                    ("success", success),
                ],
            );
        }
    }
}

fn shell_commands_for_invocation(invocation: &ToolInvocation) -> Option<Vec<Vec<String>>> {
    let ToolPayload::Function { arguments } = &invocation.payload else {
        return None;
    };

    let command = match (
        invocation.tool_name.namespace.as_deref(),
        invocation.tool_name.name.as_str(),
    ) {
        (None, "shell_command") => serde_json::from_str::<ShellCommandToolCallParams>(arguments)
            .ok()
            .map(|params| params.command),
        (None, "exec_command") => serde_json::from_str::<ExecCommandArgs>(arguments)
            .ok()
            .map(|params| params.cmd),
        (Some(_), _) | (None, _) => None,
    }?;

    parse_plain_shell_script(&command)
}
