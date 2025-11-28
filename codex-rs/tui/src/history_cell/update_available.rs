use super::HistoryCell;
use super::with_border_with_inner_width;
use crate::update_action::UpdateAction;
use crate::version::CODEX_CLI_VERSION;
use ratatui::style::Stylize;
use ratatui::text::Line;

/// Banner shown when a newer Codex CLI is available.
///
/// Renders a boxed notice with the current → latest version, a sparkle emoji header, and either a
/// runnable update command (when the CLI knows how it was installed) or a link to release notes.
///
/// # Output
///
/// ```plain
/// ╭─────────────────────────────────╮
/// │ ✨Update available! 1.0 -> 1.2.3 │
/// │ Run brew upgrade codex to update.│
/// ╰─────────────────────────────────╯
/// ```
#[cfg_attr(debug_assertions, allow(dead_code))]
#[derive(Debug)]
pub(crate) struct UpdateAvailableHistoryCell {
    pub(crate) latest_version: String,
    pub(crate) update_action: Option<UpdateAction>,
}

#[cfg_attr(debug_assertions, allow(dead_code))]
impl UpdateAvailableHistoryCell {
    /// Build an update banner describing the latest version and how to upgrade.
    ///
    /// `latest_version` is shown alongside the current CLI version; `update_action` controls
    /// whether we render a concrete command or fall back to a docs link.
    pub(crate) fn new(latest_version: String, update_action: Option<UpdateAction>) -> Self {
        Self {
            latest_version,
            update_action,
        }
    }
}

impl HistoryCell for UpdateAvailableHistoryCell {
    /// Render a boxed update notice with version delta and a follow-up action.
    ///
    /// The header shows a cyan sparkle and “Update available!” plus `{current} -> {latest}`. The
    /// following line either displays a runnable command (if known) or a link to installation
    /// options, then a link to release notes. The content is wrapped inside a border sized to the
    /// available width minus outer padding.
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        use ratatui_macros::line;
        use ratatui_macros::text;
        let update_instruction = if let Some(update_action) = self.update_action {
            line!["Run ", update_action.command_str().cyan(), " to update."]
        } else {
            line![
                "See ",
                "https://github.com/openai/codex".cyan().underlined(),
                " for installation options."
            ]
        };

        let content = text![
            line![
                "✨".cyan().bold(),
                "Update available!".bold().cyan(),
                " ",
                format!("{CODEX_CLI_VERSION} -> {}", self.latest_version).bold(),
            ],
            update_instruction,
            "",
            "See full release notes:",
            "https://github.com/openai/codex/releases/latest"
                .cyan()
                .underlined(),
        ];

        let inner_width = content
            .width()
            .min(usize::from(width.saturating_sub(4)))
            .max(1);
        with_border_with_inner_width(content.lines, inner_width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn renders_with_update_action_command() {
        let cell =
            UpdateAvailableHistoryCell::new("1.2.3".to_string(), Some(UpdateAction::BrewUpgrade));

        let rendered = cell.display_string(80);

        assert!(rendered.contains("Update available!"));
        assert!(rendered.contains("Run brew upgrade codex to update."));
        assert!(
            rendered.starts_with('╭') && rendered.contains('╯'),
            "banner should be boxed"
        );
    }

    #[test]
    fn renders_link_when_no_action_available() {
        let cell = UpdateAvailableHistoryCell::new("2.0.0".to_string(), None);

        let rendered = cell.display_string(80);

        assert!(rendered.contains("codex/releases/latest"));
        assert!(rendered.contains("See https://github.com/openai/codex"));
    }

    #[test]
    fn update_action_box_wraps() {
        let cell =
            UpdateAvailableHistoryCell::new("2.3.4".to_string(), Some(UpdateAction::BrewUpgrade));

        assert_snapshot!(cell.display_string(48));
    }

    #[test]
    fn update_link_box_wraps() {
        let cell = UpdateAvailableHistoryCell::new("2.3.4".to_string(), None);

        assert_snapshot!(cell.display_string(52));
    }
}
