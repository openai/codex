use std::time::Duration;

use codex_protocol::artificial_messages::ArtificialMessage;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;

use crate::codex::TurnContext;
use crate::exec::ExecToolCallOutput;
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
    ArtificialMessage::UserShellCommand { body }.render()
}

pub fn user_shell_command_record_item(
    command: &str,
    exec_output: &ExecToolCallOutput,
    turn_context: &TurnContext,
) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: format_user_shell_command_record(command, exec_output, turn_context),
        }],
        end_turn: None,
        phase: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::make_session_and_context;
    use crate::exec::StreamOutput;
    use codex_protocol::models::ContentItem;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn formats_basic_record() {
        let exec_output = ExecToolCallOutput {
            exit_code: 0,
            stdout: StreamOutput::new("hi".to_string()),
            stderr: StreamOutput::new(String::new()),
            aggregated_output: StreamOutput::new("hi".to_string()),
            duration: Duration::from_secs(1),
            timed_out: false,
        };
        let (_, turn_context) = make_session_and_context().await;
        let item = user_shell_command_record_item("echo hi", &exec_output, &turn_context);
        let ResponseItem::Message { content, .. } = item else {
            panic!("expected message");
        };
        let [ContentItem::InputText { text }] = content.as_slice() else {
            panic!("expected input text");
        };
        assert_eq!(
            text,
            "<user_shell_cmd><command>\necho hi\n</command>\n<result>\nExit code: 0\nDuration: 1.0000 seconds\nOutput:\nhi\n</result></user_shell_cmd>"
        );
    }

    #[tokio::test]
    async fn uses_aggregated_output_over_streams() {
        let exec_output = ExecToolCallOutput {
            exit_code: 42,
            stdout: StreamOutput::new("stdout-only".to_string()),
            stderr: StreamOutput::new("stderr-only".to_string()),
            aggregated_output: StreamOutput::new("combined output wins".to_string()),
            duration: Duration::from_millis(120),
            timed_out: false,
        };
        let (_, turn_context) = make_session_and_context().await;
        let record = format_user_shell_command_record("false", &exec_output, &turn_context);
        assert_eq!(
            record,
            "<user_shell_cmd><command>\nfalse\n</command>\n<result>\nExit code: 42\nDuration: 0.1200 seconds\nOutput:\ncombined output wins\n</result></user_shell_cmd>"
        );
    }
}
