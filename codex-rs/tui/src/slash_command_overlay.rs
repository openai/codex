use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

use crate::slash_commands::{CommandInfo, COMMANDS};

pub struct SlashCommandOverlay<'a> {
    pub filter: &'a str,
    pub selected: usize,
    pub scroll_offset: usize,
    pub max_height: usize, // includes borders
}

impl SlashCommandOverlay<'_> {
    /// Returns commands filtered and ranked by the filter string.
    pub fn filter_and_rank_commands(filter: &str) -> Vec<&'static CommandInfo> {
        let filter = filter.trim().to_ascii_lowercase();
        if filter.is_empty() {
            return COMMANDS.iter().collect();
        }
        let mut matches: Vec<_> = COMMANDS
            .iter()
            .filter(|cmd| {
                let name = &cmd.name[1..].to_ascii_lowercase(); // skip '/'
                name.starts_with(&filter)
            })
            .collect();
        // Sort alphabetically by name
        matches.sort_by(|a, b| a.name.cmp(b.name));
        matches
    }

    pub fn filtered_commands(&self) -> Vec<&'static CommandInfo> {
        Self::filter_and_rank_commands(self.filter)
    }

    pub fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
        let mut lines = Vec::new();
        let mut current = String::new();
        for word in text.split_whitespace() {
            if current.len() + word.len() + 1 > max_width && !current.is_empty() {
                lines.push(current.clone());
                current.clear();
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
        if !current.is_empty() {
            lines.push(current);
        }
        lines
    }

    pub fn overlay_height(&self) -> usize {
        let matches = self.filtered_commands().len();
        let max = self.max_height.min(12); // 12 or terminal height - 4
        matches.min(max).max(1)
    }
}

impl Widget for SlashCommandOverlay<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let commands = self.filtered_commands();
        if commands.is_empty() {
            // Do not render anything if there are no commands
            return;
        }
        // Clamp selected index to valid range
        let selected = self.selected.min(commands.len().saturating_sub(1));
        let max_lines = self.max_height;
        let chevron_width = 2; // chevron + space
        let cmd_max_len = crate::slash_commands::COMMANDS
            .iter()
            .map(|c| c.name.len())
            .max()
            .unwrap_or(0);
        let desc_start_x = area.x + 1 + chevron_width as u16 + cmd_max_len as u16 + 1; // chevron + space + cmd + space
        let desc_width = (area.x + area.width).saturating_sub(desc_start_x) as usize;

        // Compute how many lines each command will take
        let mut command_line_counts = Vec::with_capacity(commands.len());
        for cmd in &commands {
            let wrapped_desc = Self::wrap_text(cmd.description, desc_width);
            let needed = 1.max(wrapped_desc.len());
            command_line_counts.push(needed);
        }

        // Adjust scroll_offset so selected command is always fully visible
        let mut first_visible = self.scroll_offset;
        let mut lines_used = 0;
        // Find the window of commands that fits in max_lines and includes the selected command
        let mut last_visible = first_visible;
        while last_visible < commands.len() {
            let needed = command_line_counts[last_visible];
            if lines_used + needed > max_lines {
                break;
            }
            lines_used += needed;
            last_visible += 1;
        }
        // If selected is below the window, scroll down
        while selected < first_visible && first_visible > 0 {
            lines_used -= command_line_counts[first_visible];
            first_visible += 1;
            let mut temp_last = last_visible;
            while temp_last < commands.len() {
                let needed = command_line_counts[temp_last];
                if lines_used + needed > max_lines {
                    break;
                }
                lines_used += needed;
                temp_last += 1;
            }
            last_visible = temp_last;
        }
        // If selected is above the window, scroll up
        while selected < first_visible && first_visible > 0 {
            if first_visible == 0 {
                break;
            }
            first_visible -= 1;
            lines_used += command_line_counts[first_visible];
            while lines_used > max_lines && last_visible > 0 {
                lines_used -= command_line_counts[last_visible - 1];
                last_visible -= 1;
            }
        }

        // Render the visible window
        let mut y = area.y;
        for (idx, cmd) in commands
            .iter()
            .enumerate()
            .skip(first_visible)
            .take(last_visible - first_visible)
        {
            if y >= area.y + area.height {
                break;
            }
            let wrapped_desc = Self::wrap_text(cmd.description, desc_width);
            let is_selected = idx == selected;
            let chevron = if is_selected { "❭" } else { " " };
            let chevron_style = if is_selected {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let cmd_style = if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Blue)
            };
            let desc_style = Style::default().fg(Color::Gray);
            // Render: chevron, command, space, description (first line)
            let mut x = area.x + 1;
            buf.set_string(x, y, chevron, chevron_style);
            x += chevron_width as u16;
            buf.set_string(x, y, cmd.name, cmd_style);
            x += cmd.name.len() as u16 + 1;
            if let Some(first_desc) = wrapped_desc.first() {
                buf.set_string(x, y, first_desc.trim_start(), desc_style);
            }
            y += 1;
            // Render any additional wrapped lines, aligned with the first description line (no extra indentation)
            for desc_line in wrapped_desc.iter().skip(1) {
                if y >= area.y + area.height {
                    break;
                }
                buf.set_string(x, y, desc_line.trim_start(), desc_style);
                y += 1;
            }
        }
    }
}
