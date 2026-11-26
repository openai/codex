use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;

use super::picker_style::selection_style;
use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::ColumnWidthConfig;
use super::selection_popup_common::ColumnWidthMode;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::measure_rows_height_with_col_width_mode;
use super::selection_popup_common::render_rows_with_col_width_mode;
use super::slash_commands::BuiltinCommandFlags;
use super::slash_commands::CommandMatch;
use super::slash_commands::ServiceTierCommand;
use super::slash_commands::SlashCommandItem;
use super::slash_commands::commands_for_popup;
use super::slash_commands::match_command_name;
use crate::slash_command::SlashCommand;

// Hide alias commands in the default popup list so each unique action appears once.
// `quit` is an alias of `exit`, and `btw` is an alias of `side`, so we skip
// those aliases here.
const ALIAS_COMMANDS: &[SlashCommand] = &[SlashCommand::Quit, SlashCommand::Btw];
const COMMAND_COLUMN_WIDTH: ColumnWidthConfig = ColumnWidthConfig::new(
    ColumnWidthMode::AutoAllRows,
    /*name_column_width*/ None,
);

/// A selectable item in the popup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CommandItem {
    Builtin(SlashCommand),
    ServiceTier(ServiceTierCommand),
}

pub(crate) struct CommandPopup {
    disconnected: bool,
    command_filter: Option<String>,
    commands: Vec<CommandItem>,
    side_conversation_active: bool,
    state: ScrollState,
    daybreak_command_description: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CommandPopupFlags {
    pub(crate) disconnected: bool,
    pub(crate) collaboration_modes_enabled: bool,
    pub(crate) connectors_enabled: bool,
    pub(crate) plugins_command_enabled: bool,
    pub(crate) token_activity_command_enabled: bool,
    pub(crate) service_tier_commands_enabled: bool,
    pub(crate) daybreak_command_description: Option<&'static str>,
    pub(crate) goal_command_enabled: bool,
    pub(crate) voice_command_enabled: bool,
    pub(crate) worktrees_enabled: bool,
    pub(crate) windows_degraded_sandbox_active: bool,
    pub(crate) side_conversation_active: bool,
}

impl From<CommandPopupFlags> for BuiltinCommandFlags {
    fn from(value: CommandPopupFlags) -> Self {
        Self {
            collaboration_modes_enabled: value.collaboration_modes_enabled,
            connectors_enabled: value.connectors_enabled,
            plugins_command_enabled: value.plugins_command_enabled,
            token_activity_command_enabled: value.token_activity_command_enabled,
            service_tier_commands_enabled: value.service_tier_commands_enabled,
            daybreak_command_description: value.daybreak_command_description,
            goal_command_enabled: value.goal_command_enabled,
            voice_command_enabled: value.voice_command_enabled,
            worktrees_enabled: value.worktrees_enabled,
            allow_elevate_sandbox: value.windows_degraded_sandbox_active,
            side_conversation_active: value.side_conversation_active,
        }
    }
}

impl CommandPopup {
    pub(crate) fn new(
        flags: CommandPopupFlags,
        service_tier_commands: Vec<ServiceTierCommand>,
    ) -> Self {
        // Keep built-in availability in sync with the composer.
        let commands = commands_for_popup(flags.into(), &service_tier_commands)
            .into_iter()
            .map(|command| match command {
                SlashCommandItem::Builtin(cmd) => CommandItem::Builtin(cmd),
                SlashCommandItem::ServiceTier(command) => CommandItem::ServiceTier(command),
            })
            .collect();
        Self {
            disconnected: flags.disconnected,
            command_filter: None,
            commands,
            side_conversation_active: flags.side_conversation_active,
            state: ScrollState::new(),
            daybreak_command_description: flags.daybreak_command_description,
        }
    }

    /// Changing availability can reorder or hide rows. Never retarget a pending Enter.
    pub(crate) fn set_disconnected(&mut self, disconnected: bool) {
        let selected = self.current_item();
        self.disconnected = disconnected;
        self.restore_selection(selected);
    }

    pub(super) fn restore_selection(&mut self, selected: Option<CommandItem>) {
        let items = self.filtered_items();
        self.state.reset();
        self.state.selected_idx = items
            .iter()
            .position(|item| Some(item) == selected.as_ref());
        self.state.ensure_visible(items.len(), MAX_POPUP_ROWS);
    }

