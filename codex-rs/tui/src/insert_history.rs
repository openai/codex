use crate::tui;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthChar;

/// Insert a batch of history lines into the terminal scrollback above the
/// inline viewport.
///
/// The incoming `lines` are the logical lines supplied by the
/// `ConversationHistory`. They may contain embedded newlines and arbitrary
/// runs of whitespace inside individual [`Span`]s. All of that must be
/// normalised before writing to the backing terminal buffer because the
/// ratatui [`Paragraph`] widget does not perform soft‑wrapping when used in
/// conjunction with [`Terminal::insert_before`].
///
/// This function performs a minimal wrapping / normalisation pass:
///
/// * A terminal width is determined via `Terminal::size()` (falling back to
///   80 columns if the size probe fails).
/// * Each logical line is broken into words and whitespace. Consecutive
///   whitespace is collapsed to a single space; leading whitespace is
///   discarded.
/// * Words that do not fit on the current line cause a soft wrap. Extremely
///   long words (longer than the terminal width) are split character by
///   character so they still populate the display instead of overflowing the
///   line.
/// * Explicit `\n` characters inside a span force a hard line break.
/// * Empty lines (including a trailing newline at the end of the batch) are
///   preserved so vertical spacing remains faithful to the logical history.
///
/// Finally the physical lines are rendered directly into the terminal's
/// scrollback region using [`Terminal::insert_before`]. Any backend error is
/// ignored: failing to insert history is non‑fatal and a subsequent redraw
/// will eventually repaint a consistent view.
fn display_width(s: &str) -> usize {
    s.chars()
        .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
        .sum()
}

struct LineBuilder {
    term_width: usize,
    spans: Vec<Span<'static>>,
    width: usize,
}

impl LineBuilder {
    fn new(term_width: usize) -> Self {
        Self {
            term_width,
            spans: Vec::new(),
            width: 0,
        }
    }

    fn flush_line(&mut self, out: &mut Vec<Line<'static>>) {
        out.push(Line::from(std::mem::take(&mut self.spans)));
        self.width = 0;
    }

    fn push_segment(&mut self, text: String, style: Style) {
        self.width += display_width(&text);
        self.spans.push(Span::styled(text, style));
    }

    fn push_word(&mut self, word: &mut String, style: Style, out: &mut Vec<Line<'static>>) {
        if word.is_empty() {
            return;
        }
        let w_len = display_width(word);
        if self.width > 0 && self.width + w_len > self.term_width {
            self.flush_line(out);
        }
        if w_len > self.term_width && self.width == 0 {
            // Split an overlong word across multiple lines.
            let mut cur = String::new();
            let mut cur_w = 0;
            for ch in word.chars() {
                let ch_w = UnicodeWidthChar::width(ch).unwrap_or(0);
                if cur_w + ch_w > self.term_width && cur_w > 0 {
                    self.push_segment(cur.clone(), style);
                    self.flush_line(out);
                    cur.clear();
                    cur_w = 0;
                }
                cur.push(ch);
                cur_w += ch_w;
            }
            if !cur.is_empty() {
                self.push_segment(cur, style);
            }
        } else {
            self.push_segment(word.clone(), style);
        }
        word.clear();
    }

    fn consume_whitespace(&mut self, ws: &mut String, style: Style, out: &mut Vec<Line<'static>>) {
        if ws.is_empty() {
            return;
        }
        let space_w = display_width(ws);
        if self.width > 0 && self.width + space_w > self.term_width {
            self.flush_line(out);
        }
        if self.width > 0 {
            self.push_segment(" ".to_string(), style);
        }
        ws.clear();
    }
}

pub(crate) fn insert_history_lines(terminal: &mut tui::Tui, lines: Vec<Line<'static>>) {
    let term_width = terminal.size().map(|a| a.width).unwrap_or(80) as usize;
    let mut physical: Vec<Line<'static>> = Vec::new();

    for logical in lines.into_iter() {
        if logical.spans.is_empty() {
            physical.push(logical);
            continue;
        }

        let mut builder = LineBuilder::new(term_width);
        let mut buf_space = String::new();

        for span in logical.spans.into_iter() {
            let style = span.style;
            let mut buf_word = String::new();

            for ch in span.content.chars() {
                if ch == '\n' {
                    builder.push_word(&mut buf_word, style, &mut physical);
                    buf_space.clear();
                    builder.flush_line(&mut physical);
                    continue;
                }
                if ch.is_whitespace() {
                    builder.push_word(&mut buf_word, style, &mut physical);
                    buf_space.push(ch);
                } else {
                    builder.consume_whitespace(&mut buf_space, style, &mut physical);
                    buf_word.push(ch);
                }
                if builder.width >= term_width {
                    builder.flush_line(&mut physical);
                }
            }
            builder.push_word(&mut buf_word, style, &mut physical);
            // whitespace intentionally left to allow collapsing across spans
        }
        if !builder.spans.is_empty() {
            physical.push(Line::from(std::mem::take(&mut builder.spans)));
        } else {
            // Preserve explicit blank line (e.g. due to a trailing newline).
            physical.push(Line::from(Vec::<Span<'static>>::new()));
        }
    }

    let total = physical.len() as u16;
    terminal
        .insert_before(total, |buf| {
            let width = buf.area.width;
            for (i, line) in physical.into_iter().enumerate() {
                let area = Rect {
                    x: 0,
                    y: i as u16,
                    width,
                    height: 1,
                };
                Paragraph::new(line).render(area, buf);
            }
        })
        .ok();
}
