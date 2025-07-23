use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::Borders;
use ratatui::widgets::Cell;
use ratatui::widgets::Row;
use ratatui::widgets::Table;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;

use crate::custom_slash_command::discover_custom_commands;
use crate::slash_command::SlashCommand;
use crate::slash_command::built_in_slash_commands;

/// Entry representing either a built-in or a custom slash command.
#[derive(Clone)]
pub(super) enum CommandEntry {
    BuiltIn(SlashCommand),
    Custom { name: String },
}

impl CommandEntry {
    pub(super) fn command(&self) -> &str {
        match self {
            CommandEntry::BuiltIn(cmd) => cmd.command(),
            CommandEntry::Custom { name } => name.as_str(),
        }
    }

    pub(super) fn description(&self) -> &str {
        match self {
            CommandEntry::BuiltIn(cmd) => cmd.description(),
            CommandEntry::Custom { .. } => "Custom command",
        }
    }
}

const MAX_POPUP_ROWS: usize = 5;
/// Ideally this is enough to show the longest command name.
const FIRST_COLUMN_WIDTH: u16 = 20;

use ratatui::style::Modifier;

pub(crate) struct CommandPopup {
    command_filter: String,
    all_commands: Vec<CommandEntry>,
    selected_idx: Option<usize>,
}

impl CommandPopup {
    pub(crate) fn new() -> Self {
        let mut all_commands: Vec<CommandEntry> = built_in_slash_commands()
            .into_iter()
            .map(|(_name, cmd)| CommandEntry::BuiltIn(cmd))
            .collect();

        // Discover custom commands on disk and append them to the list.
        for name in discover_custom_commands() {
            all_commands.push(CommandEntry::Custom { name });
        }

        Self {
            command_filter: String::new(),
            all_commands,
            selected_idx: None,
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
        self.selected_idx = match matches_len {
            0 => None,
            _ => Some(self.selected_idx.unwrap_or(0).min(matches_len - 1)),
        };
    }

    /// Determine the preferred height of the popup. This is the number of
    /// rows required to show **at most** `MAX_POPUP_ROWS` commands plus the
    /// table/border overhead (one line at the top and one at the bottom).
    pub(crate) fn calculate_required_height(&self, _area: &Rect) -> u16 {
        let matches = self.filtered_commands();
        let row_count = matches.len().clamp(1, MAX_POPUP_ROWS) as u16;
        // Account for the border added by the Block that wraps the table.
        // 2 = one line at the top, one at the bottom.
        row_count + 2
    }

    /// Return the list of commands that match the current filter. Matching is
    /// performed using a *prefix* comparison on the command name.
    fn filtered_commands(&self) -> Vec<&CommandEntry> {
        self.all_commands
            .iter()
            .filter(|cmd| {
                self.command_filter.is_empty()
                    || cmd
                        .command()
                        .starts_with(&self.command_filter.to_ascii_lowercase())
            })
            .collect()
    }

    /// Move the selection cursor one step up.
    pub(crate) fn move_up(&mut self) {
        if let Some(len) = self.filtered_commands().len().checked_sub(1) {
            if len == usize::MAX {
                return;
            }
        }

        if let Some(idx) = self.selected_idx {
            if idx > 0 {
                self.selected_idx = Some(idx - 1);
            }
        } else if !self.filtered_commands().is_empty() {
            self.selected_idx = Some(0);
        }
    }

    /// Move the selection cursor one step down.
    pub(crate) fn move_down(&mut self) {
        let matches_len = self.filtered_commands().len();
        if matches_len == 0 {
            self.selected_idx = None;
            return;
        }

        match self.selected_idx {
            Some(idx) if idx + 1 < matches_len => {
                self.selected_idx = Some(idx + 1);
            }
            None => {
                self.selected_idx = Some(0);
            }
            _ => {}
        }
    }

    /// Return currently selected command, if any.
    pub(crate) fn selected_command(&self) -> Option<&CommandEntry> {
        let matches = self.filtered_commands();
        self.selected_idx.and_then(|idx| matches.get(idx).copied())
    }
}

impl WidgetRef for CommandPopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let matches = self.filtered_commands();

        let mut rows: Vec<Row> = Vec::new();
        let visible_matches: Vec<&CommandEntry> =
            matches.into_iter().take(MAX_POPUP_ROWS).collect();

        if visible_matches.is_empty() {
            rows.push(Row::new(vec![
                Cell::from(""),
                Cell::from("No matching commands").add_modifier(Modifier::ITALIC),
            ]));
        } else {
            let default_style = Style::default();
            let command_style = Style::default().fg(Color::LightBlue);
            for (idx, cmd) in visible_matches.iter().enumerate() {
                let (cmd_style, desc_style) = if Some(idx) == self.selected_idx {
                    (
                        command_style.bg(Color::DarkGray),
                        default_style.bg(Color::DarkGray),
                    )
                } else {
                    (command_style, default_style)
                };

                rows.push(Row::new(vec![
                    Cell::from(format!("/{}", cmd.command())).style(cmd_style),
                    Cell::from(cmd.description().to_string()).style(desc_style),
                ]));
            }
        }

        use ratatui::layout::Constraint;

        let table = Table::new(
            rows,
            [Constraint::Length(FIRST_COLUMN_WIDTH), Constraint::Min(10)],
        )
        .column_spacing(0)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        );

        table.render(area, buf);
    }
}
