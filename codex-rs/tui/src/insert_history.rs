use std::fmt;
use std::io;
use std::io::Write;

use crate::osc8::osc8_hyperlink;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_line;
use crate::wrapping::line_contains_url_like;
use crate::wrapping::line_has_mixed_url_and_non_url_tokens;
use crossterm::Command;
use crossterm::cursor::MoveDown;
use crossterm::cursor::MoveTo;
use crossterm::cursor::MoveToColumn;
use crossterm::cursor::RestorePosition;
use crossterm::cursor::SavePosition;
use crossterm::queue;
use crossterm::style::Color as CColor;
use crossterm::style::Colors;
use crossterm::style::Print;
use crossterm::style::SetAttribute;
use crossterm::style::SetBackgroundColor;
use crossterm::style::SetColors;
use crossterm::style::SetForegroundColor;
use crossterm::terminal::Clear;
use crossterm::terminal::ClearType;
use ratatui::layout::Size;
use ratatui::prelude::Backend;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;

/// Selects the terminal escape strategy for inserting history lines above the viewport.
///
/// Standard terminals support `DECSTBM` scroll regions and Reverse Index (`ESC M`),
/// which let us slide existing content down without redrawing it. Zellij silently
/// drops or mishandles those sequences, so `Zellij` mode falls back to emitting
/// newlines at the bottom of the screen and writing lines at absolute positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertHistoryMode {
    Standard,
    Zellij,
}

impl InsertHistoryMode {
    pub fn new(is_zellij: bool) -> Self {
        if is_zellij {
            Self::Zellij
        } else {
            Self::Standard
        }
    }
}

/// Insert `lines` above the viewport using the terminal's backend writer
/// (avoids direct stdout references).
pub fn insert_history_lines<B>(
    terminal: &mut crate::custom_terminal::Terminal<B>,
    lines: Vec<Line>,
) -> io::Result<()>
where
    B: Backend + Write,
{
    insert_history_lines_with_mode(terminal, lines, InsertHistoryMode::Standard)
}

