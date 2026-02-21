use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::key_hint;
use crate::render::renderable::Renderable;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;

/// Widget that displays a list of user messages queued while a turn is in progress.
///
/// The widget shows a key hint at the bottom (e.g. "⌥ + ↑ edit") telling the
/// user how to pop the most recent queued message back into the composer.
/// Because some terminals intercept certain modifier-key combinations, the
/// displayed binding is configurable via [`set_edit_binding`](Self::set_edit_binding).
pub(crate) struct QueuedUserMessages {
    pub messages: Vec<String>,
    /// Key combination rendered in the hint line.  Defaults to Alt+Up but may
    /// be overridden for terminals where that chord is unavailable.
    edit_binding: key_hint::KeyBinding,
}

impl QueuedUserMessages {
    pub(crate) fn new() -> Self {
        Self {
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
        if self.messages.is_empty() || width < 4 {
            return Box::new(());
        }

        let mut lines = vec![];

        for message in &self.messages {
            let wrapped = word_wrap_lines(
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

        lines.push(
            Line::from(vec![
                "    ".into(),
                self.edit_binding.into(),
                " edit".into(),
            ])
            .dim(),
        );

        Paragraph::new(lines).into()
    }
}

impl Renderable for QueuedUserMessages {
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
        let queue = QueuedUserMessages::new();
        assert_eq!(queue.desired_height(40), 0);
    }

    #[test]
    fn desired_height_one_message() {
        let mut queue = QueuedUserMessages::new();
        queue.messages.push("Hello, world!".to_string());
        assert_eq!(queue.desired_height(40), 2);
    }

    #[test]
    fn render_one_message() {
        let mut queue = QueuedUserMessages::new();
        queue.messages.push("Hello, world!".to_string());
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_one_message", format!("{buf:?}"));
    }

    #[test]
    fn render_two_messages() {
        let mut queue = QueuedUserMessages::new();
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
        let mut queue = QueuedUserMessages::new();
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
        let mut queue = QueuedUserMessages::new();
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
        let mut queue = QueuedUserMessages::new();
        queue
            .messages
            .push("This is\na message\nwith many\nlines".to_string());
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_many_line_message", format!("{buf:?}"));
    }
}
