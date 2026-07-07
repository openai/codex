use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::models::ResponseItem;

use crate::context::ContextualUserFragment;
use crate::context::UserShellCommand;
use crate::session::step_model_context::StepModelContext;
use crate::tools::format_exec_output_str;

fn user_shell_command_fragment(
    command: &str,
    exec_output: &ExecToolCallOutput,
    model: &StepModelContext,
) -> UserShellCommand {
    let output = format_exec_output_str(exec_output, model.model_info.truncation_policy.into());
    UserShellCommand::new(command, exec_output.exit_code, exec_output.duration, output)
}

#[cfg(test)]
pub fn format_user_shell_command_record(
    command: &str,
    exec_output: &ExecToolCallOutput,
    model: &StepModelContext,
) -> String {
    user_shell_command_fragment(command, exec_output, model).render()
}

pub fn user_shell_command_record_item(
    command: &str,
    exec_output: &ExecToolCallOutput,
    model: &StepModelContext,
) -> ResponseItem {
    ContextualUserFragment::into(user_shell_command_fragment(command, exec_output, model))
}

#[cfg(test)]
#[path = "user_shell_command_tests.rs"]
mod tests;