/// Insert `lines` above the viewport, using the escape strategy selected by `mode`.
///
/// In `Standard` mode this manipulates DECSTBM scroll regions to slide existing
/// scrollback down and writes new lines into the freed space. In `Zellij` mode it
/// emits newlines at the screen bottom to create space (since Zellij ignores scroll
/// region escapes) and writes lines at computed absolute positions. Both modes
/// update `terminal.viewport_area` so subsequent draw passes know where the
/// viewport moved to.
pub fn insert_history_lines_with_mode<B>(
    terminal: &mut crate::custom_terminal::Terminal<B>,
    lines: Vec<Line>,
    mode: InsertHistoryMode,
) -> io::Result<()>
where
    B: Backend + Write,
{
    let screen_size = terminal.backend().size().unwrap_or(Size::new(0, 0));

    let mut area = terminal.viewport_area;
    let mut should_update_area = false;
    let last_cursor_pos = terminal.last_known_cursor_pos;
    let writer = terminal.backend_mut();

    // Pre-wrap lines for terminal scrollback. Three paths:
    //
    // - URL-only-ish lines are kept intact (no hard newlines inserted) so that
    //   terminal emulators can match them as clickable links. The
    //   terminal will character-wrap these lines at the viewport
    //   boundary.
    // - Mixed lines (URL + non-URL prose) are adaptively wrapped so
    //   non-URL text still wraps naturally while URL tokens remain
    //   unsplit.
    // - Non-URL lines also flow through adaptive wrapping; behavior is
    //   equivalent to standard wrapping when no URL is present.
    let wrap_width = area.width.max(1) as usize;
    let prepared = prepare_history_lines(&lines, wrap_width);
    let wrapped = prepared.lines;
    let wrapped_lines = prepared.rows;

    if matches!(mode, InsertHistoryMode::Zellij) {
        let space_below = screen_size.height.saturating_sub(area.bottom());
        let shift_down = wrapped_lines.min(space_below);
        let scroll_up_amount = wrapped_lines.saturating_sub(shift_down);

        if scroll_up_amount > 0 {
            // Scroll the entire screen up by emitting \n at the bottom
            queue!(writer, MoveTo(0, screen_size.height.saturating_sub(1)))?;
            for _ in 0..scroll_up_amount {
                queue!(writer, Print("\n"))?;
            }
        }

        if shift_down > 0 {
            area.y += shift_down;
            should_update_area = true;
        }

        let cursor_top = area.top().saturating_sub(scroll_up_amount + shift_down);
        queue!(writer, MoveTo(0, cursor_top))?;

        for (i, line) in wrapped.iter().enumerate() {
            if i > 0 {
                queue!(writer, Print("\r\n"))?;
            }
            write_prepared_history_line(writer, line, wrap_width)?;
        }
    } else {
        let cursor_top = if area.bottom() < screen_size.height {
            let scroll_amount = wrapped_lines.min(screen_size.height - area.bottom());

            let top_1based = area.top() + 1;
            queue!(writer, SetScrollRegion(top_1based..screen_size.height))?;
            queue!(writer, MoveTo(0, area.top()))?;
            for _ in 0..scroll_amount {
                queue!(writer, Print("\x1bM"))?;
            }
            queue!(writer, ResetScrollRegion)?;

            let cursor_top = area.top().saturating_sub(1);
            area.y += scroll_amount;
            should_update_area = true;
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
        queue!(writer, SetScrollRegion(1..area.top()))?;

        // NB: we are using MoveTo instead of set_cursor_position here to avoid messing with the
        // terminal's last_known_cursor_position, which hopefully will still be accurate after we
        // fetch/restore the cursor position. insert_history_lines should be cursor-position-neutral :)
        queue!(writer, MoveTo(0, cursor_top))?;

        for line in &wrapped {
            queue!(writer, Print("\r\n"))?;
            write_prepared_history_line(writer, line, wrap_width)?;
        }

        queue!(writer, ResetScrollRegion)?;
    }

    // Restore the cursor position to where it was before we started.
    queue!(writer, MoveTo(last_cursor_pos.x, last_cursor_pos.y))?;

    let _ = writer;
    if should_update_area {
        terminal.set_viewport_area(area);
    }
    if wrapped_lines > 0 {
        terminal.note_history_rows_inserted(wrapped_lines);
    }

    Ok(())
}

struct PreparedHistoryLines<'a> {
    lines: Vec<PreparedHistoryLine<'a>>,
    rows: u16,
}

struct PreparedHistoryLine<'a> {
    line: Line<'a>,
    span_hyperlinks: Vec<Option<String>>,
}

