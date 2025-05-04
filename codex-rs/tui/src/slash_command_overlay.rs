use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

use crate::slash_commands::{COMMANDS, CommandInfo};

pub struct SlashCommandOverlay<'a> {
    pub filter: &'a str,
    pub selected: usize,
    pub scroll_offset: usize,
    pub max_height: usize, // includes borders
}

impl<'a> SlashCommandOverlay<'a> {
    pub fn filtered_commands(&self) -> Vec<&'static CommandInfo> {
        // Treat only-whitespace filter as empty, and also if filter is all spaces
        if self.filter.chars().all(|c| c.is_whitespace()) {
            COMMANDS.iter().collect()
        } else {
            let filter = self.filter.trim().to_ascii_lowercase();
            if filter.is_empty() {
                COMMANDS.iter().collect()
            } else {
                COMMANDS
                    .iter()
                    .filter(|cmd| cmd.name[1..].to_ascii_lowercase().starts_with(&filter))
                    .collect()
            }
        }
    }

    pub fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
        let mut lines = Vec::new();
        let mut current = String::new();
        for word in text.split_whitespace() {
            if current.len() + word.len() + 1 > max_width {
                if !current.is_empty() {
                    lines.push(current.clone());
                    current.clear();
                }
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

impl<'a> Widget for SlashCommandOverlay<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let commands = self.filtered_commands();
        if commands.is_empty() {
            // Do not render anything if there are no commands
            return;
        }
        let max_lines = self.max_height as usize;
        let chevron_width = 2; // chevron + space
        let cmd_max_len = crate::slash_commands::COMMANDS.iter().map(|c| c.name.len()).max().unwrap_or(0);
        let desc_start_x = area.x + 1 + chevron_width as u16 + cmd_max_len as u16 + 1; // chevron + space + cmd + space
        let desc_width = (area.x + area.width).saturating_sub(desc_start_x) as usize;

        // Compute how many lines each command will take
        let mut command_line_counts = Vec::with_capacity(commands.len());
        for cmd in &commands {
            let wrapped_desc = Self::wrap_text(&cmd.description, desc_width);
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
        while self.selected >= last_visible {
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
        while self.selected < first_visible {
            if first_visible == 0 { break; }
            first_visible -= 1;
            lines_used += command_line_counts[first_visible];
            while lines_used > max_lines {
                lines_used -= command_line_counts[last_visible - 1];
                last_visible -= 1;
            }
        }

        // Render the visible window
        let mut y = area.y;
        for idx in first_visible..last_visible {
            if y >= area.y + area.height { break; }
            let cmd = &commands[idx];
            let wrapped_desc = Self::wrap_text(&cmd.description, desc_width);
            let is_selected = idx == self.selected;
            let chevron = if is_selected { "❭" } else { " " };
            let chevron_style = if is_selected {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let cmd_style = if is_selected {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Blue)
            };
            let desc_style = Style::default().fg(Color::Gray);
            // Render: chevron, command, space, description (first line)
            let mut x = area.x + 1;
            buf.set_string(x, y, chevron, chevron_style);
            x += chevron_width as u16;
            buf.set_string(x, y, &cmd.name, cmd_style);
            x += cmd.name.len() as u16 + 1;
            if let Some(first_desc) = wrapped_desc.get(0) {
                buf.set_string(x, y, first_desc.trim_start(), desc_style);
            }
            y += 1;
            // Render any additional wrapped lines, aligned with the first description line (no extra indentation)
            for desc_line in wrapped_desc.iter().skip(1) {
                if y >= area.y + area.height { break; }
                buf.set_string(x, y, desc_line.trim_start(), desc_style);
                y += 1;
            }
        }
    }
} 