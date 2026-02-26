use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::key_hint;
use crate::render::renderable::Renderable;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_lines;

/// Widget that displays pending nudges plus user messages queued while a turn is in progress.
///
/// The widget shows pending nudges first, then queued user messages. It only
/// shows the edit hint at the bottom (e.g. "⌥ + ↑ edit") when there are actual
/// queued user messages to pop back into the composer. Because some terminals
/// intercept certain modifier-key combinations, the displayed binding is
/// configurable via [`set_edit_binding`](Self::set_edit_binding).
pub(crate) struct PendingInputPreview {
    pub pending_nudges: Vec<String>,
    pub messages: Vec<String>,
    /// Key combination rendered in the hint line.  Defaults to Alt+Up but may
    /// be overridden for terminals where that chord is unavailable.
    edit_binding: key_hint::KeyBinding,
}

impl PendingInputPreview {
    pub(crate) fn new() -> Self {
        Self {
            pending_nudges: Vec::new(),
            messages: Vec::new(),
            edit_binding: key_hint::alt(KeyCode::Up),
        }
    }

    /// Replace the keybinding shown in the hint line at the bottom of the
    /// queued-messages list.  The caller is responsible for also wiring the
    /// corresponding key event handler.
    pub(crate) fn set_edit_binding(&mut self, binding: key_hint::KeyBinding) {
        self.edit_binding = binding;
    }

    fn as_renderable(&self, width: u16) -> Box<dyn Renderable> {
        if (self.pending_nudges.is_empty() && self.messages.is_empty()) || width < 4 {
            return Box::new(());
        }

        let mut lines = vec![];

        for nudge in &self.pending_nudges {
            let wrapped = adaptive_wrap_lines(
                nudge
                    .lines()
                    .map(|line| format!("pending nudge: {line}").dim()),
                RtOptions::new(width as usize)
                    .initial_indent(Line::from("  ! ".dim()))
                    .subsequent_indent(Line::from("    ")),
            );
            let len = wrapped.len();
            for line in wrapped.into_iter().take(3) {
                lines.push(line);
            }
            if len > 3 {
                lines.push(Line::from("    …".dim()));
            }
        }

        for message in &self.messages {
            let wrapped = adaptive_wrap_lines(
                message.lines().map(|line| line.dim().italic()),
                RtOptions::new(width as usize)
                    .initial_indent(Line::from("  ↳ ".dim()))
                    .subsequent_indent(Line::from("    ")),
            );
            let len = wrapped.len();
            for line in wrapped.into_iter().take(3) {
                lines.push(line);
            }
            if len > 3 {
                lines.push(Line::from("    …".dim().italic()));
            }
        }

        if !self.messages.is_empty() {
            lines.push(
                Line::from(vec![
                    "    ".into(),
                    self.edit_binding.into(),
                    " edit".into(),
                ])
                .dim(),
            );
        }

        Paragraph::new(lines).into()
    }
}

impl Renderable for PendingInputPreview {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        self.as_renderable(area.width).render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.as_renderable(width).desired_height(width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;

    #[test]
    fn desired_height_empty() {
        let queue = PendingInputPreview::new();
        assert_eq!(queue.desired_height(40), 0);
    }

    #[test]
    fn desired_height_one_message() {
        let mut queue = PendingInputPreview::new();
        queue.messages.push("Hello, world!".to_string());
        assert_eq!(queue.desired_height(40), 2);
    }

    #[test]
    fn render_one_message() {
        let mut queue = PendingInputPreview::new();
        queue.messages.push("Hello, world!".to_string());
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_one_message", format!("{buf:?}"));
    }

    #[test]
    fn render_two_messages() {
        let mut queue = PendingInputPreview::new();
        queue.messages.push("Hello, world!".to_string());
        queue.messages.push("This is another message".to_string());
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_two_messages", format!("{buf:?}"));
    }

    #[test]
    fn render_more_than_three_messages() {
        let mut queue = PendingInputPreview::new();
        queue.messages.push("Hello, world!".to_string());
        queue.messages.push("This is another message".to_string());
        queue.messages.push("This is a third message".to_string());
        queue.messages.push("This is a fourth message".to_string());
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_more_than_three_messages", format!("{buf:?}"));
    }

    #[test]
    fn render_wrapped_message() {
        let mut queue = PendingInputPreview::new();
        queue
            .messages
            .push("This is a longer message that should be wrapped".to_string());
        queue.messages.push("This is another message".to_string());
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_wrapped_message", format!("{buf:?}"));
    }

    #[test]
    fn render_many_line_message() {
        let mut queue = PendingInputPreview::new();
        queue
            .messages
            .push("This is\na message\nwith many\nlines".to_string());
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_many_line_message", format!("{buf:?}"));
    }

    #[test]
    fn long_url_like_message_does_not_expand_into_wrapped_ellipsis_rows() {
        let mut queue = PendingInputPreview::new();
        queue.messages.push(
            "example.test/api/v1/projects/alpha-team/releases/2026-02-17/builds/1234567890/artifacts/reports/performance/summary/detail/session_id=abc123def456ghi789"
                .to_string(),
        );

        let width = 36;
        let height = queue.desired_height(width);
        assert_eq!(
            height, 2,
            "expected one message row plus hint row for URL-like token"
        );

        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);

        let rendered_rows = (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buf[(x, y)].symbol().chars().next().unwrap_or(' '))
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert!(
            !rendered_rows.iter().any(|row| row.contains('…')),
            "expected no wrapped-ellipsis row for URL-like token, got rows: {rendered_rows:?}"
        );
    }

    #[test]
    fn render_one_pending_nudge() {
        let mut queue = PendingInputPreview::new();
        queue.pending_nudges.push("Implement the plan.".to_string());
        let width = 48;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_one_pending_nudge", format!("{buf:?}"));
    }

    #[test]
    fn render_pending_nudges_above_queued_messages() {
        let mut queue = PendingInputPreview::new();
        queue.pending_nudges.push("Implement the plan.".to_string());
        queue
            .pending_nudges
            .push("Summarize the diff after the tool call.".to_string());
        queue.messages.push("Queued follow-up question".to_string());
        let width = 52;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!(
            "render_pending_nudges_above_queued_messages",
            format!("{buf:?}")
        );
    }
}
