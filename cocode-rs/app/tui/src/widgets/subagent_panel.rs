//! Subagent status panel widget.
//!
//! Displays active subagents with their status and progress.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Widget;

use crate::i18n::t;
use crate::state::SubagentInstance;
use crate::state::SubagentStatus;

/// Subagent panel widget.
///
/// Displays a list of active subagents with their status and progress.
pub struct SubagentPanel<'a> {
    subagents: &'a [SubagentInstance],
    max_display: i32,
}

impl<'a> SubagentPanel<'a> {
    /// Create a new subagent panel.
    pub fn new(subagents: &'a [SubagentInstance]) -> Self {
        Self {
            subagents,
            max_display: 5,
        }
    }

    /// Set the maximum number of subagents to display.
    pub fn max_display(mut self, max: i32) -> Self {
        self.max_display = max;
        self
    }
}

impl Widget for SubagentPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 3 || area.width < 10 {
            return;
        }

        // Create border
        let block = Block::default()
            .title(format!(" {} ", t!("subagent.title")).bold())
            .borders(Borders::ALL)
            .border_style(Style::default().cyan());

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 1 || self.subagents.is_empty() {
            return;
        }

        // Render subagents
        let mut y = inner.y;
        for subagent in self.subagents.iter().take(self.max_display as usize) {
            if y >= inner.y + inner.height {
                break;
            }

            // Status icon
            let (icon, style) = match subagent.status {
                SubagentStatus::Running => ("⚙", Style::default().yellow()),
                SubagentStatus::Completed => ("✓", Style::default().green()),
                SubagentStatus::Failed => ("✗", Style::default().red()),
                SubagentStatus::Backgrounded => ("◐", Style::default().blue()),
            };

            // Format: "icon type: description"
            let type_str = &subagent.agent_type;
            let desc_str = &subagent.description;

            // Render icon
            buf.set_string(inner.x, y, icon, style);

            // Render agent type
            let type_x = inner.x + 2;
            let type_width = type_str.len().min((inner.width as usize).saturating_sub(3));
            buf.set_string(type_x, y, &type_str[..type_width], style.bold());

            // Render colon
            let colon_x = type_x + type_width as u16;
            if colon_x < inner.x + inner.width - 1 {
                buf.set_string(colon_x, y, ": ", Style::default().dim());
            }

            // Render description (truncated if needed)
            let desc_x = colon_x + 2;
            if desc_x < inner.x + inner.width - 1 {
                let available = (inner.x + inner.width - desc_x) as usize;
                let desc = if desc_str.len() > available {
                    format!("{}...", &desc_str[..available.saturating_sub(3)])
                } else {
                    desc_str.clone()
                };
                buf.set_string(desc_x, y, desc, Style::default());
            }

            y += 1;

            // Render progress on next line if available
            if let Some(ref progress) = subagent.progress {
                if y < inner.y + inner.height {
                    let progress_str = if let (Some(current), Some(total)) =
                        (progress.current_step, progress.total_steps)
                    {
                        format!(
                            "  {}",
                            t!("subagent.step_progress", current = current, total = total)
                        )
                    } else if let Some(ref msg) = progress.message {
                        format!("  {}", msg)
                    } else {
                        String::new()
                    };

                    if !progress_str.is_empty() {
                        let available = inner.width as usize;
                        let text = if progress_str.len() > available {
                            format!("{}...", &progress_str[..available.saturating_sub(3)])
                        } else {
                            progress_str
                        };
                        buf.set_string(inner.x, y, text, Style::default().dim());
                        y += 1;
                    }
                }
            }
        }

        // Show count if more items exist
        if self.subagents.len() > self.max_display as usize {
            if y < inner.y + inner.height {
                let remaining = self.subagents.len() - self.max_display as usize;
                let text = format!("  {}", t!("subagent.more", count = remaining));
                buf.set_string(inner.x, y, text, Style::default().dim());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cocode_protocol::AgentProgress;

    fn create_test_subagents() -> Vec<SubagentInstance> {
        vec![
            SubagentInstance {
                id: "agent-1".to_string(),
                agent_type: "Explore".to_string(),
                description: "Searching for API endpoints".to_string(),
                status: SubagentStatus::Running,
                progress: Some(AgentProgress {
                    message: Some("Reading files...".to_string()),
                    current_step: Some(2),
                    total_steps: Some(5),
                }),
                result: None,
                output_file: None,
            },
            SubagentInstance {
                id: "agent-2".to_string(),
                agent_type: "Plan".to_string(),
                description: "Creating implementation plan".to_string(),
                status: SubagentStatus::Completed,
                progress: None,
                result: Some("Plan created".to_string()),
                output_file: None,
            },
        ]
    }

    #[test]
    fn test_panel_creation() {
        let subagents = create_test_subagents();
        let panel = SubagentPanel::new(&subagents);

        let area = Rect::new(0, 0, 50, 10);
        let mut buf = Buffer::empty(area);

        panel.render(area, &mut buf);

        let content: String = buf.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("Subagents"));
    }

    #[test]
    fn test_empty_panel() {
        let subagents: Vec<SubagentInstance> = vec![];
        let panel = SubagentPanel::new(&subagents);

        let area = Rect::new(0, 0, 50, 10);
        let mut buf = Buffer::empty(area);

        panel.render(area, &mut buf);

        // Should still render the border
        let content: String = buf.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("Subagents"));
    }

    #[test]
    fn test_max_display() {
        let mut subagents = create_test_subagents();
        // Add more subagents
        for i in 3..10 {
            subagents.push(SubagentInstance {
                id: format!("agent-{}", i),
                agent_type: "Test".to_string(),
                description: format!("Test agent {}", i),
                status: SubagentStatus::Running,
                progress: None,
                result: None,
                output_file: None,
            });
        }

        let panel = SubagentPanel::new(&subagents).max_display(3);

        let area = Rect::new(0, 0, 50, 15);
        let mut buf = Buffer::empty(area);

        panel.render(area, &mut buf);

        let content: String = buf.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("more"));
    }
}
