use crate::tui; // for the Tui type alias
use ratatui::layout::Rect;
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
pub(crate) fn insert_history_lines(terminal: &mut tui::Tui, lines: Vec<Line<'static>>) {
    let term_width = terminal.size().map(|a| a.width).unwrap_or(80) as usize;
    let mut physical: Vec<Line<'static>> = Vec::new();

    for logical in lines.into_iter() {
        if logical.spans.is_empty() {
            physical.push(logical);
            continue;
        }

        let mut line_spans: Vec<Span<'static>> = Vec::new();
        let mut line_width: usize = 0;

        // Helper that finalises the current in‑progress line.
        let flush_line =
            |store: &mut Vec<Line<'static>>, spans: &mut Vec<Span<'static>>, width: &mut usize| {
                store.push(Line::from(spans.clone()));
                spans.clear();
                *width = 0;
            };

        // Iterate spans tokenising into words and whitespace so wrapping can
        // happen at word boundaries.
        for span in logical.spans.into_iter() {
            let style = span.style;
            let mut buf_word = String::new();
            let mut buf_space = String::new();
            let flush_word = |word: &mut String,
                              line_spans: &mut Vec<Span<'static>>,
                              line_width: &mut usize,
                              store: &mut Vec<Line<'static>>| {
                if word.is_empty() {
                    return;
                }
                let w_len: usize = word
                    .chars()
                    .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
                    .sum();
                if *line_width > 0 && *line_width + w_len > term_width {
                    flush_line(store, line_spans, line_width);
                }
                if w_len > term_width && *line_width == 0 {
                    // Break an overlong word across multiple physical lines.
                    let mut cur = String::new();
                    let mut cur_w = 0usize;
                    for ch in word.chars() {
                        let ch_w = UnicodeWidthChar::width(ch).unwrap_or(0);
                        if cur_w + ch_w > term_width && cur_w > 0 {
                            line_spans.push(Span::styled(cur.clone(), style));
                            flush_line(store, line_spans, line_width);
                            cur.clear();
                            cur_w = 0;
                        }
                        cur.push(ch);
                        cur_w += ch_w;
                    }
                    if !cur.is_empty() {
                        line_spans.push(Span::styled(cur.clone(), style));
                        *line_width += cur_w;
                    }
                } else {
                    line_spans.push(Span::styled(word.clone(), style));
                    *line_width += w_len;
                }
                word.clear();
            };

            for ch in span.content.chars() {
                if ch == '\n' {
                    flush_word(
                        &mut buf_word,
                        &mut line_spans,
                        &mut line_width,
                        &mut physical,
                    );
                    buf_space.clear();
                    flush_line(&mut physical, &mut line_spans, &mut line_width);
                    continue;
                }
                if ch.is_whitespace() {
                    if !buf_word.is_empty() {
                        flush_word(
                            &mut buf_word,
                            &mut line_spans,
                            &mut line_width,
                            &mut physical,
                        );
                    }
                    buf_space.push(ch);
                } else {
                    if !buf_space.is_empty() {
                        // Collapse a run of whitespace to a single space if it fits.
                        let space_w: usize = buf_space
                            .chars()
                            .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
                            .sum();
                        if line_width > 0 && line_width + space_w > term_width {
                            flush_line(&mut physical, &mut line_spans, &mut line_width);
                        }
                        if line_width > 0 {
                            // avoid leading spaces
                            line_spans.push(Span::styled(" ".to_string(), style));
                            line_width += 1;
                        }
                        buf_space.clear();
                    }
                    buf_word.push(ch);
                }
                // Soft wrap when the line exactly fills the available width.
                if line_width >= term_width {
                    flush_line(&mut physical, &mut line_spans, &mut line_width);
                }
            }
            // Flush any dangling word at span end. Whitespace is intentionally
            // deferred so runs can collapse across span boundaries.
            flush_word(
                &mut buf_word,
                &mut line_spans,
                &mut line_width,
                &mut physical,
            );
        }
        if !line_spans.is_empty() {
            physical.push(Line::from(line_spans));
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