    /// Only a new filter selects the first match. Redraws must also preserve
    /// an empty selection if the previously highlighted command disappeared.
    pub(crate) fn on_composer_text_change(&mut self, text: String) {
        let filter = text
            .lines()
            .next()
            .and_then(|line| line.strip_prefix('/'))
            .and_then(|rest| rest.split_whitespace().next())
            .unwrap_or_default();
        if self.command_filter.as_deref() == Some(filter) {
            return;
        }
        self.command_filter = Some(filter.to_owned());
        self.state.reset();
        self.state.clamp_selection(self.filtered_items().len());
    }

    /// Determine the preferred height of the popup for a given width.
    /// Accounts for wrapped descriptions so that long tooltips don't overflow.
    pub(crate) fn calculate_required_height(&self, width: u16) -> u16 {
        let rows = self.rows_from_matches(self.filtered());

        measure_rows_height_with_col_width_mode(
            &rows,
            &self.state,
            MAX_POPUP_ROWS,
            width,
            COMMAND_COLUMN_WIDTH,
        )
    }

    /// Compute exact/prefix matches over built-in commands and user prompts,
    /// paired with optional highlight indices. Preserves the original
    /// presentation order for built-ins and prompts.
    fn filtered(&self) -> Vec<(CommandItem, Option<Vec<usize>>)> {
        let filter = self.command_filter.as_deref().unwrap_or_default();
        let mut out: Vec<(CommandItem, Option<Vec<usize>>)> = Vec::new();
        if filter.is_empty() {
            for command in self.commands.iter() {
                if matches!(command, CommandItem::Builtin(cmd) if ALIAS_COMMANDS.contains(cmd)) {
                    continue;
                }
                if self.unavailable_reason(command).is_some() {
                    continue;
                }
                out.push((command.clone(), None));
            }
            return out;
        }

        let filter_lower = filter.to_lowercase();
        let filter_chars = filter.chars().count();
        let mut prefix = Vec::new();
        for command in &self.commands {
            let matches = match match_command_name(command.command(), &filter_lower) {
                Some(CommandMatch::Exact) => &mut out,
                Some(CommandMatch::Prefix) => &mut prefix,
                None => continue,
            };
            matches.push((command.clone(), Some((0..filter_chars).collect())));
        }

        out.extend(prefix);
        if self.side_conversation_active || self.disconnected {
            out.sort_by_key(|(item, _)| self.unavailable_reason(item).is_some());
        }
        out
    }

    pub(crate) fn filtered_items(&self) -> Vec<CommandItem> {
        self.filtered().into_iter().map(|(c, _)| c).collect()
    }

    /// With no actionable selection, don't reinterpret a disabled offline row
    /// or a lost highlight as a request to submit the underlying draft.
    pub(crate) fn blocks_submit_fallback(&self) -> bool {
        (self.disconnected || self.state.selected_idx.is_none())
            && !self.filtered_items().is_empty()
    }

    fn rows_from_matches(
        &self,
        matches: Vec<(CommandItem, Option<Vec<usize>>)>,
    ) -> Vec<GenericDisplayRow> {
        matches
            .into_iter()
            .enumerate()
            .map(|(index, (item, indices))| {
                let name = format!("/{}", item.command());
                let description = match item {
                    CommandItem::Builtin(SlashCommand::Feedback) if self.disconnected => {
                        "show the current thread ID"
                    }
                    CommandItem::Builtin(SlashCommand::Daybreak) => self
                        .daybreak_command_description
                        .unwrap_or(item.description()),
                    _ => item.description(),
                }
                .to_string();
                let disabled_reason = self.unavailable_reason(&item);
                GenericDisplayRow {
                    name,
                    name_prefix_spans: vec![if self.state.selected_idx == Some(index)
                        && disabled_reason.is_none()
                    {
                        "› ".into()
                    } else {
                        "  ".into()
                    }],
                    selection_style: Some(selection_style()),
                    match_indices: indices.map(|v| v.into_iter().map(|i| i + 1).collect()),
                    description: Some(description),
                    is_disabled: disabled_reason.is_some(),
                    disabled_reason: disabled_reason.map(str::to_owned),
                    ..Default::default()
                }
            })
            .collect()
    }

