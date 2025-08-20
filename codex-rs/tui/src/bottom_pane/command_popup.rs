use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;

use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::render_rows;
use crate::slash_command::SlashCommand;
use crate::slash_command::built_in_slash_commands;
use codex_common::fuzzy_match::fuzzy_match;

pub(crate) struct CommandPopup {
    command_filter: String,
    all_commands: Vec<(&'static str, SlashCommand)>,
    state: ScrollState,
}

impl CommandPopup {
    pub(crate) fn new() -> Self {
        Self {
            command_filter: String::new(),
            all_commands: built_in_slash_commands(),
            state: ScrollState::new(),
        }
    }

    /// Update the filter string based on the current composer text. The text
    /// passed in is expected to start with a leading '/'. Everything after the
    /// *first* '/" on the *first* line becomes the active filter that is used
    /// to narrow down the list of available commands.
    pub(crate) fn on_composer_text_change(&mut self, text: String) {
        let first_line = text.lines().next().unwrap_or("");

        if let Some(stripped) = first_line.strip_prefix('/') {
            // Extract the *first* token (sequence of non-whitespace
            // characters) after the slash so that `/clear something` still
            // shows the help for `/clear`.
            let token = stripped.trim_start();
            let cmd_token = token.split_whitespace().next().unwrap_or("");

            // Update the filter keeping the original case (commands are all
            // lower-case for now but this may change in the future).
            self.command_filter = cmd_token.to_string();
        } else {
            // The composer no longer starts with '/'. Reset the filter so the
            // popup shows the *full* command list if it is still displayed
            // for some reason.
            self.command_filter.clear();
        }

        // Reset or clamp selected index based on new filtered list.
        let matches_len = self.filtered_commands().len();
        self.state.clamp_selection(matches_len);
        self.state
            .ensure_visible(matches_len, MAX_POPUP_ROWS.min(matches_len));
    }

    /// Determine the preferred height of the popup. This is the number of
    /// rows required to show at most MAX_POPUP_ROWS commands.
    pub(crate) fn calculate_required_height(&self) -> u16 {
        self.filtered_commands().len().clamp(1, MAX_POPUP_ROWS) as u16
    }

    /// Compute fuzzy-filtered matches paired with optional highlight indices and score.
    /// Sorted by ascending score, then by command name for stability.
    fn filtered(&self) -> Vec<(&SlashCommand, Option<Vec<usize>>, i32)> {
        let filter = self.command_filter.trim();
        let mut out: Vec<(&SlashCommand, Option<Vec<usize>>, i32)> = Vec::new();
        if filter.is_empty() {
            for (_, cmd) in self.all_commands.iter() {
                out.push((cmd, None, 0));
            }
            // Keep the original presentation order when no filter is applied.
            return out;
        } else {
            for (_, cmd) in self.all_commands.iter() {
                if let Some((indices, score)) = fuzzy_match(cmd.command(), filter) {
                    out.push((cmd, Some(indices), score));
                }
            }
        }
        // When filtering, sort by ascending score and then by command for stability.
        out.sort_by(|a, b| a.2.cmp(&b.2).then_with(|| a.0.command().cmp(b.0.command())));
        out
    }

    fn filtered_commands(&self) -> Vec<&SlashCommand> {
        self.filtered().into_iter().map(|(c, _, _)| c).collect()
    }

    /// Move the selection cursor one step up.
    pub(crate) fn move_up(&mut self) {
        let matches = self.filtered_commands();
        let len = matches.len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    /// Move the selection cursor one step down.
    pub(crate) fn move_down(&mut self) {
        let matches = self.filtered_commands();
        let matches_len = matches.len();
        self.state.move_down_wrap(matches_len);
        self.state
            .ensure_visible(matches_len, MAX_POPUP_ROWS.min(matches_len));
    }

    /// Return currently selected command, if any.
    pub(crate) fn selected_command(&self) -> Option<&SlashCommand> {
        let matches = self.filtered_commands();
        self.state
            .selected_idx
            .and_then(|idx| matches.get(idx).copied())
    }
}

impl WidgetRef for CommandPopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let matches = self.filtered();
        let rows_all: Vec<GenericDisplayRow> = if matches.is_empty() {
            Vec::new()
        } else {
            matches
                .into_iter()
                .map(|(cmd, indices, _)| GenericDisplayRow {
                    name: format!("/{}", cmd.command()),
                    match_indices: indices.map(|v| v.into_iter().map(|i| i + 1).collect()),
                    is_current: false,
                    description: Some(cmd.description().to_string()),
                })
                .collect()
        };
        render_rows(area, buf, &rows_all, &self.state, MAX_POPUP_ROWS, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_includes_init_when_typing_prefix() {
        let mut popup = CommandPopup::new();
        // Simulate the composer line starting with '/in' so the popup filters
        // matching commands by prefix.
        popup.on_composer_text_change("/in".to_string());

        // Access the filtered list via the selected command and ensure that
        // one of the matches is the new "init" command.
        let matches = popup.filtered_commands();
        assert!(
            matches.iter().any(|cmd| cmd.command() == "init"),
            "expected '/init' to appear among filtered commands"
        );
    }

    #[test]
    fn selecting_init_by_exact_match() {
        let mut popup = CommandPopup::new();
        popup.on_composer_text_change("/init".to_string());

        // When an exact match exists, the selected command should be that
        // command by default.
        let selected = popup.selected_command();
        match selected {
            Some(cmd) => assert_eq!(cmd.command(), "init"),
            None => panic!("expected a selected command for exact match"),
        }
    }
}
