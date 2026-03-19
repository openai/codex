use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;

use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::render_rows;
use super::slash_commands;
use crate::render::Insets;
use crate::render::RectExt;
use crate::slash_command::SlashCommand;

const ALIAS_COMMANDS: &[SlashCommand] = &[SlashCommand::Quit, SlashCommand::Approvals];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommandItem {
    Builtin(SlashCommand),
}

pub(crate) struct CommandPopup {
    command_filter: String,
    builtins: Vec<(&'static str, SlashCommand)>,
    state: ScrollState,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CommandPopupFlags {
    pub(crate) collaboration_modes_enabled: bool,
    pub(crate) connectors_enabled: bool,
    pub(crate) fast_command_enabled: bool,
    pub(crate) personality_command_enabled: bool,
    pub(crate) realtime_conversation_enabled: bool,
    pub(crate) audio_device_selection_enabled: bool,
    pub(crate) windows_degraded_sandbox_active: bool,
}

impl From<CommandPopupFlags> for slash_commands::BuiltinCommandFlags {
    fn from(value: CommandPopupFlags) -> Self {
        Self {
            collaboration_modes_enabled: value.collaboration_modes_enabled,
            connectors_enabled: value.connectors_enabled,
            fast_command_enabled: value.fast_command_enabled,
            personality_command_enabled: value.personality_command_enabled,
            realtime_conversation_enabled: value.realtime_conversation_enabled,
            audio_device_selection_enabled: value.audio_device_selection_enabled,
            allow_elevate_sandbox: value.windows_degraded_sandbox_active,
        }
    }
}

impl CommandPopup {
    pub(crate) fn new(flags: CommandPopupFlags) -> Self {
        let builtins: Vec<(&'static str, SlashCommand)> =
            slash_commands::builtins_for_input(flags.into())
                .into_iter()
                .filter(|(name, _)| !name.starts_with("debug"))
                .collect();
        Self {
            command_filter: String::new(),
            builtins,
            state: ScrollState::new(),
        }
    }

    pub(crate) fn on_composer_text_change(&mut self, text: String) {
        let first_line = text.lines().next().unwrap_or("");

        if let Some(stripped) = first_line.strip_prefix('/') {
            let token = stripped.trim_start();
            let cmd_token = token.split_whitespace().next().unwrap_or("");
            self.command_filter = cmd_token.to_string();
        } else {
            self.command_filter.clear();
        }

        let matches_len = self.filtered_items().len();
        self.state.clamp_selection(matches_len);
        self.state
            .ensure_visible(matches_len, MAX_POPUP_ROWS.min(matches_len));
    }

    pub(crate) fn calculate_required_height(&self, width: u16) -> u16 {
        use super::selection_popup_common::measure_rows_height;
        let rows = self.rows_from_matches(self.filtered());

        measure_rows_height(&rows, &self.state, MAX_POPUP_ROWS, width)
    }

    fn filtered(&self) -> Vec<(CommandItem, Option<Vec<usize>>)> {
        let filter = self.command_filter.trim();
        let mut out: Vec<(CommandItem, Option<Vec<usize>>)> = Vec::new();
        if filter.is_empty() {
            for (_, cmd) in &self.builtins {
                if ALIAS_COMMANDS.contains(cmd) {
                    continue;
                }
                out.push((CommandItem::Builtin(*cmd), None));
            }
            return out;
        }

        let filter_lower = filter.to_lowercase();
        let filter_chars = filter.chars().count();
        let mut exact = Vec::new();
        let mut prefix = Vec::new();
        let indices_for = || Some((0..filter_chars).collect());

        for (_, cmd) in &self.builtins {
            let command = cmd.command();
            let command_lower = command.to_lowercase();
            if command_lower == filter_lower {
                exact.push((CommandItem::Builtin(*cmd), indices_for()));
            } else if command_lower.starts_with(&filter_lower) {
                prefix.push((CommandItem::Builtin(*cmd), indices_for()));
            }
        }

        out.extend(exact);
        out.extend(prefix);
        out
    }

    fn filtered_items(&self) -> Vec<CommandItem> {
        self.filtered()
            .into_iter()
            .map(|(command, _)| command)
            .collect()
    }

    fn rows_from_matches(
        &self,
        matches: Vec<(CommandItem, Option<Vec<usize>>)>,
    ) -> Vec<GenericDisplayRow> {
        matches
            .into_iter()
            .map(|(item, indices)| {
                let CommandItem::Builtin(cmd) = item;
                GenericDisplayRow {
                    name: format!("/{}", cmd.command()),
                    name_prefix_spans: Vec::new(),
                    match_indices: indices.map(|v| v.into_iter().map(|i| i + 1).collect()),
                    display_shortcut: None,
                    description: Some(cmd.description().to_string()),
                    category_tag: None,
                    wrap_indent: None,
                    is_disabled: false,
                    disabled_reason: None,
                }
            })
            .collect()
    }

    pub(crate) fn move_up(&mut self) {
        let len = self.filtered_items().len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    pub(crate) fn move_down(&mut self) {
        let matches_len = self.filtered_items().len();
        self.state.move_down_wrap(matches_len);
        self.state
            .ensure_visible(matches_len, MAX_POPUP_ROWS.min(matches_len));
    }

    pub(crate) fn selected_item(&self) -> Option<CommandItem> {
        let matches = self.filtered_items();
        self.state
            .selected_idx
            .and_then(|idx| matches.get(idx).copied())
    }
}

impl WidgetRef for CommandPopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let rows = self.rows_from_matches(self.filtered());
        render_rows(
            area.inset(Insets::tlbr(
                /*top*/ 0, /*left*/ 2, /*bottom*/ 0, /*right*/ 0,
            )),
            buf,
            &rows,
            &self.state,
            MAX_POPUP_ROWS,
            "no matches",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn filter_includes_init_when_typing_prefix() {
        let mut popup = CommandPopup::new(CommandPopupFlags::default());
        popup.on_composer_text_change("/in".to_string());

        let matches = popup.filtered_items();
        let has_init = matches.iter().any(|item| match item {
            CommandItem::Builtin(cmd) => cmd.command() == "init",
        });
        assert!(
            has_init,
            "expected '/init' to appear among filtered commands"
        );
    }

    #[test]
    fn selecting_init_by_exact_match() {
        let mut popup = CommandPopup::new(CommandPopupFlags::default());
        popup.on_composer_text_change("/init".to_string());

        let selected = popup.selected_item();
        match selected {
            Some(CommandItem::Builtin(cmd)) => assert_eq!(cmd.command(), "init"),
            None => panic!("expected a selected command for exact match"),
        }
    }

    #[test]
    fn model_is_first_suggestion_for_mo() {
        let mut popup = CommandPopup::new(CommandPopupFlags::default());
        popup.on_composer_text_change("/mo".to_string());
        let matches = popup.filtered_items();
        match matches.first() {
            Some(CommandItem::Builtin(cmd)) => assert_eq!(cmd.command(), "model"),
            None => panic!("expected at least one match for '/mo'"),
        }
    }

    #[test]
    fn filtered_commands_keep_presentation_order_for_prefix() {
        let mut popup = CommandPopup::new(CommandPopupFlags::default());
        popup.on_composer_text_change("/m".to_string());

        let cmds: Vec<&str> = popup
            .filtered_items()
            .into_iter()
            .map(|item| match item {
                CommandItem::Builtin(cmd) => cmd.command(),
            })
            .collect();
        assert_eq!(cmds, vec!["model", "mention", "mcp"]);
    }

    #[test]
    fn prefix_filter_limits_matches_for_ac() {
        let mut popup = CommandPopup::new(CommandPopupFlags::default());
        popup.on_composer_text_change("/ac".to_string());

        let cmds: Vec<&str> = popup
            .filtered_items()
            .into_iter()
            .map(|item| match item {
                CommandItem::Builtin(cmd) => cmd.command(),
            })
            .collect();
        assert!(
            !cmds.contains(&"compact"),
            "expected prefix search for '/ac' to exclude 'compact', got {cmds:?}"
        );
    }

    #[test]
    fn quit_hidden_in_empty_filter_but_shown_for_prefix() {
        let mut popup = CommandPopup::new(CommandPopupFlags::default());
        popup.on_composer_text_change("/".to_string());
        let items = popup.filtered_items();
        assert!(!items.contains(&CommandItem::Builtin(SlashCommand::Quit)));

        popup.on_composer_text_change("/qu".to_string());
        let items = popup.filtered_items();
        assert!(items.contains(&CommandItem::Builtin(SlashCommand::Quit)));
    }

    #[test]
    fn collab_command_hidden_when_collaboration_modes_disabled() {
        let mut popup = CommandPopup::new(CommandPopupFlags::default());
        popup.on_composer_text_change("/".to_string());

        let cmds: Vec<&str> = popup
            .filtered_items()
            .into_iter()
            .map(|item| match item {
                CommandItem::Builtin(cmd) => cmd.command(),
            })
            .collect();
        assert!(
            !cmds.contains(&"collab"),
            "expected '/collab' to be hidden when collaboration modes are disabled, got {cmds:?}"
        );
        assert!(
            !cmds.contains(&"plan"),
            "expected '/plan' to be hidden when collaboration modes are disabled, got {cmds:?}"
        );
    }

    #[test]
    fn collab_command_visible_when_collaboration_modes_enabled() {
        let mut popup = CommandPopup::new(CommandPopupFlags {
            collaboration_modes_enabled: true,
            connectors_enabled: false,
            fast_command_enabled: false,
            personality_command_enabled: true,
            realtime_conversation_enabled: false,
            audio_device_selection_enabled: false,
            windows_degraded_sandbox_active: false,
        });
        popup.on_composer_text_change("/collab".to_string());

        match popup.selected_item() {
            Some(CommandItem::Builtin(cmd)) => assert_eq!(cmd.command(), "collab"),
            other => panic!("expected collab to be selected for exact match, got {other:?}"),
        }
    }

    #[test]
    fn plan_command_visible_when_collaboration_modes_enabled() {
        let mut popup = CommandPopup::new(CommandPopupFlags {
            collaboration_modes_enabled: true,
            connectors_enabled: false,
            fast_command_enabled: false,
            personality_command_enabled: true,
            realtime_conversation_enabled: false,
            audio_device_selection_enabled: false,
            windows_degraded_sandbox_active: false,
        });
        popup.on_composer_text_change("/plan".to_string());

        match popup.selected_item() {
            Some(CommandItem::Builtin(cmd)) => assert_eq!(cmd.command(), "plan"),
            other => panic!("expected plan to be selected for exact match, got {other:?}"),
        }
    }

    #[test]
    fn personality_command_hidden_when_disabled() {
        let mut popup = CommandPopup::new(CommandPopupFlags {
            collaboration_modes_enabled: true,
            connectors_enabled: false,
            fast_command_enabled: false,
            personality_command_enabled: false,
            realtime_conversation_enabled: false,
            audio_device_selection_enabled: false,
            windows_degraded_sandbox_active: false,
        });
        popup.on_composer_text_change("/pers".to_string());

        let cmds: Vec<&str> = popup
            .filtered_items()
            .into_iter()
            .map(|item| match item {
                CommandItem::Builtin(cmd) => cmd.command(),
            })
            .collect();
        assert!(
            !cmds.contains(&"personality"),
            "expected '/personality' to be hidden when disabled, got {cmds:?}"
        );
    }

    #[test]
    fn personality_command_visible_when_enabled() {
        let mut popup = CommandPopup::new(CommandPopupFlags {
            collaboration_modes_enabled: true,
            connectors_enabled: false,
            fast_command_enabled: false,
            personality_command_enabled: true,
            realtime_conversation_enabled: false,
            audio_device_selection_enabled: false,
            windows_degraded_sandbox_active: false,
        });
        popup.on_composer_text_change("/personality".to_string());

        match popup.selected_item() {
            Some(CommandItem::Builtin(cmd)) => assert_eq!(cmd.command(), "personality"),
            other => panic!("expected personality to be selected for exact match, got {other:?}"),
        }
    }

    #[test]
    fn settings_command_hidden_when_audio_device_selection_is_disabled() {
        let mut popup = CommandPopup::new(CommandPopupFlags {
            collaboration_modes_enabled: false,
            connectors_enabled: false,
            fast_command_enabled: false,
            personality_command_enabled: true,
            realtime_conversation_enabled: true,
            audio_device_selection_enabled: false,
            windows_degraded_sandbox_active: false,
        });
        popup.on_composer_text_change("/aud".to_string());

        let cmds: Vec<&str> = popup
            .filtered_items()
            .into_iter()
            .map(|item| match item {
                CommandItem::Builtin(cmd) => cmd.command(),
            })
            .collect();

        assert!(
            !cmds.contains(&"settings"),
            "expected '/settings' to be hidden when audio device selection is disabled, got {cmds:?}"
        );
    }

    #[test]
    fn debug_commands_are_hidden_from_popup() {
        let popup = CommandPopup::new(CommandPopupFlags::default());
        let cmds: Vec<&str> = popup
            .filtered_items()
            .into_iter()
            .map(|item| match item {
                CommandItem::Builtin(cmd) => cmd.command(),
            })
            .collect();

        assert!(
            !cmds.iter().any(|name| name.starts_with("debug")),
            "expected no /debug* command in popup menu, got {cmds:?}"
        );
    }
}