    /// Move the selection cursor one step up.
    pub(crate) fn move_up(&mut self) {
        let len = self.filtered_items().len();
        for _ in 0..len {
            self.state.move_up_wrap(len);
            if self.selected_item().is_some() {
                break;
            }
        }
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    /// Move the selection cursor one step down.
    pub(crate) fn move_down(&mut self) {
        let matches_len = self.filtered_items().len();
        for _ in 0..matches_len {
            self.state.move_down_wrap(matches_len);
            if self.selected_item().is_some() {
                break;
            }
        }
        self.state
            .ensure_visible(matches_len, MAX_POPUP_ROWS.min(matches_len));
    }

    /// Remember the row even if it is temporarily disabled.
    pub(super) fn current_item(&self) -> Option<CommandItem> {
        self.state
            .selected_idx
            .and_then(|idx| self.filtered_items().into_iter().nth(idx))
    }

    pub(crate) fn selected_item(&self) -> Option<CommandItem> {
        self.current_item()
            .filter(|item| self.unavailable_reason(item).is_none())
    }

    fn unavailable_reason(&self, item: &CommandItem) -> Option<&'static str> {
        if self.disconnected
            && !matches!(item, CommandItem::Builtin(cmd) if cmd.available_when_disconnected())
        {
            return Some("requires a server connection");
        }
        if self.side_conversation_active
            && !matches!(item, CommandItem::Builtin(cmd) if cmd.available_in_side_conversation())
        {
            return Some("not available in a side conversation");
        }
        None
    }
}

impl CommandItem {
    pub(crate) fn command(&self) -> &str {
        match self {
            Self::Builtin(cmd) => cmd.command(),
            Self::ServiceTier(command) => &command.name,
        }
    }

    fn description(&self) -> &str {
        match self {
            Self::Builtin(cmd) => cmd.description(),
            Self::ServiceTier(command) => &command.description,
        }
    }
}

impl WidgetRef for CommandPopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let rows = self.rows_from_matches(self.filtered());
        render_rows_with_col_width_mode(
            area,
            buf,
            &rows,
            &self.state,
            MAX_POPUP_ROWS,
            "no matches",
            COMMAND_COLUMN_WIDTH,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn unavailable_side_conversation_command_is_shown_only_when_searched() {
        let mut popup = CommandPopup::new(
            CommandPopupFlags {
                side_conversation_active: true,
                ..CommandPopupFlags::default()
            },
            Vec::new(),
        );
        popup.on_composer_text_change("/".to_string());
        assert!(
            !popup
                .filtered_items()
                .contains(&CommandItem::Builtin(SlashCommand::Archive))
        );

        popup.on_composer_text_change("/arch".to_string());
        assert_eq!(
            popup.filtered_items(),
            vec![CommandItem::Builtin(SlashCommand::Archive)]
        );
        assert_eq!(popup.selected_item(), None);

        let width = 76;
        let area = Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            width,
            popup.calculate_required_height(width),
        );
        let mut buf = Buffer::empty(area);
        popup.render_ref(area, &mut buf);
        insta::assert_snapshot!("side_conversation_unavailable_command", format!("{buf:?}"));
        popup.on_composer_text_change("/a".to_string());
        popup.move_up();
        assert!(popup.selected_item().is_some());
    }

    #[test]
    fn filter_includes_init_when_typing_prefix() {
        let mut popup = CommandPopup::new(CommandPopupFlags::default(), Vec::new());
        // Simulate the composer line starting with '/in' so the popup filters
        // matching commands by prefix.
        popup.on_composer_text_change("/in".to_string());

        // Access the filtered list via the selected command and ensure that
        // one of the matches is the new "init" command.
        let matches = popup.filtered_items();
        let has_init = matches.iter().any(|item| match item {
            CommandItem::Builtin(cmd) => cmd.command() == "init",
            CommandItem::ServiceTier(_) => false,
        });
        assert!(
            has_init,
            "expected '/init' to appear among filtered commands"
        );
    }

    #[test]
    fn selecting_init_by_exact_match() {
        let mut popup = CommandPopup::new(CommandPopupFlags::default(), Vec::new());
        popup.on_composer_text_change("/init".to_string());

        // When an exact match exists, the selected command should be that
        // command by default.
        let selected = popup.selected_item();
        match selected {
            Some(CommandItem::Builtin(cmd)) => assert_eq!(cmd.command(), "init"),
            Some(CommandItem::ServiceTier(command)) => {
                panic!("expected init command, got service tier {command:?}")
            }
            None => panic!("expected a selected command for exact match"),
        }
    }

    #[test]
    fn model_is_first_suggestion_for_mo() {
        let mut popup = CommandPopup::new(CommandPopupFlags::default(), Vec::new());
        popup.on_composer_text_change("/mo".to_string());
        let matches = popup.filtered_items();
        match matches.first() {
            Some(CommandItem::Builtin(cmd)) => assert_eq!(cmd.command(), "model"),
            Some(CommandItem::ServiceTier(command)) => {
                panic!("expected model command, got service tier {command:?}")
            }
            None => panic!("expected at least one match for '/mo'"),
        }
    }

