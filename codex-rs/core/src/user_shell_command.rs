use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;

pub const USER_SHELL_COMMAND_OPEN: &str = "<user_shell_command>";
pub const USER_SHELL_COMMAND_CLOSE: &str = "</user_shell_command>";
pub const USER_SHELL_COMMAND_OUTPUT_OPEN: &str = "<user_shell_command_output>";
pub const USER_SHELL_COMMAND_OUTPUT_CLOSE: &str = "</user_shell_command_output>";

pub fn is_user_shell_command_text(text: &str) -> bool {
    let trimmed = text.trim_start();
    let lowered = trimmed.to_ascii_lowercase();
    lowered.starts_with(USER_SHELL_COMMAND_OPEN)
        || lowered.starts_with(USER_SHELL_COMMAND_OUTPUT_OPEN)
}

pub fn format_user_shell_command_record(command: &str, output: &str) -> String {
    format!(
        "{USER_SHELL_COMMAND_OPEN}\n{command}\n{USER_SHELL_COMMAND_CLOSE}\n{USER_SHELL_COMMAND_OUTPUT_OPEN}\n{output}\n{USER_SHELL_COMMAND_OUTPUT_CLOSE}"
    )
}

pub fn user_shell_command_record_item(command: &str, output: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: format_user_shell_command_record(command, output),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_user_shell_command_text_variants() {
        assert!(is_user_shell_command_text(
            "<user_shell_command>\necho hi\n</user_shell_command>"
        ));
        assert!(is_user_shell_command_text(
            "   <user_shell_command_output>\nhi\n</user_shell_command_output>"
        ));
        assert!(!is_user_shell_command_text("echo hi"));
    }

    #[test]
    fn builds_record_item_with_both_sections() {
        let item = user_shell_command_record_item("echo hi", "hi");
        let ResponseItem::Message { content, .. } = item else {
            panic!("expected message");
        };
        let [ContentItem::InputText { text }] = content.as_slice() else {
            panic!("expected input text");
        };
        assert_eq!(
            text,
            "<user_shell_command>\necho hi\n</user_shell_command>\n<user_shell_command_output>\nhi\n</user_shell_command_output>"
        );
    }
}
