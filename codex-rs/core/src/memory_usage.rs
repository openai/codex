use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::unified_exec::ExecCommandArgs;
use codex_memories_read::usage::MEMORIES_USAGE_METRIC;
use codex_memories_read::usage::memories_usage_kinds_from_command;
use codex_protocol::models::ShellCommandToolCallParams;
use codex_protocol::models::ShellToolCallParams;
use std::path::PathBuf;

pub(crate) async fn emit_metric_for_tool_read(invocation: &ToolInvocation, success: bool) {
    let Some((command, _)) = shell_command_for_invocation(invocation) else {
        return;
    };
    let kinds = memories_usage_kinds_from_command(&command);
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
            .map(|params| {
                (
                    params.command,
                    invocation.turn.resolve_path(params.workdir).to_path_buf(),
                )
            }),
        (None, "shell_command") => serde_json::from_str::<ShellCommandToolCallParams>(arguments)
            .ok()
            .map(|params| {
                if !invocation.turn.tools_config.allow_login_shell && params.login == Some(true) {
                    return (
                        Vec::new(),
                        invocation.turn.resolve_path(params.workdir).to_path_buf(),
                    );
                }
                let use_login_shell = params
                    .login
                    .unwrap_or(invocation.turn.tools_config.allow_login_shell);
                let command = invocation
                    .session
                    .user_shell()
                    .derive_exec_args(&params.command, use_login_shell);
                (
                    command,
                    invocation.turn.resolve_path(params.workdir).to_path_buf(),
                )
            }),
        (None, "exec_command") => serde_json::from_str::<ExecCommandArgs>(arguments)
            .ok()
            .and_then(|params| {
                let command = crate::tools::handlers::unified_exec::get_command(
                    &params,
                    invocation.session.user_shell(),
                    &invocation.turn.tools_config.unified_exec_shell_mode,
                    invocation.turn.tools_config.allow_login_shell,
                )
                .ok()?;
                Some((
                    command,
                    invocation.turn.resolve_path(params.workdir).to_path_buf(),
                ))
            }),
        (Some(_), _) | (None, _) => None,
    }
}