    #[test]
    fn service_tier_command_uses_catalog_name_and_description() {
        let mut popup = CommandPopup::new(
            CommandPopupFlags {
                service_tier_commands_enabled: true,
                ..CommandPopupFlags::default()
            },
            vec![ServiceTierCommand {
                id: "priority".to_string(),
                name: "fast".to_string(),
                description: "Fastest inference with increased plan usage".to_string(),
            }],
        );
        popup.on_composer_text_change("/fa".to_string());

        match popup.selected_item() {
            Some(CommandItem::ServiceTier(command)) => assert_eq!(
                command,
                ServiceTierCommand {
                    id: "priority".to_string(),
                    name: "fast".to_string(),
                    description: "Fastest inference with increased plan usage".to_string(),
                }
            ),
            other => panic!("expected fast service tier to be selected, got {other:?}"),
        }
        let rows = popup.rows_from_matches(popup.filtered());
        assert_eq!(
            rows.first().and_then(|row| row.description.as_deref()),
            Some("Fastest inference with increased plan usage")
        );
    }

    #[test]
    fn command_popup_wrap_boundary_preserves_following_choice() {
        let mut popup = CommandPopup::new(
            CommandPopupFlags {
                service_tier_commands_enabled: true,
                ..CommandPopupFlags::default()
            },
            vec![
                ServiceTierCommand {
                    id: "priority".to_string(),
                    name: "tier-one".to_string(),
                    description: "Use faster inference".to_string(),
                },
                ServiceTierCommand {
                    id: "default".to_string(),
                    name: "tier-two".to_string(),
                    description: "Keep default speed".to_string(),
                },
            ],
        );
        popup.on_composer_text_change("/tier".to_string());

        // The first description exactly fits at width 33, including the left inset.
        // Rendering at the requested height must also retain the second choice
        // when the first description wraps one column below that boundary.
        let mut snapshots = Vec::new();
        for width in [32, 33, 34] {
            let area = Rect::new(
                /*x*/ 0,
                /*y*/ 0,
                width,
                popup.calculate_required_height(width),
            );
            let mut buf = Buffer::empty(area);
            popup.render_ref(area, &mut buf);

            let snapshot = format!("{buf:?}");
            assert!(snapshot.contains("/tier-two"));
            assert!(snapshot.contains("Keep default speed"));
            snapshots.push(format!("width {width}\n{snapshot}"));
        }

        insta::assert_snapshot!("command_popup_wrap_boundary", snapshots.join("\n\n"));
    }

    #[test]
    fn filtered_commands_keep_presentation_order_for_prefix() {
        let mut popup = CommandPopup::new(CommandPopupFlags::default(), Vec::new());
        popup.on_composer_text_change("/m".to_string());

        let cmds: Vec<String> = popup
            .filtered_items()
            .into_iter()
            .map(|item| match item {
                CommandItem::Builtin(cmd) => cmd.command().to_string(),
                CommandItem::ServiceTier(command) => command.name,
            })
            .collect();
        assert_eq!(
            cmds,
            vec![
                "model".to_string(),
                "memories".to_string(),
                "mention".to_string(),
                "mcp".to_string()
            ]
        );
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn app_command_popup_snapshot() {
        let mut popup = CommandPopup::new(CommandPopupFlags::default(), Vec::new());
        popup.on_composer_text_change("/app".to_string());

        let width = 72;
        let area = Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            width,
            popup.calculate_required_height(width),
        );
        let mut buf = Buffer::empty(area);
        popup.render_ref(area, &mut buf);

        insta::assert_snapshot!("command_popup_app", format!("{buf:?}"));
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn voice_command_popup_snapshot() {
        let mut popup = CommandPopup::new(
            CommandPopupFlags {
                voice_command_enabled: true,
                ..CommandPopupFlags::default()
            },
            Vec::new(),
        );
        popup.on_composer_text_change("/voice".to_string());

        let width = 72;
        let area = Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            width,
            popup.calculate_required_height(width),
        );
        let mut buf = Buffer::empty(area);
        popup.render_ref(area, &mut buf);

        insta::assert_snapshot!("command_popup_voice", format!("{buf:?}"));
    }

    #[test]
    fn prefix_filter_limits_matches_for_ac() {
        let mut popup = CommandPopup::new(CommandPopupFlags::default(), Vec::new());
        popup.on_composer_text_change("/ac".to_string());

        let cmds: Vec<String> = popup
            .filtered_items()
            .into_iter()
            .map(|item| match item {
                CommandItem::Builtin(cmd) => cmd.command().to_string(),
                CommandItem::ServiceTier(command) => command.name,
            })
            .collect();
        assert!(
            !cmds.iter().any(|cmd| cmd == "compact"),
            "expected prefix search for '/ac' to exclude 'compact', got {cmds:?}"
        );
    }

