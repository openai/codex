use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::render::renderable::Renderable;
use crate::text_formatting::truncate_text;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;

const MAX_SESSION_LABEL_GRAPHEMES: usize = 48;
const MAX_VISIBLE_SESSIONS: usize = 2;

pub(crate) struct UnifiedExecFooter {
    sessions: Vec<String>,
}

impl UnifiedExecFooter {
    pub(crate) fn new() -> Self {
        Self {
            sessions: Vec::new(),
        }
    }

    pub(crate) fn set_sessions(&mut self, sessions: Vec<String>) -> bool {
        if self.sessions == sessions {
            return false;
        }
        self.sessions = sessions;
        true
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    fn render_lines(&self, width: u16) -> Vec<Line<'static>> {
        if self.sessions.is_empty() || width < 4 {
            return Vec::new();
        }

        let label = "Background terminal running:";
        let mut spans = Vec::new();
        spans.push(label.dim());
        spans.push(" ".into());

        let visible = self.sessions.iter().take(MAX_VISIBLE_SESSIONS);
        let mut visible_count = 0usize;
        for (idx, command) in visible.enumerate() {
            if idx > 0 {
                spans.push(" · ".dim());
            }
            let truncated = truncate_text(command, MAX_SESSION_LABEL_GRAPHEMES);
            spans.push(truncated.cyan());
            visible_count += 1;
        }

        let remaining = self.sessions.len().saturating_sub(visible_count);
        if remaining > 0 {
            spans.push(" · ".dim());
            spans.push(format!("{remaining} more running").dim());
        }

        let indent = " ".repeat(label.len() + 1);
        let line = Line::from(spans);
        word_wrap_lines(
            std::iter::once(line),
            RtOptions::new(width as usize).subsequent_indent(Line::from(indent).dim()),
        )
    }
}

impl Renderable for UnifiedExecFooter {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        Paragraph::new(self.render_lines(area.width)).render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.render_lines(width).len() as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;

    #[test]
    fn desired_height_empty() {
        let footer = UnifiedExecFooter::new();
        assert_eq!(footer.desired_height(40), 0);
    }

    #[test]
    fn render_two_sessions() {
        let mut footer = UnifiedExecFooter::new();
        footer.set_sessions(vec!["echo hello".to_string(), "rg \"foo\" src".to_string()]);
        let width = 50;
        let height = footer.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        footer.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_two_sessions", format!("{buf:?}"));
    }

    #[test]
    fn render_more_sessions() {
        let mut footer = UnifiedExecFooter::new();
        footer.set_sessions(vec![
            "echo hello".to_string(),
            "rg \"foo\" src".to_string(),
            "cat README.md".to_string(),
        ]);
        let width = 50;
        let height = footer.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        footer.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_more_sessions", format!("{buf:?}"));
    }
}
