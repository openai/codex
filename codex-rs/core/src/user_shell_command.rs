use std::time::Duration;

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;

use crate::exec::ExecToolCallOutput;
use crate::tools::format_exec_output_str;

pub const USER_SHELL_COMMAND_OPEN: &str = "<user_shell_command>";
pub const USER_SHELL_COMMAND_CLOSE: &str = "</user_shell_command>";

pub fn is_user_shell_command_text(text: &str) -> bool {
    let trimmed = text.trim_start();
    let lowered = trimmed.to_ascii_lowercase();
    lowered.starts_with(USER_SHELL_COMMAND_OPEN)
}

fn format_duration_line(duration: Duration) -> String {
    let duration_seconds = duration.as_secs_f64();
    format!("Duration: {duration_seconds:.4} seconds")
}

fn format_user_shell_command_body(command: &str, exec_output: &ExecToolCallOutput) -> String {
    let mut sections = Vec::new();
    sections.push("<command>".to_string());
    sections.push(command.to_string());
    sections.push("</command>".to_string());
    sections.push("<result>".to_string());
    sections.push(format!("Exit code: {}", exec_output.exit_code));
    sections.push(format_duration_line(exec_output.duration));
    sections.push("Output:".to_string());
    sections.push(format_exec_output_str(exec_output));
    sections.push("</result>".to_string());
    sections.join("\n")
}

pub fn format_user_shell_command_record(command: &str, exec_output: &ExecToolCallOutput) -> String {
    let body = format_user_shell_command_body(command, exec_output);
    format!("{USER_SHELL_COMMAND_OPEN}\n{body}\n{USER_SHELL_COMMAND_CLOSE}")
}

pub fn user_shell_command_record_item(
    command: &str,
    exec_output: &ExecToolCallOutput,
) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: format_user_shell_command_record(command, exec_output),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::StreamOutput;
    use pretty_assertions::assert_eq;

    #[test]
    fn detects_user_shell_command_text_variants() {
        assert!(is_user_shell_command_text(
            "<user_shell_command>\necho hi\n</user_shell_command>"
        ));
        assert!(!is_user_shell_command_text("echo hi"));
    }

    #[test]
    fn builds_record_item_with_freeform_sections() {
        let exec_output = ExecToolCallOutput {
            exit_code: 0,
            stdout: StreamOutput::new("hi".to_string()),
            stderr: StreamOutput::new(String::new()),
            aggregated_output: StreamOutput::new("hi".to_string()),
            duration: Duration::from_secs(1),
            timed_out: false,
        };
        let item = user_shell_command_record_item("echo hi", &exec_output);
        let ResponseItem::Message { content, .. } = item else {
            panic!("expected message");
        };
        let [ContentItem::InputText { text }] = content.as_slice() else {
            panic!("expected input text");
        };
        assert_eq!(
            text,
            "<user_shell_command>\n<command>\necho hi\n</command>\n<result>\nExit code: 0\nDuration: 1.0000 seconds\nOutput:\nhi\n</result>\n</user_shell_command>"
        );
    }

    #[test]
    fn formats_stderr_only_output() {
        let exec_output = ExecToolCallOutput {
            exit_code: 42,
            stdout: StreamOutput::new(String::new()),
            stderr: StreamOutput::new("boom".to_string()),
            aggregated_output: StreamOutput::new("boom".to_string()),
            duration: Duration::from_millis(120),
            timed_out: false,
        };
        let record = format_user_shell_command_record("false", &exec_output);
        assert_eq!(
            record,
            "<user_shell_command>\n<command>\nfalse\n</command>\n<result>\nExit code: 42\nDuration: 0.1200 seconds\nOutput:\nboom\n</result>\n</user_shell_command>"
        );
    }

    #[test]
    fn formats_combined_output_in_order() {
        let combined = "out line\nerr line";
        let exec_output = ExecToolCallOutput {
            exit_code: 1,
            stdout: StreamOutput::new("out line\n".to_string()),
            stderr: StreamOutput::new("err line".to_string()),
            aggregated_output: StreamOutput::new(combined.to_string()),
            duration: Duration::from_millis(250),
            timed_out: false,
        };
        let record = format_user_shell_command_record("cmd", &exec_output);
        assert_eq!(
            record,
            "<user_shell_command>\n<command>\ncmd\n</command>\n<result>\nExit code: 1\nDuration: 0.2500 seconds\nOutput:\nout line\nerr line\n</result>\n</user_shell_command>"
        );
    }
}