    #[test]
    fn changing_filter_resets_selection_after_scrolling() {
        let mut popup = CommandPopup::new(CommandPopupFlags::default(), Vec::new());
        popup.on_composer_text_change("/".to_string());

        for _ in 0..MAX_POPUP_ROWS {
            popup.move_down();
        }
        assert!(popup.state.scroll_top > 0);

        popup.on_composer_text_change("/st".to_string());

        assert_eq!(
            popup.selected_item(),
            Some(CommandItem::Builtin(SlashCommand::Status))
        );
        assert_eq!(popup.state.scroll_top, 0);
        let width = 72;
        let area = Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            width,
            popup.calculate_required_height(width),
        );
        let mut buf = Buffer::empty(area);
        popup.render_ref(area, &mut buf);
        insta::assert_snapshot!(
            "command_popup_filter_reset_after_scroll",
            format!("{buf:?}")
        );
    }

    #[test]
    fn quit_hidden_in_empty_filter_but_shown_for_prefix() {
        let mut popup = CommandPopup::new(CommandPopupFlags::default(), Vec::new());
        popup.on_composer_text_change("/".to_string());
        let items = popup.filtered_items();
        assert!(!items.contains(&CommandItem::Builtin(SlashCommand::Quit)));

        popup.on_composer_text_change("/qu".to_string());
        let items = popup.filtered_items();
        assert!(items.contains(&CommandItem::Builtin(SlashCommand::Quit)));
    }

    #[test]
    fn btw_hidden_in_empty_filter_but_shown_for_prefix() {
        let mut popup = CommandPopup::new(CommandPopupFlags::default(), Vec::new());
        popup.on_composer_text_change("/".to_string());
        let items = popup.filtered_items();
        assert!(!items.contains(&CommandItem::Builtin(SlashCommand::Btw)));

        popup.on_composer_text_change("/bt".to_string());
        let items = popup.filtered_items();
        assert!(items.contains(&CommandItem::Builtin(SlashCommand::Btw)));
    }

    #[test]
    fn plan_command_hidden_when_collaboration_modes_disabled() {
        let mut popup = CommandPopup::new(CommandPopupFlags::default(), Vec::new());
        popup.on_composer_text_change("/".to_string());

        let cmds: Vec<String> = popup
            .filtered_items()
            .into_iter()
            .map(|item| match item {
                CommandItem::Builtin(cmd) => cmd.command().to_string(),
                CommandItem::ServiceTier(command) => command.name,
            })
            .collect();
        assert!(
            !cmds.iter().any(|cmd| cmd == "plan"),
            "expected '/plan' to be hidden when collaboration modes are disabled, got {cmds:?}"
        );
    }

    #[test]
    fn plan_command_visible_when_collaboration_modes_enabled() {
        let mut popup = CommandPopup::new(
            CommandPopupFlags {
                disconnected: false,
                collaboration_modes_enabled: true,
                connectors_enabled: false,
                plugins_command_enabled: false,
                token_activity_command_enabled: false,
                service_tier_commands_enabled: false,
                daybreak_command_description: None,
                goal_command_enabled: false,
                voice_command_enabled: false,
                worktrees_enabled: true,
                windows_degraded_sandbox_active: false,
                side_conversation_active: false,
            },
            Vec::new(),
        );
        popup.on_composer_text_change("/plan".to_string());

        match popup.selected_item() {
            Some(CommandItem::Builtin(cmd)) => assert_eq!(cmd.command(), "plan"),
            Some(CommandItem::ServiceTier(command)) => {
                panic!("expected plan command, got service tier {command:?}")
            }
            other => panic!("expected plan to be selected for exact match, got {other:?}"),
        }
    }

    #[test]
    fn debug_commands_are_hidden_from_popup() {
        let popup = CommandPopup::new(CommandPopupFlags::default(), Vec::new());
        let cmds: Vec<String> = popup
            .filtered_items()
            .into_iter()
            .map(|item| match item {
                CommandItem::Builtin(cmd) => cmd.command().to_string(),
                CommandItem::ServiceTier(command) => command.name,
            })
            .collect();

        assert!(
            !cmds.iter().any(|name| name.starts_with("debug")),
            "expected no /debug* command in popup menu, got {cmds:?}"
        );
    }
}
