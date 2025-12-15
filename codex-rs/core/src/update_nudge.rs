use crate::update_action::get_update_action;

pub(crate) fn update_available_nudge() -> String {
    match get_update_action() {
        Some(action) => {
            let command = action.command_str();
            format!("Update available. Run `{command}` to update.")
        }
        None => "Update available. See https://github.com/openai/codex for installation options."
            .to_string(),
    }
}
