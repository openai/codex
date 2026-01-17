//! Renders the queue of user messages entered while a turn is still running.
//!
//! The queue is visual-only state: it shows what the user typed while the app
//! was busy and offers a hint for editing the most recent queued message. It
//! does not own submission behavior; it only wraps, truncates, and styles the
//! queued strings for display in the bottom pane.
//!
//! Correctness relies on preserving message order and on keeping the hint
//! aligned with the actual edit action (currently `Alt+Up`). The rendering path
//! never mutates queue contents; callers remain responsible for enqueueing and
//! dequeuing messages as turns begin or end.
//!
//! Each message is rendered with a dimmed gutter (`↳`), wrapped to the available
//! width, and truncated to three lines with an ellipsis line when additional
//! content exists. A final hint line is appended only when there is at least one
//! queued message.

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

/// Displays user messages queued while a turn is in progress.
///
/// The widget truncates each queued message to three wrapped lines, appending a
/// dimmed ellipsis when more content exists, then ends with a keyboard hint for
/// editing the latest queued message.
pub(crate) struct QueuedUserMessages {
    /// Ordered queued messages, oldest first.
    ///
    /// New messages are appended by the caller; the renderer preserves this
    /// ordering so users can see the queue in chronological sequence.
    pub messages: Vec<String>,
}

impl QueuedUserMessages {
    /// Creates an empty queue widget with no visible height.
    pub(crate) fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    /// Converts the queued messages into a renderable paragraph for a width.
    ///
    /// Returns an empty renderable when there is insufficient space to display
    /// the gutters or when there are no messages to show. Each message is
    /// wrapped with a two-character gutter, truncated to three lines, and
    /// followed by an ellipsis line when content was clipped.
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
                key_hint::alt(KeyCode::Up).into(),
                " edit".into(),
            ])
            .dim(),
        );

        Paragraph::new(lines).into()
    }
}

impl Renderable for QueuedUserMessages {
    /// Renders the queued messages paragraph into the given area.
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        self.as_renderable(area.width).render(area, buf);
    }

    /// Returns the desired height for the current queue at the given width.
    ///
    /// The height matches the wrapped, truncated paragraph returned by
    /// [`QueuedUserMessages::as_renderable`].
    fn desired_height(&self, width: u16) -> u16 {
        self.as_renderable(width).desired_height(width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;

    /// Verifies empty queues have no height.
    #[test]
    fn desired_height_empty() {
        let queue = QueuedUserMessages::new();
        assert_eq!(queue.desired_height(40), 0);
    }

    /// Ensures a single short message fits in two lines (message + hint).
    #[test]
    fn desired_height_one_message() {
        let mut queue = QueuedUserMessages::new();
        queue.messages.push("Hello, world!".to_string());
        assert_eq!(queue.desired_height(40), 2);
    }

    /// Snapshots the rendering for one queued message.
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

    /// Snapshots the rendering for two queued messages.
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

    /// Snapshots the rendering when more than three messages are queued.
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

    /// Snapshots wrapping behavior for a long queued message.
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

    /// Snapshots rendering of queued messages containing explicit newlines.
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
