use codex_app_server_protocol::ThreadItem;

/// Returns false for protocol items that are retained for session bookkeeping but are not part of
/// the user-visible conversation. The app-server protocol does not currently carry a general
/// visibility flag, so require both the internal command marker and its recipient metadata before
/// suppressing an item.
pub(crate) fn is_user_visible_thread_item(item: &ThreadItem) -> bool {
    !is_internal_clock_wait_thread_item(item)
}

pub(crate) fn is_internal_clock_wait_thread_item(item: &ThreadItem) -> bool {
    let ThreadItem::CommandExecution {
        command,
        aggregated_output,
        ..
    } = item
    else {
        return false;
    };

    let is_cot_command = command == "[cot]" || command.starts_with("[cot] ");
    let is_clock_wait = aggregated_output
        .as_deref()
        .is_some_and(|output| output.lines().any(|line| line == "Recipient: clock.wait"));

    is_cot_command && is_clock_wait
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::CommandExecutionSource;
    use codex_app_server_protocol::CommandExecutionStatus;
    use codex_utils_absolute_path::AbsolutePathBuf;

    fn command_item(command: &str, output: Option<&str>) -> ThreadItem {
        ThreadItem::CommandExecution {
            id: "item-1".to_string(),
            command: command.to_string(),
            cwd: AbsolutePathBuf::from_absolute_path("/tmp").expect("absolute path"),
            process_id: None,
            status: CommandExecutionStatus::Completed,
            command_actions: Vec::new(),
            aggregated_output: output.map(str::to_string),
            exit_code: Some(0),
            duration_ms: None,
            source: CommandExecutionSource::Agent,
        }
    }

    #[test]
    fn hides_internal_clock_wait_command() {
        let item = command_item(
            "[cot] {\"seconds\":7200}",
            Some("Channel: analysis\nRecipient: clock.wait\nStream channel: agent"),
        );

        assert!(!is_user_visible_thread_item(&item));
    }

    #[test]
    fn does_not_hide_user_commands_that_only_resemble_internal_items() {
        assert!(is_user_visible_thread_item(&command_item(
            "[cot] hello",
            None
        )));
        assert!(is_user_visible_thread_item(&command_item(
            "echo hello",
            Some("Recipient: clock.wait"),
        )));
    }
}