fn prepare_history_lines<'a>(lines: &'a [Line<'a>], wrap_width: usize) -> PreparedHistoryLines<'a> {
    let mut prepared_lines = Vec::new();
    let mut rows = 0usize;

    for line in lines {
        let wrapped =
            if line_contains_url_like(line) && !line_has_mixed_url_and_non_url_tokens(line) {
                vec![line.clone()]
            } else {
                adaptive_wrap_line(line, RtOptions::new(wrap_width))
            };
        rows += wrapped
            .iter()
            .map(|wrapped_line| wrapped_line.width().max(1).div_ceil(wrap_width))
            .sum::<usize>();
        prepared_lines.extend(mark_markdown_link_spans(wrapped));
    }

    PreparedHistoryLines {
        lines: prepared_lines,
        rows: rows as u16,
    }
}

/// Render a single wrapped history line: clear continuation rows for wide lines,
/// set foreground/background colors, and write styled spans. Caller is responsible
/// for cursor positioning and any leading `\r\n`.
#[cfg(test)]
fn write_history_line<W: Write>(writer: &mut W, line: &Line, wrap_width: usize) -> io::Result<()> {
    let prepared = mark_markdown_link_spans(vec![line.clone()]);
    let prepared = prepared.first().expect("one prepared line");
    write_prepared_history_line(writer, prepared, wrap_width)
}

fn write_prepared_history_line<W: Write>(
    writer: &mut W,
    prepared: &PreparedHistoryLine<'_>,
    wrap_width: usize,
) -> io::Result<()> {
    let line = &prepared.line;
    let physical_rows = line.width().max(1).div_ceil(wrap_width) as u16;
    if physical_rows > 1 {
        queue!(writer, SavePosition)?;
        for _ in 1..physical_rows {
            queue!(writer, MoveDown(1), MoveToColumn(0))?;
            queue!(writer, Clear(ClearType::UntilNewLine))?;
        }
        queue!(writer, RestorePosition)?;
    }
    queue!(
        writer,
        SetColors(Colors::new(
            line.style
                .fg
                .map(std::convert::Into::into)
                .unwrap_or(CColor::Reset),
            line.style
                .bg
                .map(std::convert::Into::into)
                .unwrap_or(CColor::Reset)
        ))
    )?;
    queue!(writer, Clear(ClearType::UntilNewLine))?;
    // Merge line-level style into each span so that ANSI colors reflect
    // line styles (e.g., blockquotes with green fg).
    let merged_spans: Vec<Span> = line
        .spans
        .iter()
        .map(|s| Span {
            style: s.style.patch(line.style),
            content: s.content.clone(),
        })
        .collect();
    let merged_span_refs = merged_spans.iter().collect::<Vec<_>>();
    write_spans_with_hyperlinks(writer, &merged_span_refs, &prepared.span_hyperlinks)
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

#[cfg(test)]
fn write_spans<'a, I>(mut writer: &mut impl Write, content: I) -> io::Result<()>
where
    I: IntoIterator<Item = &'a Span<'a>>,
{
    let spans = content.into_iter().collect::<Vec<_>>();
    let hyperlinks = vec![None; spans.len()];
    write_spans_with_hyperlinks(&mut writer, &spans, &hyperlinks)
}

fn write_spans_with_hyperlinks(
    mut writer: &mut impl Write,
    spans: &[&Span<'_>],
    hyperlinks: &[Option<String>],
) -> io::Result<()> {
    let mut fg = Color::Reset;
    let mut bg = Color::Reset;
    let mut last_modifier = Modifier::empty();

    for (span, hyperlink) in spans.iter().zip(hyperlinks) {
        write_span(
            &mut writer,
            span,
            hyperlink.as_deref(),
            &mut fg,
            &mut bg,
            &mut last_modifier,
        )?;
    }

    queue!(
        writer,
        SetForegroundColor(CColor::Reset),
        SetBackgroundColor(CColor::Reset),
        SetAttribute(crossterm::style::Attribute::Reset),
    )
}

fn write_span(
    writer: &mut impl Write,
    span: &Span<'_>,
    hyperlink_destination: Option<&str>,
    fg: &mut Color,
    bg: &mut Color,
    last_modifier: &mut Modifier,
) -> io::Result<()> {
    let mut modifier = Modifier::empty();
    modifier.insert(span.style.add_modifier);
    modifier.remove(span.style.sub_modifier);
    if modifier != *last_modifier {
        let diff = ModifierDiff {
            from: *last_modifier,
            to: modifier,
        };
        diff.queue(&mut *writer)?;
        *last_modifier = modifier;
    }
    let next_fg = span.style.fg.unwrap_or(Color::Reset);
    let next_bg = span.style.bg.unwrap_or(Color::Reset);
    if next_fg != *fg || next_bg != *bg {
        queue!(
            writer,
            SetColors(Colors::new(next_fg.into(), next_bg.into()))
        )?;
        *fg = next_fg;
        *bg = next_bg;
    }

    if let Some(destination) = hyperlink_destination {
        queue!(
            writer,
            Print(osc8_hyperlink(destination, span.content.as_ref()))
        )?;
    } else {
        queue!(writer, Print(span.content.clone()))?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct SpanPosition {
    line: usize,
    span: usize,
}

struct LinkSpanSnapshot<'a> {
    position: SpanPosition,
    content: &'a str,
    style: Style,
}

struct MarkdownLink<'a> {
    label: &'a [LinkSpanSnapshot<'a>],
    destination_spans: &'a [LinkSpanSnapshot<'a>],
    destination: String,
    end: usize,
}

impl<'a> MarkdownLink<'a> {
    fn parse(spans: &'a [LinkSpanSnapshot<'a>], start: usize) -> Option<Self> {
        if !is_link_label_styled(spans.get(start)?) {
            return None;
        }

        let mut separator = start;
        while separator < spans.len() && is_link_label_styled(&spans[separator]) {
            separator += 1;
        }
        if !matches!(spans.get(separator)?.content, " (" | "(") {
            return None;
        }

        let destination_start = separator + 1;
        let mut close = destination_start;
        while close < spans.len() && is_link_destination_styled(&spans[close]) {
            close += 1;
        }
        if close == destination_start || spans.get(close)?.content != ")" {
            return None;
        }

        let destination = spans[destination_start..close]
            .iter()
            .map(|span| span.content)
            .collect::<String>();
        if !is_remote_url(&destination) {
            return None;
        }

        Some(Self {
            label: &spans[start..separator],
            destination_spans: &spans[destination_start..close],
            destination,
            end: close + 1,
        })
    }
}

