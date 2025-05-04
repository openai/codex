use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};

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
        let block = Block::default()
            .title("Available Commands")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));
        block.render(area, buf);
        if commands.is_empty() {
            let msg = "No commands found";
            let y = area.y + 1;
            buf.set_string(area.x + 2, y, msg, Style::default().fg(Color::Red));
            return;
        }
        let max_lines = self.max_height.saturating_sub(2) as usize; // borders
        let desc_indent = 6;
        let desc_width = area.width.saturating_sub(desc_indent as u16 + 2) as usize;
        let mut rendered_cmds = Vec::new();
        for cmd in &commands {
            let desc_lines = Self::wrap_text(&cmd.description, desc_width);
            rendered_cmds.push((cmd, desc_lines));
        }
        let mut lines_used = 0;
        let mut visible = Vec::new();
        let mut idx = self.scroll_offset;
        while idx < rendered_cmds.len() && lines_used < max_lines {
            let (cmd, desc_lines) = &rendered_cmds[idx];
            let needed = 1 + desc_lines.len();
            if lines_used + needed > max_lines {
                break;
            }
            visible.push((idx, cmd, desc_lines));
            lines_used += needed;
            idx += 1;
        }
        let mut y = area.y + 1;
        for (idx, cmd, desc_lines) in visible {
            if y >= area.y + area.height - 1 { break; }
            let is_selected = idx == self.selected;
            let chevron = if is_selected { "❭" } else { " " };
            let chevron_style = if is_selected {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let cmd_style = if is_selected {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default().fg(Color::Blue)
            };
            // Chevron
            buf.set_string(area.x + 1, y, chevron, chevron_style);
            // Command name
            buf.set_string(area.x + 3, y, &cmd.name, cmd_style);
            y += 1;
            // Description rows, indented
            let desc_style = Style::default().fg(Color::Gray);
            for line in desc_lines {
                if y >= area.y + area.height - 1 { break; }
                buf.set_string(area.x + desc_indent, y, &line, desc_style);
                y += 1;
            }
        }
    }
} 