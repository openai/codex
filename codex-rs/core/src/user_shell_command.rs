use std::time::Duration;

use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::models::ResponseItem;

use crate::contextual_user_message::USER_SHELL_COMMAND_FRAGMENT;
use crate::session::turn_context::TurnContext;
use crate::tools::format_exec_output_str;

fn format_duration_line(duration: Duration) -> String {
    let duration_seconds = duration.as_secs_f64();
    format!("Duration: {duration_seconds:.4} seconds")
}

fn format_user_shell_command_body(
    command: &str,
    exec_output: &ExecToolCallOutput,
    turn_context: &TurnContext,
) -> String {
    let mut sections = Vec::new();
    sections.push("<command>".to_string());
    sections.push(command.to_string());
    sections.push("</command>".to_string());
    sections.push("<result>".to_string());
    sections.push(format!("Exit code: {}", exec_output.exit_code));
    sections.push(format_duration_line(exec_output.duration));
    sections.push("Output:".to_string());
    sections.push(format_exec_output_str(
        exec_output,
        turn_context.truncation_policy,
    ));
    sections.push("</result>".to_string());
    sections.join("\n")
}

pub fn format_user_shell_command_record(
    command: &str,
    exec_output: &ExecToolCallOutput,
    turn_context: &TurnContext,
) -> String {
    let body = format_user_shell_command_body(command, exec_output, turn_context);
    USER_SHELL_COMMAND_FRAGMENT.wrap(body)
}

pub fn user_shell_command_record_item(
    command: &str,
    exec_output: &ExecToolCallOutput,
    turn_context: &TurnContext,
) -> ResponseItem {
    USER_SHELL_COMMAND_FRAGMENT.into_message(format_user_shell_command_record(
        command,
        exec_output,
        turn_context,
    ))
}

#[cfg(test)]
#[path = "user_shell_command_tests.rs"]
mod tests;
