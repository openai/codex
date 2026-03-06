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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PendingPreviewMessage {
    pub(crate) text: String,
    pub(crate) repeating: bool,
    pub(crate) steer: bool,
}

/// Widget that displays pending steers plus user messages queued while a turn is in progress.
///
/// The widget shows pending steers first, then queued user messages. It only
/// shows the edit hint at the bottom (e.g. "⌥ + ↑ edit") when there are actual
/// queued user messages to pop back into the composer. Because some terminals
/// intercept certain modifier-key combinations, the displayed binding is
/// configurable via [`set_edit_binding`](Self::set_edit_binding).
pub(crate) struct PendingInputPreview {
    pub pending_steers: Vec<PendingPreviewMessage>,
    pub queued_messages: Vec<PendingPreviewMessage>,
    /// Key combination rendered in the hint line.  Defaults to Alt+Up but may
    /// be overridden for terminals where that chord is unavailable.
    edit_binding: key_hint::KeyBinding,
}

const PREVIEW_LINE_LIMIT: usize = 3;

impl PendingInputPreview {
    pub(crate) fn new() -> Self {
        Self {
            pending_steers: Vec::new(),
            queued_messages: Vec::new(),
            edit_binding: key_hint::alt(KeyCode::Up),
        }
    }

    /// Replace the keybinding shown in the hint line at the bottom of the
    /// queued-messages list.  The caller is responsible for also wiring the
    /// corresponding key event handler.
    pub(crate) fn set_edit_binding(&mut self, binding: key_hint::KeyBinding) {
        self.edit_binding = binding;
    }

    fn push_truncated_preview_lines(
        lines: &mut Vec<Line<'static>>,
        wrapped: Vec<Line<'static>>,
        overflow_line: Line<'static>,
    ) {
        let wrapped_len = wrapped.len();
        lines.extend(wrapped.into_iter().take(PREVIEW_LINE_LIMIT));
        if wrapped_len > PREVIEW_LINE_LIMIT {
            lines.push(overflow_line);
        }
    }

