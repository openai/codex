use std::fmt;
use std::io;
use std::io::Write;

use crate::tui;
use crossterm::Command;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Color as CColor;
use crossterm::style::Colors;
use crossterm::style::Print;
use crossterm::style::SetAttribute;
use crossterm::style::SetBackgroundColor;
use crossterm::style::SetColors;
use crossterm::style::SetForegroundColor;
use ratatui::layout::Size;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::text::Line;
use ratatui::text::Span;
use textwrap::Options as TwOptions;
use textwrap::WordSplitter;

/// Insert `lines` above the viewport using the terminal's backend writer
/// (avoids direct stdout references).
pub(crate) fn insert_history_lines(terminal: &mut tui::Terminal, lines: Vec<Line>) {
    let mut out = std::io::stdout();
    insert_history_lines_to_writer(terminal, &mut out, lines);
}

/// Like `insert_history_lines`, but writes ANSI to the provided writer. This
/// is intended for testing where a capture buffer is used instead of stdout.
pub fn insert_history_lines_to_writer<B, W>(
    terminal: &mut crate::custom_terminal::Terminal<B>,
    writer: &mut W,
    lines: Vec<Line>,
) where
    B: ratatui::backend::Backend,
    W: Write,
{
    let screen_size = terminal.backend().size().unwrap_or(Size::new(0, 0));

    let mut area = terminal.viewport_area;

    // Pre-wrap lines using word-aware wrapping so terminal scrollback sees the same
    // formatting as the TUI. This avoids character-level hard wrapping by the terminal.
    let wrapped = word_wrap_lines(&lines, area.width.max(1));
    let wrapped_lines = wrapped.len() as u16;
    let cursor_top = if area.bottom() < screen_size.height {
        // If the viewport is not at the bottom of the screen, scroll it down to make room.
        // Don't scroll it past the bottom of the screen.
        let scroll_amount = wrapped_lines.min(screen_size.height - area.bottom());

        // Emit ANSI to scroll the lower region (from the top of the viewport to the bottom
        // of the screen) downward by `scroll_amount` lines. We do this by:
        //   1) Limiting the scroll region to [area.top()+1 .. screen_height] (1-based bounds)
        //   2) Placing the cursor at the top margin of that region
        //   3) Emitting Reverse Index (RI, ESC M) `scroll_amount` times
        //   4) Resetting the scroll region back to full screen
        let top_1based = area.top() + 1; // Convert 0-based row to 1-based for DECSTBM
        queue!(writer, SetScrollRegion(top_1based..screen_size.height)).ok();
        queue!(writer, MoveTo(0, area.top())).ok();
        for _ in 0..scroll_amount {
            // Reverse Index (RI): ESC M
            queue!(writer, Print("\x1bM")).ok();
        }
        queue!(writer, ResetScrollRegion).ok();

        let cursor_top = area.top().saturating_sub(1);
        area.y += scroll_amount;
        terminal.set_viewport_area(area);
        cursor_top
    } else {
        area.top().saturating_sub(1)
    };

    // Limit the scroll region to the lines from the top of the screen to the
    // top of the viewport. With this in place, when we add lines inside this
    // area, only the lines in this area will be scrolled. We place the cursor
    // at the end of the scroll region, and add lines starting there.
    //
    // ┌─Screen───────────────────────┐
    // │┌╌Scroll region╌╌╌╌╌╌╌╌╌╌╌╌╌╌┐│
    // │┆                            ┆│
    // │┆                            ┆│
    // │┆                            ┆│
    // │█╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┘│
    // │╭─Viewport───────────────────╮│
    // ││                            ││
    // │╰────────────────────────────╯│
    // └──────────────────────────────┘
    queue!(writer, SetScrollRegion(1..area.top())).ok();

    // NB: we are using MoveTo instead of set_cursor_position here to avoid messing with the
    // terminal's last_known_cursor_position, which hopefully will still be accurate after we
    // fetch/restore the cursor position. insert_history_lines should be cursor-position-neutral :)
    queue!(writer, MoveTo(0, cursor_top)).ok();

    for line in wrapped {
        queue!(writer, Print("\r\n")).ok();
        write_spans(writer, line.iter()).ok();
    }

    queue!(writer, ResetScrollRegion).ok();

    // Restore the cursor position to where it was before we started.
    queue!(
        writer,
        MoveTo(
            terminal.last_known_cursor_pos.x,
            terminal.last_known_cursor_pos.y
        )
    )
    .ok();
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetScrollRegion(pub std::ops::Range<u16>);

impl Command for SetScrollRegion {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b[{};{}r", self.0.start, self.0.end)
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        panic!("tried to execute SetScrollRegion command using WinAPI, use ANSI instead");
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        // TODO(nornagon): is this supported on Windows?
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetScrollRegion;

impl Command for ResetScrollRegion {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b[r")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        panic!("tried to execute ResetScrollRegion command using WinAPI, use ANSI instead");
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        // TODO(nornagon): is this supported on Windows?
        true
    }
}

struct ModifierDiff {
    pub from: Modifier,
    pub to: Modifier,
}

impl ModifierDiff {
    fn queue<W>(self, mut w: W) -> io::Result<()>
    where
        W: io::Write,
    {
        use crossterm::style::Attribute as CAttribute;
        let removed = self.from - self.to;
        if removed.contains(Modifier::REVERSED) {
            queue!(w, SetAttribute(CAttribute::NoReverse))?;
        }
        if removed.contains(Modifier::BOLD) {
            queue!(w, SetAttribute(CAttribute::NormalIntensity))?;
            if self.to.contains(Modifier::DIM) {
                queue!(w, SetAttribute(CAttribute::Dim))?;
            }
        }
        if removed.contains(Modifier::ITALIC) {
            queue!(w, SetAttribute(CAttribute::NoItalic))?;
        }
        if removed.contains(Modifier::UNDERLINED) {
            queue!(w, SetAttribute(CAttribute::NoUnderline))?;
        }
        if removed.contains(Modifier::DIM) {
            queue!(w, SetAttribute(CAttribute::NormalIntensity))?;
        }
        if removed.contains(Modifier::CROSSED_OUT) {
            queue!(w, SetAttribute(CAttribute::NotCrossedOut))?;
        }
        if removed.contains(Modifier::SLOW_BLINK) || removed.contains(Modifier::RAPID_BLINK) {
            queue!(w, SetAttribute(CAttribute::NoBlink))?;
        }

        let added = self.to - self.from;
        if added.contains(Modifier::REVERSED) {
            queue!(w, SetAttribute(CAttribute::Reverse))?;
        }
        if added.contains(Modifier::BOLD) {
            queue!(w, SetAttribute(CAttribute::Bold))?;
        }
        if added.contains(Modifier::ITALIC) {
            queue!(w, SetAttribute(CAttribute::Italic))?;
        }
        if added.contains(Modifier::UNDERLINED) {
            queue!(w, SetAttribute(CAttribute::Underlined))?;
        }
        if added.contains(Modifier::DIM) {
            queue!(w, SetAttribute(CAttribute::Dim))?;
        }
        if added.contains(Modifier::CROSSED_OUT) {
            queue!(w, SetAttribute(CAttribute::CrossedOut))?;
        }
        if added.contains(Modifier::SLOW_BLINK) {
            queue!(w, SetAttribute(CAttribute::SlowBlink))?;
        }
        if added.contains(Modifier::RAPID_BLINK) {
            queue!(w, SetAttribute(CAttribute::RapidBlink))?;
        }

        Ok(())
    }
}

fn write_spans<'a, I>(mut writer: &mut impl Write, content: I) -> io::Result<()>
where
    I: Iterator<Item = &'a Span<'a>>,
{
    let mut fg = Color::Reset;
    let mut bg = Color::Reset;
    let mut last_modifier = Modifier::empty();
    for span in content {
        let mut modifier = Modifier::empty();
        modifier.insert(span.style.add_modifier);
        modifier.remove(span.style.sub_modifier);
        if modifier != last_modifier {
            let diff = ModifierDiff {
                from: last_modifier,
                to: modifier,
            };
            diff.queue(&mut writer)?;
            last_modifier = modifier;
        }
        let next_fg = span.style.fg.unwrap_or(Color::Reset);
        let next_bg = span.style.bg.unwrap_or(Color::Reset);
        if next_fg != fg || next_bg != bg {
            queue!(
                writer,
                SetColors(Colors::new(next_fg.into(), next_bg.into()))
            )?;
            fg = next_fg;
            bg = next_bg;
        }

        queue!(writer, Print(span.content.clone()))?;
    }

    queue!(
        writer,
        SetForegroundColor(CColor::Reset),
        SetBackgroundColor(CColor::Reset),
        SetAttribute(crossterm::style::Attribute::Reset),
    )
}

/// Word-aware wrapping for a list of `Line`s preserving styles.
pub(crate) fn word_wrap_lines<'a, I>(lines: I, width: u16) -> Vec<Line<'static>>
where
    I: IntoIterator<Item = &'a Line<'a>>,
{
    let mut out = Vec::new();
    let w = width.max(1) as usize;
    for line in lines {
        out.extend(word_wrap_line(line, w));
    }
    out
}

fn word_wrap_line(line: &Line, width: usize) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![to_owned_line(line)];
    }
    // Concatenate content and keep span boundaries for later re-slicing.
    let mut flat = String::new();
    let mut span_bounds = Vec::new(); // (start_byte, end_byte, style)
    let mut cursor = 0usize;
    for s in &line.spans {
        let text = s.content.as_ref();
        let start = cursor;
        flat.push_str(text);
        cursor += text.len();
        span_bounds.push((start, cursor, s.style));
    }

    // Use textwrap for robust word-aware wrapping; no hyphenation, no breaking words.
    let opts = TwOptions::new(width)
        .break_words(false)
        .word_splitter(WordSplitter::NoHyphenation);
    let wrapped = textwrap::wrap(&flat, &opts);

    if wrapped.len() <= 1 {
        return vec![to_owned_line(line)];
    }

    // Map wrapped pieces back to byte ranges in `flat` sequentially.
    let mut start_cursor = 0usize;
    let mut out: Vec<Line<'static>> = Vec::with_capacity(wrapped.len());
    for piece in wrapped {
        let piece_str: &str = &piece;
        if piece_str.is_empty() {
            out.push(Line {
                style: line.style,
                alignment: line.alignment,
                spans: Vec::new(),
            });
            continue;
        }
        // Find the next occurrence of piece_str at or after start_cursor.
        // textwrap preserves order, so a linear scan is sufficient.
        if let Some(rel) = flat[start_cursor..].find(piece_str) {
            let s = start_cursor + rel;
            let e = s + piece_str.len();
            out.push(slice_line_spans(line, &span_bounds, s, e));
            start_cursor = e;
        } else {
            // Fallback: slice by length from cursor.
            let s = start_cursor;
            let e = (start_cursor + piece_str.len()).min(flat.len());
            out.push(slice_line_spans(line, &span_bounds, s, e));
            start_cursor = e;
        }
    }

    out
}

fn to_owned_line(l: &Line<'_>) -> Line<'static> {
    Line {
        style: l.style,
        alignment: l.alignment,
        spans: l
            .spans
            .iter()
            .map(|s| Span {
                style: s.style,
                content: std::borrow::Cow::Owned(s.content.to_string()),
            })
            .collect(),
    }
}

fn slice_line_spans(
    original: &Line<'_>,
    span_bounds: &[(usize, usize, ratatui::style::Style)],
    start_byte: usize,
    end_byte: usize,
) -> Line<'static> {
    let mut acc: Vec<Span<'static>> = Vec::new();
    for (i, (s, e, style)) in span_bounds.iter().enumerate() {
        if *e <= start_byte {
            continue;
        }
        if *s >= end_byte {
            break;
        }
        let seg_start = start_byte.max(*s);
        let seg_end = end_byte.min(*e);
        if seg_end > seg_start {
            let local_start = seg_start - *s;
            let local_end = seg_end - *s;
            let content = original.spans[i].content.as_ref();
            let slice = &content[local_start..local_end];
            acc.push(Span {
                style: *style,
                content: std::borrow::Cow::Owned(slice.to_string()),
            });
        }
        if *e >= end_byte {
            break;
        }
    }
    Line {
        style: original.style,
        alignment: original.alignment,
        spans: acc,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_bold_then_regular_spans() {
        use ratatui::style::Stylize;

        let spans = ["A".bold(), "B".into()];

        let mut actual: Vec<u8> = Vec::new();
        write_spans(&mut actual, spans.iter()).unwrap();

        let mut expected: Vec<u8> = Vec::new();
        queue!(
            expected,
            SetAttribute(crossterm::style::Attribute::Bold),
            Print("A"),
            SetAttribute(crossterm::style::Attribute::NormalIntensity),
            Print("B"),
            SetForegroundColor(CColor::Reset),
            SetBackgroundColor(CColor::Reset),
            SetAttribute(crossterm::style::Attribute::Reset),
        )
        .unwrap();

        assert_eq!(
            String::from_utf8(actual).unwrap(),
            String::from_utf8(expected).unwrap()
        );
    }

    #[test]
    fn line_height_counts_double_width_emoji() {
        let line = Line::from("😀😀😀"); // each emoji ~ width 2
        assert_eq!(word_wrap_line(&line, 4).len(), 2);
        assert_eq!(word_wrap_line(&line, 2).len(), 3);
        assert_eq!(word_wrap_line(&line, 6).len(), 1);
    }

    #[test]
    fn word_wrap_does_not_split_words_simple_english() {
        let sample = "Years passed, and Willowmere thrived in peace and friendship. Mira’s herb garden flourished with both ordinary and enchanted plants, and travelers spoke of the kindness of the woman who tended them.";
        let line = Line::from(sample);
        // Force small width to exercise wrapping at spaces.
        let wrapped = word_wrap_lines(&[line], 40);
        let joined: String = wrapped
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !joined.contains("bo\nth"),
            "word 'both' should not be split across lines:\n{joined}"
        );
        assert!(
            !joined.contains("Willowm\nere"),
            "should not split inside words:\n{joined}"
        );
    }
}
