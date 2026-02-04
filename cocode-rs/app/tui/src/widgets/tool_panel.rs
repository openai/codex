//! Tool execution panel widget.
//!
//! Displays currently running and recently completed tools.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::List;
use ratatui::widgets::ListItem;
use ratatui::widgets::Widget;

use crate::i18n::t;
use crate::state::ToolExecution;
use crate::state::ToolStatus;

/// Tool panel widget showing tool execution status.
pub struct ToolPanel<'a> {
    tools: &'a [ToolExecution],
    max_display: usize,
}

impl<'a> ToolPanel<'a> {
    /// Create a new tool panel.
    pub fn new(tools: &'a [ToolExecution]) -> Self {
        Self {
            tools,
            max_display: 5,
        }
    }

    /// Set the maximum number of tools to display.
    pub fn max_display(mut self, max: usize) -> Self {
        self.max_display = max;
        self
    }

    /// Format a tool for display.
    fn format_tool(tool: &ToolExecution) -> ListItem<'static> {
        let status_icon = match tool.status {
            ToolStatus::Running => Span::raw("⏳").yellow(),
            ToolStatus::Completed => Span::raw("✓").green(),
            ToolStatus::Failed => Span::raw("✗").red(),
        };

        let name = Span::raw(format!(" {}", tool.name));

        let progress = tool
            .progress
            .as_ref()
            .map(|p| Span::raw(format!(" - {p}")).dim())
            .unwrap_or_else(|| Span::raw(""));

        let line = Line::from(vec![status_icon, name, progress]);
        ListItem::new(line)
    }
}

impl Widget for ToolPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 || self.tools.is_empty() {
            return;
        }

        // Take the most recent tools
        let display_tools: Vec<_> = self.tools.iter().rev().take(self.max_display).collect();

        let items: Vec<ListItem> = display_tools
            .iter()
            .rev()
            .map(|t| Self::format_tool(t))
            .collect();

        let running_count = self
            .tools
            .iter()
            .filter(|t| t.status == ToolStatus::Running)
            .count();

        let title = if running_count > 0 {
            format!(" {} ", t!("tool.title_running", count = running_count))
        } else {
            format!(" {} ", t!("tool.title"))
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(ratatui::style::Style::default().dim())
            .title(title);

        let list = List::new(items).block(block);

        list.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tool(name: &str, status: ToolStatus) -> ToolExecution {
        ToolExecution {
            call_id: format!("call-{name}"),
            name: name.to_string(),
            status,
            progress: None,
            output: None,
        }
    }

    #[test]
    fn test_tool_panel_empty() {
        let tools: Vec<ToolExecution> = vec![];
        let panel = ToolPanel::new(&tools);

        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        panel.render(area, &mut buf);

        // Should render nothing (empty tools)
    }

    #[test]
    fn test_tool_panel_with_tools() {
        let tools = vec![
            make_tool("bash", ToolStatus::Running),
            make_tool("read", ToolStatus::Completed),
        ];
        let panel = ToolPanel::new(&tools);

        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        panel.render(area, &mut buf);

        let content: String = buf.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("bash"));
        assert!(content.contains("read"));
    }

    #[test]
    fn test_format_tool_running() {
        let tool = make_tool("test", ToolStatus::Running);
        let _item = ToolPanel::format_tool(&tool);
        // Item should be created successfully
    }

    #[test]
    fn test_max_display() {
        let tools: Vec<_> = (0..10)
            .map(|i| make_tool(&format!("tool-{i}"), ToolStatus::Completed))
            .collect();
        let panel = ToolPanel::new(&tools).max_display(3);

        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        panel.render(area, &mut buf);

        // Should only show 3 most recent tools
    }
}