    fn as_renderable(&self, width: u16) -> Box<dyn Renderable> {
        if (self.pending_steers.is_empty() && self.queued_messages.is_empty()) || width < 4 {
            return Box::new(());
        }

        let mut lines = vec![];

        for steer in &self.pending_steers {
            let wrapped = if steer.repeating {
                adaptive_wrap_lines(
                    steer.text.lines().map(|line| Line::from(line.cyan())),
                    RtOptions::new(width as usize)
                        .initial_indent(Line::from("  ! repeat steer: ".cyan()))
                        .subsequent_indent(Line::from("    ")),
                )
            } else {
                adaptive_wrap_lines(
                    steer.text.lines().map(|line| Line::from(line.blue())),
                    RtOptions::new(width as usize)
                        .initial_indent(Line::from("  ! pending steer: ".blue()))
                        .subsequent_indent(Line::from("    ")),
                )
            };
            let overflow_line = if steer.repeating {
                Line::from("    …".cyan())
            } else {
                Line::from("    …".blue())
            };
            Self::push_truncated_preview_lines(&mut lines, wrapped, overflow_line);
        }

        for message in &self.queued_messages {
            let (wrapped, overflow_line) = match (message.steer, message.repeating) {
                (true, true) => (
                    adaptive_wrap_lines(
                        message
                            .text
                            .lines()
                            .map(|line| Line::from(line.cyan().italic())),
                        RtOptions::new(width as usize)
                            .initial_indent(Line::from("  ↻ ! ".cyan()))
                            .subsequent_indent(Line::from("    ")),
                    ),
                    Line::from("    …".cyan().italic()),
                ),
                (true, false) => (
                    adaptive_wrap_lines(
                        message
                            .text
                            .lines()
                            .map(|line| Line::from(line.blue().italic())),
                        RtOptions::new(width as usize)
                            .initial_indent(Line::from("  ↳ ! ".blue()))
                            .subsequent_indent(Line::from("    ")),
                    ),
                    Line::from("    …".blue().italic()),
                ),
                (false, true) => (
                    adaptive_wrap_lines(
                        message
                            .text
                            .lines()
                            .map(|line| Line::from(line.yellow().italic())),
                        RtOptions::new(width as usize)
                            .initial_indent(Line::from("  ↻ ".yellow()))
                            .subsequent_indent(Line::from("    ")),
                    ),
                    Line::from("    …".yellow().italic()),
                ),
                (false, false) => (
                    adaptive_wrap_lines(
                        message
                            .text
                            .lines()
                            .map(|line| Line::from(line.green().italic())),
                        RtOptions::new(width as usize)
                            .initial_indent(Line::from("  ↳ ".green()))
                            .subsequent_indent(Line::from("    ")),
                    ),
                    Line::from("    …".green().italic()),
                ),
            };
            Self::push_truncated_preview_lines(&mut lines, wrapped, overflow_line);
        }

        if !self.queued_messages.is_empty() {
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

    fn normal(text: &str) -> PendingPreviewMessage {
        PendingPreviewMessage {
            text: text.to_string(),
            repeating: false,
            steer: false,
        }
    }

    fn repeating(text: &str) -> PendingPreviewMessage {
        PendingPreviewMessage {
            text: text.to_string(),
            repeating: true,
            steer: false,
        }
    }

    fn steer_normal(text: &str) -> PendingPreviewMessage {
        PendingPreviewMessage {
            text: text.to_string(),
            repeating: false,
            steer: true,
        }
    }

    fn steer_repeating(text: &str) -> PendingPreviewMessage {
        PendingPreviewMessage {
            text: text.to_string(),
            repeating: true,
            steer: true,
        }
    }

    #[test]
    fn desired_height_empty() {
        let queue = PendingInputPreview::new();
        assert_eq!(queue.desired_height(40), 0);
    }

    #[test]
    fn desired_height_one_message() {
        let mut queue = PendingInputPreview::new();
        queue.queued_messages.push(normal("Hello, world!"));
        assert_eq!(queue.desired_height(40), 2);
    }

    #[test]
    fn render_one_message() {
        let mut queue = PendingInputPreview::new();
        queue.queued_messages.push(normal("Hello, world!"));
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_one_message", format!("{buf:?}"));
    }

    #[test]
    fn render_two_messages() {
        let mut queue = PendingInputPreview::new();
        queue.queued_messages.push(normal("Hello, world!"));
        queue
            .queued_messages
            .push(normal("This is another message"));
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_two_messages", format!("{buf:?}"));
    }

    #[test]
    fn render_more_than_three_messages() {
        let mut queue = PendingInputPreview::new();
        queue.queued_messages.push(normal("Hello, world!"));
        queue
            .queued_messages
            .push(normal("This is another message"));
        queue
            .queued_messages
            .push(normal("This is a third message"));
        queue
            .queued_messages
            .push(normal("This is a fourth message"));
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
            .queued_messages
            .push(normal("This is a longer message that should be wrapped"));
        queue
            .queued_messages
            .push(normal("This is another message"));
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
            .queued_messages
            .push(normal("This is\na message\nwith many\nlines"));
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_many_line_message", format!("{buf:?}"));
    }

    #[test]
    fn long_url_like_message_does_not_expand_into_wrapped_ellipsis_rows() {
        let mut queue = PendingInputPreview::new();
        queue.queued_messages.push(
            normal(
                "example.test/api/v1/projects/alpha-team/releases/2026-02-17/builds/1234567890/artifacts/reports/performance/summary/detail/session_id=abc123def456ghi789",
            ),
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
    fn render_one_pending_steer() {
        let mut queue = PendingInputPreview::new();
        queue.pending_steers.push(steer_normal("Please continue."));
        let width = 48;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_one_pending_steer", format!("{buf:?}"));
    }

    #[test]
    fn render_pending_steers_above_queued_messages() {
        let mut queue = PendingInputPreview::new();
        queue.pending_steers.push(steer_normal("Please continue."));
        queue
            .pending_steers
            .push(steer_repeating("Check the last command output."));
        queue
            .queued_messages
            .push(repeating("Queued follow-up question"));
        let width = 52;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!(
            "render_pending_steers_above_queued_messages",
            format!("{buf:?}")
        );
    }

    #[test]
    fn render_multiline_pending_steer_uses_single_prefix_and_truncates() {
        let mut queue = PendingInputPreview::new();
        queue.pending_steers.push(steer_repeating(
            "First line\nSecond line\nThird line\nFourth line",
        ));
        let width = 48;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!(
            "render_multiline_pending_steer_uses_single_prefix_and_truncates",
            format!("{buf:?}")
        );
    }
}
