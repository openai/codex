use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::symbols::border::QUADRANT_LEFT_HALF;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Cell;
use ratatui::widgets::Row;
use ratatui::widgets::Table;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;

use crate::slash_command::SlashCommand;
use crate::slash_command::built_in_slash_commands;

const MAX_POPUP_ROWS: usize = 5;
/// Ideally this is enough to show the longest command name.
const FIRST_COLUMN_WIDTH: u16 = 20;

use ratatui::style::Modifier;

pub(crate) struct CommandPopup {
    command_filter: String,
    all_commands: Vec<(&'static str, SlashCommand)>,
    selected_idx: Option<usize>,
    // Index of the first visible row in the filtered list.
    scroll_top: usize,
}

impl CommandPopup {
    pub(crate) fn new() -> Self {
        Self {
            command_filter: String::new(),
            all_commands: built_in_slash_commands(),
            selected_idx: None,
            scroll_top: 0,
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

        self.adjust_scroll(matches_len);
    }

    /// Determine the preferred height of the popup. This is the number of
    /// rows required to show at most MAX_POPUP_ROWS commands.
    pub(crate) fn calculate_required_height(&self) -> u16 {
        self.filtered_commands().len().clamp(1, MAX_POPUP_ROWS) as u16
    }

    /// Return the list of commands that match the current filter. Matching is
    /// performed using a case-insensitive prefix comparison on the command name.
    fn filtered_commands(&self) -> Vec<&SlashCommand> {
        let filter = self.command_filter.as_str();
        self.all_commands
            .iter()
            .filter_map(|(_name, cmd)| {
                if filter.is_empty() {
                    return Some(cmd);
                }
                let name = cmd.command();
                if name.len() >= filter.len() && name[..filter.len()].eq_ignore_ascii_case(filter) {
                    Some(cmd)
                } else {
                    None
                }
            })
            .collect::<Vec<&SlashCommand>>()
    }

    /// Move the selection cursor one step up.
    pub(crate) fn move_up(&mut self) {
        let matches = self.filtered_commands();
        let len = matches.len();
        if len == 0 {
            self.selected_idx = None;
            self.scroll_top = 0;
            return;
        }

        match self.selected_idx {
            Some(idx) if idx > 0 => self.selected_idx = Some(idx - 1),
            Some(_) => self.selected_idx = Some(len - 1), // wrap to last
            None => self.selected_idx = Some(0),
        }

        self.adjust_scroll(len);
    }

    /// Move the selection cursor one step down.
    pub(crate) fn move_down(&mut self) {
        let matches = self.filtered_commands();
        let matches_len = matches.len();
        if matches_len == 0 {
            self.selected_idx = None;
            self.scroll_top = 0;
            return;
        }

        match self.selected_idx {
            Some(idx) if idx + 1 < matches_len => {
                self.selected_idx = Some(idx + 1);
            }
            Some(_idx_last) => {
                self.selected_idx = Some(0);
            }
            None => {
                self.selected_idx = Some(0);
            }
        }

        self.adjust_scroll(matches_len);
    }

    /// Return currently selected command, if any.
    pub(crate) fn selected_command(&self) -> Option<&SlashCommand> {
        let matches = self.filtered_commands();
        self.selected_idx.and_then(|idx| matches.get(idx).copied())
    }

    fn adjust_scroll(&mut self, matches_len: usize) {
        if matches_len == 0 {
            self.scroll_top = 0;
            return;
        }
        let visible_rows = MAX_POPUP_ROWS.min(matches_len);
        if let Some(sel) = self.selected_idx {
            if sel < self.scroll_top {
                self.scroll_top = sel;
            } else {
                let bottom = self.scroll_top + visible_rows - 1;
                if sel > bottom {
                    self.scroll_top = sel + 1 - visible_rows;
                }
            }
        } else {
            self.scroll_top = 0;
        }
    }
}

impl WidgetRef for CommandPopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let matches = self.filtered_commands();

        let mut rows: Vec<Row> = Vec::new();

        if matches.is_empty() {
            rows.push(Row::new(vec![
                Cell::from(""),
                Cell::from("No matching commands").add_modifier(Modifier::ITALIC),
            ]));
        } else {
            let default_style = Style::default();
            let command_style = Style::default().fg(Color::LightBlue);
            let max_rows_from_area = area.height as usize;
            let visible_rows = MAX_POPUP_ROWS
                .min(matches.len())
                .min(max_rows_from_area.max(1));

            let mut start_idx = self.scroll_top.min(matches.len().saturating_sub(1));
            if let Some(sel) = self.selected_idx {
                if sel < start_idx {
                    start_idx = sel;
                } else if visible_rows > 0 {
                    let bottom = start_idx + visible_rows - 1;
                    if sel > bottom {
                        start_idx = sel + 1 - visible_rows;
                    }
                }
            }

            for (global_idx, cmd) in matches
                .iter()
                .enumerate()
                .skip(start_idx)
                .take(visible_rows)
            {
                rows.push(Row::new(vec![
                    Cell::from(Line::from(vec![
                        if Some(global_idx) == self.selected_idx {
                            Span::styled(
                                "›",
                                Style::default().bg(Color::DarkGray).fg(Color::LightCyan),
                            )
                        } else {
                            Span::styled(QUADRANT_LEFT_HALF, Style::default().fg(Color::DarkGray))
                        },
                        Span::styled(format!("/{}", cmd.command()), command_style),
                    ])),
                    Cell::from(cmd.description().to_string()).style(default_style),
                ]));
            }
        }

        use ratatui::layout::Constraint;

        let table = Table::new(
            rows,
            [Constraint::Length(FIRST_COLUMN_WIDTH), Constraint::Min(10)],
        )
        .column_spacing(0);
        // .block(
        //     Block::default()
        //         .borders(Borders::LEFT)
        //         .border_type(BorderType::QuadrantOutside)
        //         .border_style(Style::default().fg(Color::DarkGray)),
        // );

        table.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    fn move_down_wraps_to_top() {
        let mut popup = CommandPopup::new();
        // Show all commands by simulating composer input starting with '/'.
        popup.on_composer_text_change("/".to_string());
        let len = popup.filtered_commands().len();
        assert!(len > 0);

        // Move to last item.
        for _ in 0..len.saturating_sub(1) {
            popup.move_down();
        }
        // Next move_down should wrap to index 0.
        popup.move_down();
        assert_eq!(popup.selected_idx, Some(0));
    }

    #[test]
    fn move_up_wraps_to_bottom() {
        let mut popup = CommandPopup::new();
        popup.on_composer_text_change("/".to_string());
        let len = popup.filtered_commands().len();
        assert!(len > 0);

        // Initial selection is 0; moving up should wrap to last.
        popup.move_up();
        assert_eq!(popup.selected_idx, Some(len - 1));
    }

    #[test]
    fn respects_tiny_terminal_height_when_rendering() {
        let mut popup = CommandPopup::new();
        popup.on_composer_text_change("/".to_string());
        assert!(popup.filtered_commands().len() >= 3);

        let area = Rect::new(0, 0, 50, 2);
        let mut buf = Buffer::empty(area);
        popup.render(area, &mut buf);

        let mut non_empty_rows = 0u16;
        for y in 0..area.height {
            let mut row_has_content = false;
            for x in 0..area.width {
                let c = buf[(x, y)].symbol();
                if !c.trim().is_empty() {
                    row_has_content = true;
                    break;
                }
            }
            if row_has_content {
                non_empty_rows += 1;
            }
        }

        assert_eq!(non_empty_rows, 2);
    }
}