fn mark_markdown_link_spans(lines: Vec<Line<'_>>) -> Vec<PreparedHistoryLine<'_>> {
    let mut prepared = lines
        .into_iter()
        .map(|line| {
            let span_hyperlinks = vec![None; line.spans.len()];
            PreparedHistoryLine {
                line,
                span_hyperlinks,
            }
        })
        .collect::<Vec<_>>();

    let annotations =
        {
            let snapshots =
                prepared
                    .iter()
                    .enumerate()
                    .flat_map(|(line_index, prepared_line)| {
                        prepared_line.line.spans.iter().enumerate().map(
                            move |(span_index, span)| LinkSpanSnapshot {
                                position: SpanPosition {
                                    line: line_index,
                                    span: span_index,
                                },
                                content: span.content.as_ref(),
                                style: span.style,
                            },
                        )
                    })
                    .collect::<Vec<_>>();

            let mut annotations = Vec::new();
            let mut start = 0;
            while start < snapshots.len() {
                let Some(link) = MarkdownLink::parse(&snapshots, start) else {
                    start += 1;
                    continue;
                };

                for snapshot in link.label.iter().chain(link.destination_spans) {
                    annotations.push((snapshot.position, link.destination.clone()));
                }
                start = link.end;
            }
            annotations
        };

    for (position, destination) in annotations {
        prepared[position.line].span_hyperlinks[position.span] = Some(destination);
    }

    prepared
}

fn is_link_label_styled(span: &LinkSpanSnapshot<'_>) -> bool {
    span.style.add_modifier.contains(Modifier::UNDERLINED)
        && !span.style.sub_modifier.contains(Modifier::UNDERLINED)
        && span.style.fg == Some(Color::Cyan)
        && !span.content.trim().is_empty()
}

fn is_link_destination_styled(span: &LinkSpanSnapshot<'_>) -> bool {
    is_link_label_styled(span)
}

fn is_remote_url(text: &str) -> bool {
    text.starts_with("http://") || text.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown_render::render_markdown_text;
    use crate::test_backend::VT100Backend;
    use ratatui::layout::Rect;
    use ratatui::style::Color;
    use ratatui::style::Stylize;

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
    fn write_history_line_emits_osc8_for_remote_markdown_link() {
        let text = render_markdown_text("[OpenAI](https://openai.com)");
        let line = text.lines.first().expect("rendered link line");

        let mut actual: Vec<u8> = Vec::new();
        write_history_line(&mut actual, line, /*wrap_width*/ 80).unwrap();

        let actual = String::from_utf8(actual).unwrap();
        assert!(
            actual.contains("\u{1b}]8;;https://openai.com\u{1b}\\OpenAI\u{1b}]8;;\u{1b}\\"),
            "label should be printed as an ST-terminated OSC-8 hyperlink: {actual:?}"
        );
        assert!(
            actual.contains(
                "\u{1b}]8;;https://openai.com\u{1b}\\https://openai.com\u{1b}]8;;\u{1b}\\"
            ),
            "destination should be printed as an ST-terminated OSC-8 hyperlink: {actual:?}"
        );
        assert!(
            !actual.contains('\u{7}'),
            "new OSC-8 output should not use BEL"
        );
    }

    #[test]
    fn write_history_line_does_not_emit_osc8_for_underlined_non_markdown_url_pattern() {
        let line = Line::from(vec![
            "underlined note".underlined(),
            " (".into(),
            "https://example.com/not-a-markdown-destination".underlined(),
            ")".into(),
        ]);

        let mut actual: Vec<u8> = Vec::new();
        write_history_line(&mut actual, &line, /*wrap_width*/ 80).unwrap();

        let actual = String::from_utf8(actual).unwrap();
        assert!(
            !actual.contains("\u{1b}]8;;"),
            "plain underlined text should stay as styled text, not OSC-8: {actual:?}"
        );
    }

    #[test]
    fn inserted_history_preserves_osc8_when_markdown_link_wraps_before_writing() {
        let text = render_markdown_text("[OpenAI Platform](https://openai.com/docs/codex/osc8)");
        let width: u16 = 24;
        let height: u16 = 8;
        let backend = VT100Backend::new(width, height);
        let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
        term.set_viewport_area(Rect::new(0, height - 1, width, 1));

        insert_history_lines(&mut term, text.lines).expect("history insertion should succeed");

        let actual = String::from_utf8_lossy(term.backend().written_output());
        assert!(
            actual.contains(
                "\u{1b}]8;;https://openai.com/docs/codex/osc8\u{1b}\\OpenAI Platform\u{1b}]8;;\u{1b}\\"
            ),
            "wrapped label should still be printed as OSC-8: {actual:?}"
        );
        assert!(
            actual.contains("\u{1b}]8;;https://openai.com/docs/codex/osc8\u{1b}\\https://"),
            "wrapped destination should still be printed as OSC-8: {actual:?}"
        );
    }

    #[test]
    fn write_history_line_does_not_expand_heading_underline_into_link_label() {
        let text = render_markdown_text("# See [docs](https://example.com/docs)");
        let line = text.lines.first().expect("rendered heading line");

        let mut actual: Vec<u8> = Vec::new();
        write_history_line(&mut actual, line, /*wrap_width*/ 80).unwrap();

        let actual = String::from_utf8(actual).unwrap();
        assert!(
            actual.contains("\u{1b}]8;;https://example.com/docs\u{1b}\\docs\u{1b}]8;;\u{1b}\\"),
            "actual markdown label should be OSC-8: {actual:?}"
        );
        assert!(
            !actual.contains("\u{1b}]8;;https://example.com/docs\u{1b}\\# See"),
            "heading marker/text before the link must not be included in OSC-8 label: {actual:?}"
        );
    }

    #[test]
    fn write_history_line_emits_osc8_for_link_inside_blockquote() {
        let text = render_markdown_text("> [OpenAI](https://openai.com)");
        let line = text.lines.first().expect("rendered blockquote link line");

        let mut actual: Vec<u8> = Vec::new();
        write_history_line(&mut actual, line, /*wrap_width*/ 80).unwrap();

        let actual = String::from_utf8(actual).unwrap();
        assert!(
            actual.contains("\u{1b}]8;;https://openai.com\u{1b}\\OpenAI\u{1b}]8;;\u{1b}\\"),
            "blockquote link label should be OSC-8 despite line-level blockquote style: {actual:?}"
        );
    }

    #[test]
    fn vt100_blockquote_line_emits_green_fg() {
        // Set up a small off-screen terminal
        let width: u16 = 40;
        let height: u16 = 10;
        let backend = VT100Backend::new(width, height);
        let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
        // Place viewport on the last line so history inserts scroll upward
        let viewport = Rect::new(0, height - 1, width, 1);
        term.set_viewport_area(viewport);

        // Build a blockquote-like line: apply line-level green style and prefix "> "
        let mut line: Line<'static> = Line::from(vec!["> ".into(), "Hello world".into()]);
        line = line.style(Color::Green);
        insert_history_lines(&mut term, vec![line])
            .expect("Failed to insert history lines in test");

        let mut saw_colored = false;
        'outer: for row in 0..height {
            for col in 0..width {
                if let Some(cell) = term.backend().vt100().screen().cell(row, col)
                    && cell.has_contents()
                    && cell.fgcolor() != vt100::Color::Default
                {
                    saw_colored = true;
                    break 'outer;
                }
            }
        }
        assert!(
            saw_colored,
            "expected at least one colored cell in vt100 output"
        );
    }

    #[test]
    fn vt100_blockquote_wrap_preserves_color_on_all_wrapped_lines() {
        // Force wrapping by using a narrow viewport width and a long blockquote line.
        let width: u16 = 20;
        let height: u16 = 8;
        let backend = VT100Backend::new(width, height);
        let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
        // Viewport is the last line so history goes directly above it.
        let viewport = Rect::new(0, height - 1, width, 1);
        term.set_viewport_area(viewport);

        // Create a long blockquote with a distinct prefix and enough text to wrap.
        let mut line: Line<'static> = Line::from(vec![
            "> ".into(),
            "This is a long quoted line that should wrap".into(),
        ]);
        line = line.style(Color::Green);

        insert_history_lines(&mut term, vec![line])
            .expect("Failed to insert history lines in test");

        // Parse and inspect the final screen buffer.
        let screen = term.backend().vt100().screen();

        // Collect rows that are non-empty; these should correspond to our wrapped lines.
        let mut non_empty_rows: Vec<u16> = Vec::new();
        for row in 0..height {
            let mut any = false;
            for col in 0..width {
                if let Some(cell) = screen.cell(row, col)
                    && cell.has_contents()
                    && cell.contents() != "\0"
                    && cell.contents() != " "
                {
                    any = true;
                    break;
                }
            }
            if any {
                non_empty_rows.push(row);
            }
        }

        // Expect at least two rows due to wrapping.
        assert!(
            non_empty_rows.len() >= 2,
            "expected wrapped output to span >=2 rows, got {non_empty_rows:?}",
        );

        // For each non-empty row, ensure all non-space cells are using a non-default fg color.
        for row in non_empty_rows {
            for col in 0..width {
                if let Some(cell) = screen.cell(row, col) {
                    let contents = cell.contents();
                    if !contents.is_empty() && contents != " " {
                        assert!(
                            cell.fgcolor() != vt100::Color::Default,
                            "expected non-default fg on row {row} col {col}, got {:?}",
                            cell.fgcolor()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn vt100_colored_prefix_then_plain_text_resets_color() {
        let width: u16 = 40;
        let height: u16 = 6;
        let backend = VT100Backend::new(width, height);
        let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
        let viewport = Rect::new(0, height - 1, width, 1);
        term.set_viewport_area(viewport);

        // First span colored, rest plain.
        let line: Line<'static> = Line::from(vec![
            Span::styled("1. ", ratatui::style::Style::default().fg(Color::LightBlue)),
            Span::raw("Hello world"),
        ]);

        insert_history_lines(&mut term, vec![line])
            .expect("Failed to insert history lines in test");

        let screen = term.backend().vt100().screen();

        // Find the first non-empty row; verify first three cells are colored, following cells default.
        'rows: for row in 0..height {
            let mut has_text = false;
            for col in 0..width {
                if let Some(cell) = screen.cell(row, col)
                    && cell.has_contents()
                    && cell.contents() != " "
                {
                    has_text = true;
                    break;
                }
            }
            if !has_text {
                continue;
            }

            // Expect "1. Hello world" starting at col 0.
            for col in 0..3 {
                let cell = screen.cell(row, col).unwrap();
                assert!(
                    cell.fgcolor() != vt100::Color::Default,
                    "expected colored prefix at col {col}, got {:?}",
                    cell.fgcolor()
                );
            }
            for col in 3..(3 + "Hello world".len() as u16) {
                let cell = screen.cell(row, col).unwrap();
                assert_eq!(
                    cell.fgcolor(),
                    vt100::Color::Default,
                    "expected default color for plain text at col {col}, got {:?}",
                    cell.fgcolor()
                );
            }
            break 'rows;
        }
    }

    #[test]
    fn vt100_deep_nested_mixed_list_third_level_marker_is_colored() {
        // Markdown with five levels (ordered → unordered → ordered → unordered → unordered).
        let md = "1. First\n   - Second level\n     1. Third level (ordered)\n        - Fourth level (bullet)\n          - Fifth level to test indent consistency\n";
        let text = render_markdown_text(md);
        let lines: Vec<Line<'static>> = text.lines.clone();

        let width: u16 = 60;
        let height: u16 = 12;
        let backend = VT100Backend::new(width, height);
        let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
        let viewport = ratatui::layout::Rect::new(0, height - 1, width, 1);
        term.set_viewport_area(viewport);

        insert_history_lines(&mut term, lines).expect("Failed to insert history lines in test");

        let screen = term.backend().vt100().screen();

        // Reconstruct screen rows as strings to locate the 3rd level line.
        let rows: Vec<String> = screen.rows(0, width).collect();

        let needle = "1. Third level (ordered)";
        let row_idx = rows
            .iter()
            .position(|r| r.contains(needle))
            .unwrap_or_else(|| {
                panic!("expected to find row containing {needle:?}, have rows: {rows:?}")
            });
        let col_start = rows[row_idx].find(needle).unwrap() as u16; // column where '1' starts

        // Verify that the numeric marker ("1.") at the third level is colored
        // (non-default fg) and the content after the following space resets to default.
        for c in [col_start, col_start + 1] {
            let cell = screen.cell(row_idx as u16, c).unwrap();
            assert!(
                cell.fgcolor() != vt100::Color::Default,
                "expected colored 3rd-level marker at row {row_idx} col {c}, got {:?}",
                cell.fgcolor()
            );
        }
        let content_col = col_start + 3; // skip '1', '.', and the space
        if let Some(cell) = screen.cell(row_idx as u16, content_col) {
            assert_eq!(
                cell.fgcolor(),
                vt100::Color::Default,
                "expected default color for 3rd-level content at row {row_idx} col {content_col}, got {:?}",
                cell.fgcolor()
            );
        }
    }

    #[test]
    fn vt100_prefixed_url_keeps_prefix_and_url_on_same_row() {
        let width: u16 = 48;
        let height: u16 = 8;
        let backend = VT100Backend::new(width, height);
        let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
        let viewport = Rect::new(0, height - 1, width, 1);
        term.set_viewport_area(viewport);

        let url = "http://a-long-url.com/this/that/blablablab/new.aspx/many_people_like_how";
        let line: Line<'static> = Line::from(vec!["  │ ".into(), url.into()]);

        insert_history_lines(&mut term, vec![line]).expect("insert history");

        let rows: Vec<String> = term.backend().vt100().screen().rows(0, width).collect();

        assert!(
            rows.iter().any(|r| r.contains("│ http://a-long-url.com")),
            "expected prefix and URL on same row, rows: {rows:?}"
        );
        assert!(
            !rows.iter().any(|r| r.trim_end() == "│"),
            "unexpected orphan prefix row, rows: {rows:?}"
        );
    }

    #[test]
    fn vt100_prefixed_url_like_without_scheme_keeps_prefix_and_token_on_same_row() {
        let width: u16 = 48;
        let height: u16 = 8;
        let backend = VT100Backend::new(width, height);
        let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
        let viewport = Rect::new(0, height - 1, width, 1);
        term.set_viewport_area(viewport);

        let url_like =
            "example.test/api/v1/projects/alpha-team/releases/2026-02-17/builds/1234567890";
        let line: Line<'static> = Line::from(vec!["  │ ".into(), url_like.into()]);

        insert_history_lines(&mut term, vec![line]).expect("insert history");

        let rows: Vec<String> = term.backend().vt100().screen().rows(0, width).collect();

        assert!(
            rows.iter()
                .any(|r| r.contains("│ example.test/api/v1/projects")),
            "expected prefix and URL-like token on same row, rows: {rows:?}"
        );
        assert!(
            !rows.iter().any(|r| r.trim_end() == "│"),
            "unexpected orphan prefix row, rows: {rows:?}"
        );
    }

    #[test]
    fn vt100_prefixed_mixed_url_line_wraps_suffix_words_together() {
        let width: u16 = 24;
        let height: u16 = 10;
        let backend = VT100Backend::new(width, height);
        let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
        let viewport = Rect::new(0, height - 1, width, 1);
        term.set_viewport_area(viewport);

        let url = "https://example.test/path/abcdef12345";
        let line: Line<'static> = Line::from(vec![
            "  │ ".into(),
            "see ".into(),
            url.into(),
            " tail words".into(),
        ]);

        insert_history_lines(&mut term, vec![line]).expect("insert mixed history");

        let rows: Vec<String> = term.backend().vt100().screen().rows(0, width).collect();
        assert!(
            rows.iter().any(|r| r.contains("│ see")),
            "expected prefixed prose before URL, rows: {rows:?}"
        );
        assert!(
            rows.iter().any(|r| r.contains("tail words")),
            "expected suffix words to wrap as a phrase, rows: {rows:?}"
        );
    }

    #[test]
    fn vt100_unwrapped_url_like_clears_continuation_rows() {
        let width: u16 = 20;
        let height: u16 = 10;
        let backend = VT100Backend::new(width, height);
        let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
        let viewport = Rect::new(0, height - 1, width, 1);
        term.set_viewport_area(viewport);

        let filler_line: Line<'static> = Line::from(vec![
            "  │ ".into(),
            "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX".into(),
        ]);
        insert_history_lines(&mut term, vec![filler_line]).expect("insert filler history");

        let url_like = "example.test/api/v1/short";
        let url_line: Line<'static> = Line::from(vec!["  │ ".into(), url_like.into()]);
        insert_history_lines(&mut term, vec![url_line]).expect("insert url-like history");

        let rows: Vec<String> = term.backend().vt100().screen().rows(0, width).collect();
        let first_row = rows
            .iter()
            .position(|row| row.contains("│ example.test/api"))
            .unwrap_or_else(|| panic!("expected url-like first row in screen rows: {rows:?}"));
        assert!(
            first_row + 1 < rows.len(),
            "expected a continuation row for wrapped URL-like line, rows: {rows:?}"
        );
        let continuation_row = rows[first_row + 1].trim_end();

        assert!(
            continuation_row.contains("/v1/short") || continuation_row.contains("short"),
            "expected continuation row to contain wrapped URL-like tail, got: {continuation_row:?}"
        );
        assert!(
            !continuation_row.contains('X'),
            "expected continuation row to be cleared before writing wrapped URL-like content, got: {continuation_row:?}"
        );
    }

    #[test]
    fn vt100_long_unwrapped_url_does_not_insert_extra_blank_gap_before_content() {
        let width: u16 = 56;
        let height: u16 = 24;
        let backend = VT100Backend::new(width, height);
        let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
        let viewport = Rect::new(0, height - 1, width, 1);
        term.set_viewport_area(viewport);

        let prompt = "Write a long URL as output for testing";
        insert_history_lines(&mut term, vec![Line::from(prompt)]).expect("insert prompt line");

        let long_url = format!(
            "https://example.test/api/v1/projects/alpha-team/releases/2026-02-17/builds/1234567890/{}",
            "very-long-segment-".repeat(16),
        );
        let url_line: Line<'static> = Line::from(vec!["• ".into(), long_url.into()]);
        insert_history_lines(&mut term, vec![url_line]).expect("insert long url line");

        let rows: Vec<String> = term.backend().vt100().screen().rows(0, width).collect();
        let prompt_row = rows
            .iter()
            .position(|row| row.contains("Write a long URL as output for testing"))
            .unwrap_or_else(|| panic!("expected prompt row in screen rows: {rows:?}"));
        let url_row = rows
            .iter()
            .position(|row| row.contains("• https://example.test/api"))
            .unwrap_or_else(|| panic!("expected URL first row in screen rows: {rows:?}"));

        assert!(
            url_row <= prompt_row + 2,
            "expected URL content to appear immediately after prompt (allowing at most one spacer row), got prompt_row={prompt_row}, url_row={url_row}, rows={rows:?}",
        );
    }

    #[test]
    fn vt100_zellij_mode_inserts_history_and_updates_viewport() {
        let width: u16 = 32;
        let height: u16 = 8;
        let backend = VT100Backend::new(width, height);
        let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
        let viewport = Rect::new(0, 4, width, 2);
        term.set_viewport_area(viewport);

        let line: Line<'static> = Line::from("zellij history");
        insert_history_lines_with_mode(&mut term, vec![line], InsertHistoryMode::Zellij)
            .expect("insert zellij history");

        let rows: Vec<String> = term.backend().vt100().screen().rows(0, width).collect();
        assert!(
            rows.iter().any(|row| row.contains("zellij history")),
            "expected zellij history row in screen output, rows: {rows:?}"
        );
        assert_eq!(term.viewport_area, Rect::new(0, 5, width, 2));
        assert_eq!(term.visible_history_rows(), 1);
    }
}
