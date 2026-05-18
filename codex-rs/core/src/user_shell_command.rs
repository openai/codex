use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::models::ResponseItem;

use crate::context::ContextualUserFragment;
use crate::context::UserShellCommand;
use crate::session::turn_context::TurnContext;
use crate::tools::format_exec_output_str_with_original_token_count;

fn user_shell_command_fragment(
    command: &str,
    exec_output: &ExecToolCallOutput,
    turn_context: &TurnContext,
) -> UserShellCommand {
    let (output, original_token_count) = format_exec_output_str_with_original_token_count(
        exec_output,
        turn_context.truncation_policy,
    );
    UserShellCommand::new(
        command,
        exec_output.exit_code,
        exec_output.duration,
        output,
        original_token_count,
    )
}

#[cfg(test)]
pub fn format_user_shell_command_record(
    command: &str,
    exec_output: &ExecToolCallOutput,
    turn_context: &TurnContext,
) -> String {
    user_shell_command_fragment(command, exec_output, turn_context).render()
}

pub fn user_shell_command_record_item(
    command: &str,
    exec_output: &ExecToolCallOutput,
    turn_context: &TurnContext,
) -> ResponseItem {
    ContextualUserFragment::into(user_shell_command_fragment(
        command,
        exec_output,
        turn_context,
    ))
}

#[cfg(test)]
#[path = "user_shell_command_tests.rs"]
mod tests;
