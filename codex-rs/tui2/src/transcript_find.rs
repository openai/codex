//! Inline "find in transcript" support for the TUI2 inline viewport.
//!
//! This module is intentionally UI-framework-light: it holds the find state and provides helpers
//! that `app.rs` calls from the normal render loop.
//!
//! **Integration points (in `tui2/src/app.rs`):**
//!
//! - Early key handling delegation via [`TranscriptFind::handle_key_event`]
//! - Per-frame update/jump hook via [`TranscriptFind::on_render`]
//! - Per-row highlight hook via [`TranscriptFind::render_line`]
//! - Prompt rendering + cursor positioning via [`TranscriptFind::render_prompt_line`] and
//!   [`TranscriptFind::cursor_position`]
//!
//! The search operates on the flattened transcript lines (`Line<'static>`) that are already used
//! for scrolling and selection, and produces stable jump targets via
//! [`TranscriptLineMeta::cell_line`](TranscriptLineMeta::cell_line).

use crate::render::line_utils::line_to_static;
use crate::tui::scrolling::TranscriptLineMeta;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr as _;

use std::ops::Range;

#[derive(Debug, Default)]
/// Stateful "find in transcript" controller for the inline viewport.
///
/// The state is designed around the "ephemeral prompt" UX:
///
/// - `Ctrl-F` enters editing mode (prompt line becomes visible)
/// - `Esc` closes the prompt while keeping highlights (query remains)
/// - `Esc` again clears the query/highlights
/// - `Ctrl-G` advances to the next match while editing or while a query is still active
///
/// The caller is expected to:
///
/// - call [`TranscriptFind::on_render`] after flattening the transcript into `lines` + `line_meta`
/// - call [`TranscriptFind::render_line`] for visible rows
/// - use [`TranscriptFind::render_prompt_line`] + [`TranscriptFind::cursor_position`] when editing
pub(crate) struct TranscriptFind {
    /// Current query text (plain UTF-8 string).
    query: String,

    /// Whether the prompt is currently visible and receiving input.
    editing: bool,

    /// Cached width of the flattened transcript viewport.
    ///
    /// Width changes can change wrapping and therefore the flattened line list.
    last_width: Option<u16>,

    /// Cached flattened line count.
    ///
    /// Note: callers should also invoke [`TranscriptFind::note_lines_changed`] when content changes
    /// without changing the count (e.g., streaming updates).
    last_lines_len: Option<usize>,

    /// All matches across the flattened transcript, in display order (line-major, left-to-right).
    matches: Vec<TranscriptFindMatch>,

    /// Per-line mapping to indices into `matches`, used by `render_line`.
    line_match_indices: Vec<Vec<usize>>,

    /// Index into `matches` representing the current match (if any).
    current_match: Option<usize>,

    /// Stable identifier for the current match used to preserve selection across recompute.
    ///
    /// `(line_index, match_range_start)`.
    current_key: Option<(usize, usize)>,

    /// A navigation action to apply on the next [`on_render`](TranscriptFind::on_render) call.
    pending: Option<TranscriptFindPendingAction>,
}

#[derive(Debug, Clone, Copy)]
/// Deferred navigation requests applied at the next render.
///
/// This allows key events to be handled without directly mutating the scroll state; instead we
/// produce a stable anchor from [`TranscriptLineMeta`] during [`TranscriptFind::on_render`].
enum TranscriptFindPendingAction {
    /// Jump to the "best" match for the current viewport (used after Enter).
    Jump,
    /// Advance the current match, wrapping around (used by Ctrl-G).
    Next,
}

#[derive(Debug, Clone)]
/// One match within the flattened transcript.
///
/// Matches are stored in "scan order" (line-major, then left-to-right), and carry a stable anchor
/// for jumping the viewport.
struct TranscriptFindMatch {
    line_index: usize,
    range: Range<usize>,
    /// Stable `(cell_index, line_in_cell)` anchor used to update scroll state.
    anchor: Option<(usize, usize)>,
}

impl TranscriptFind {
    /// Whether find is active (either editing or a non-empty query is present).
    pub(crate) fn is_active(&self) -> bool {
        self.editing || !self.query.is_empty()
    }

    /// Whether the find prompt should be visible.
    pub(crate) fn is_visible(&self) -> bool {
        self.editing
    }

    /// Force a recompute on the next render.
    ///
    /// This is intended for "content changed but line count didn't" scenarios, such as streaming
    /// assistant output updating in-place.
    pub(crate) fn note_lines_changed(&mut self) {
        if self.is_active() {
            self.last_lines_len = None;
        }
    }

    /// Handle find-related key events.
    ///
    /// Returns `true` when the key was consumed by the find UI/state machine.
    ///
    /// This method updates internal state only; it does not directly change the transcript scroll
    /// position. Navigation keys (e.g. `Ctrl-G`, `Enter`) set a pending action that is applied on
    /// the next [`on_render`](Self::on_render) call, where we can map the current match to a stable
    /// `(cell_index, line_in_cell)` anchor.
    pub(crate) fn handle_key_event(&mut self, key_event: &KeyEvent) -> bool {
        match *key_event {
            KeyEvent {
                code: KeyCode::Char('f'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            } => {
                self.begin_edit();
                true
            }
            KeyEvent {
                code: KeyCode::Esc,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } if self.editing => {
                self.end_edit();
                true
            }
            KeyEvent {
                code: KeyCode::Esc,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } if !self.query.is_empty() => {
                self.clear();
                true
            }
            KeyEvent {
                code: KeyCode::Char('g'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } if self.editing || !self.query.is_empty() => {
                self.set_pending(TranscriptFindPendingAction::Next);
                true
            }
            _ if self.editing => {
                self.handle_edit_key(*key_event);
                true
            }
            _ => false,
        }
    }

    /// Cursor position for the prompt line (when editing).
    ///
    /// The caller passes the full frame `area` and the computed top of the chat widget (`chat_top`)
    /// so we can position the cursor on the line directly above the composer.
    pub(crate) fn cursor_position(&self, area: Rect, chat_top: u16) -> Option<(u16, u16)> {
        if !self.editing || chat_top <= area.y {
            return None;
        }

        let prefix_w = "/ ".width() as u16;
        let query_w = self.query.width() as u16;
        let x = area
            .x
            .saturating_add(prefix_w)
            .saturating_add(query_w)
            .min(area.right().saturating_sub(1));
        let y = chat_top.saturating_sub(1);
        Some((x, y))
    }

    /// Update match state and apply any pending navigation request.
    ///
    /// Returns a stable transcript anchor `(cell_index, line_in_cell)` to be consumed by the
    /// transcript scroll state (e.g. to jump the viewport to the current match).
    ///
    /// Returns `None` when:
    /// - find is inactive, or
    /// - no navigation action is pending, or
    /// - the current match does not map to a stable anchor (e.g. no `CellLine` meta).
    pub(crate) fn on_render(
        &mut self,
        lines: &[Line<'static>],
        line_meta: &[TranscriptLineMeta],
        width: u16,
        preferred_line: usize,
    ) -> Option<(usize, usize)> {
        if !self.is_active() {
            return None;
        }

        self.ensure_up_to_date(lines, line_meta, width, preferred_line);
        self.apply_pending(preferred_line)
    }

    /// Render a transcript line with match highlighting (if active).
    ///
    /// Highlighting is computed on the flattened plain text of the line (see [`line_plain_text`])
    /// and then applied back onto the styled spans by splitting and patching styles.
    ///
    /// Styling conventions:
    /// - All matches: underlined
    /// - Current match: reversed + bold + underlined
    pub(crate) fn render_line(&self, line_index: usize, line: &Line<'_>) -> Line<'static> {
        if self.query.is_empty() {
            return line_to_static(line);
        }

        let indices = self.match_indices_for_line(line_index);
        if indices.is_empty() {
            return line_to_static(line);
        }

        let mut ranges: Vec<(Range<usize>, Style)> = Vec::with_capacity(indices.len());
        for idx in indices {
            let m = &self.matches[*idx];
            let style = if self.current_match == Some(*idx) {
                Style::new().reversed().bold().underlined()
            } else {
                Style::new().underlined()
            };
            ranges.push((m.range.clone(), style));
        }
        highlight_line(line, &ranges)
    }

    /// Render the prompt row shown above the composer while editing.
    pub(crate) fn render_prompt_line(&self) -> Option<Line<'static>> {
        if !self.editing {
            return None;
        }

        let (current, total) = self.match_summary();
        let mut spans: Vec<Span<'static>> = vec!["/ ".dim()];
        spans.push(self.query.clone().into());
        if !self.query.is_empty() {
            spans.push(format!("  {current}/{total}").dim());
        }
        Some(Line::from(spans))
    }

    /// Handle key events while editing the query.
    ///
    /// This is only called when [`Self::editing`] is true and the event wasn't handled by the
    /// top-level key bindings (e.g., `Ctrl-F`, `Ctrl-G`, `Esc`).
    fn handle_edit_key(&mut self, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Enter,
                kind: KeyEventKind::Press,
                ..
            } => {
                self.editing = false;
                self.set_pending(TranscriptFindPendingAction::Jump);
            }
            KeyEvent {
                code: KeyCode::Char('u'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                self.clear_query();
            }
            KeyEvent {
                code: KeyCode::Backspace,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                self.backspace();
            }
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } if !crate::key_hint::has_ctrl_or_alt(modifiers) => {
                self.push_char(c);
            }
            _ => {}
        }
    }

    /// Enter editing mode (idempotent).
    fn begin_edit(&mut self) {
        if self.editing {
            return;
        }
        self.editing = true;
    }

    /// Exit editing mode without clearing the query/highlights.
    fn end_edit(&mut self) {
        self.editing = false;
    }

    /// Clear all state, including query/highlights and cached match results.
    pub(crate) fn clear(&mut self) {
        self.query.clear();
        self.editing = false;
        self.last_width = None;
        self.last_lines_len = None;
        self.matches.clear();
        self.line_match_indices.clear();
        self.current_match = None;
        self.current_key = None;
        self.pending = None;
    }

    /// Clear the query text while leaving editing mode unchanged.
    ///
    /// This resets the cached width so matches will be recomputed when the user types again.
    fn clear_query(&mut self) {
        self.query.clear();
        self.last_width = None;
    }

    /// Remove a single character from the end of the query.
    fn backspace(&mut self) {
        if self.query.pop().is_some() {
            self.last_width = None;
        }
    }

    /// Append one character to the query.
    fn push_char(&mut self, ch: char) {
        self.query.push(ch);
        self.last_width = None;
    }

    /// Record a navigation request to be applied on the next render.
    fn set_pending(&mut self, pending: TranscriptFindPendingAction) {
        self.pending = Some(pending);
    }

    /// Recompute matches when the flattened transcript representation changes.
    ///
    /// This function is keyed on `width` and `lines.len()` to avoid work on every frame; callers
    /// should invoke [`TranscriptFind::note_lines_changed`] if content changes without affecting
    /// those keys.
    ///
    /// When recomputing:
    /// - we preserve the selected match using [`Self::current_key`]
    /// - we store a stable jump anchor (cell + line-in-cell) when available via
    ///   [`TranscriptLineMeta::cell_line`]
    fn ensure_up_to_date(
        &mut self,
        lines: &[Line<'static>],
        line_meta: &[TranscriptLineMeta],
        width: u16,
        preferred_line: usize,
    ) {
        // Fast path: empty query means there are no matches, and we should treat find as inactive
        // (even if `editing` is still true).
        if self.query.is_empty() {
            self.matches.clear();
            self.line_match_indices.clear();
            self.current_match = None;
            self.current_key = None;
            self.last_width = Some(width);
            self.last_lines_len = Some(lines.len());
            return;
        }

        // Cache key: if width and line count are unchanged, the flattened transcript representation
        // is assumed stable enough to reuse the previous match computation. Callers should use
        // `note_lines_changed()` when content changes without affecting these keys.
        if self.last_width == Some(width) && self.last_lines_len == Some(lines.len()) {
            return;
        }

        // Preserve selection across recompute by remembering the currently-selected match's
        // identity (line index + start byte offset in the flattened plain text).
        let current_key = self.current_key.take();
        self.matches.clear();
        self.line_match_indices = vec![Vec::new(); lines.len()];

        // Scan each flattened line, extracting plain text and recording match ranges. We keep:
        // - a global `matches` list in scan order for navigation (Ctrl-G)
        // - a per-line index list for rendering highlights efficiently
        for (line_index, line) in lines.iter().enumerate() {
            let plain = line_plain_text(line);
            let ranges = find_match_ranges(&plain, &self.query);
            for range in ranges {
                let idx = self.matches.len();
                // Only `CellLine` entries have stable anchors suitable for scroll jumps.
                let anchor = line_meta
                    .get(line_index)
                    .and_then(TranscriptLineMeta::cell_line);
                self.matches.push(TranscriptFindMatch {
                    line_index,
                    range: range.clone(),
                    anchor,
                });
                self.line_match_indices[line_index].push(idx);
            }
        }

        // Choose the current match:
        // 1) Prefer to keep the previous selection if it still exists in the new match set.
        // 2) Otherwise, pick the first match at/after the preferred line (top of the viewport).
        // 3) Otherwise, wrap to the first match.
        self.current_match = current_key
            .and_then(|(line_index, start)| {
                self.matches
                    .iter()
                    .position(|m| m.line_index == line_index && m.range.start == start)
            })
            .or_else(|| {
                self.matches
                    .iter()
                    .position(|m| m.line_index >= preferred_line)
                    .or_else(|| (!self.matches.is_empty()).then_some(0))
            });
        self.current_key = self.current_match.map(|i| {
            let m = &self.matches[i];
            (m.line_index, m.range.start)
        });

        self.last_width = Some(width);
        self.last_lines_len = Some(lines.len());
    }

    /// Apply a pending navigation action (if any) and return the anchor to scroll to.
    ///
    /// The returned `(cell_index, line_in_cell)` is derived from [`TranscriptLineMeta`] and is used
    /// by the caller to update the transcript scroll state.
    ///
    /// Note: the internal "current match" is updated even when an anchor is unavailable; in that
    /// case this returns `None` and the caller should not change scroll position.
    fn apply_pending(&mut self, preferred_line: usize) -> Option<(usize, usize)> {
        let pending = self.pending.take()?;
        if self.matches.is_empty() {
            self.current_match = None;
            self.current_key = None;
            return None;
        }

        match pending {
            TranscriptFindPendingAction::Jump => {
                if self.current_match.is_none() {
                    self.current_match = self
                        .matches
                        .iter()
                        .position(|m| m.line_index >= preferred_line)
                        .or_else(|| (!self.matches.is_empty()).then_some(0));
                }
            }
            TranscriptFindPendingAction::Next => {
                self.current_match = Some(match self.current_match {
                    Some(i) => (i + 1) % self.matches.len(),
                    None => 0,
                });
            }
        }

        self.current_key = self.current_match.map(|i| {
            let m = &self.matches[i];
            (m.line_index, m.range.start)
        });

        self.current_match.and_then(|i| self.matches[i].anchor)
    }

    /// Return indices into `matches` for the given flattened line index.
    ///
    /// Out-of-range indices return an empty slice to simplify call sites.
    fn match_indices_for_line(&self, line_index: usize) -> &[usize] {
        self.line_match_indices
            .get(line_index)
            .map_or(&[], |v| v.as_slice())
    }

    /// Return `(current, total)` match counts for the prompt display.
    ///
    /// `current` is 1-based (`0` when no match is selected), and `total` is the number of matches.
    fn match_summary(&self) -> (usize, usize) {
        let total = self.matches.len();
        let current = self.current_match.map(|i| i + 1).unwrap_or(0);
        (current, total)
    }
}

/// Convert a styled [`Line`] into plain text by concatenating its spans.
///
/// Find operates on plain text for match computation. Styling is applied later during
/// [`highlight_line`].
fn line_plain_text(line: &Line<'_>) -> String {
    let mut out = String::new();
    for span in &line.spans {
        out.push_str(span.content.as_ref());
    }
    out
}

/// Find all non-overlapping match ranges of `needle` within `haystack`.
///
/// Uses "smart-case" semantics:
/// - If `needle` contains any ASCII uppercase letters, matching is case-sensitive.
/// - Otherwise, both strings are compared in ASCII lowercase.
fn find_match_ranges(haystack: &str, needle: &str) -> Vec<Range<usize>> {
    if needle.is_empty() {
        return Vec::new();
    }
    let is_case_sensitive = needle.chars().any(|c| c.is_ascii_uppercase());
    if is_case_sensitive {
        find_match_ranges_exact(haystack, needle)
    } else {
        let haystack = haystack.to_ascii_lowercase();
        let needle = needle.to_ascii_lowercase();
        find_match_ranges_exact(&haystack, &needle)
    }
}

/// Find all non-overlapping match ranges of `needle` within `haystack` (case-sensitive).
///
/// Ranges are byte indices into the provided `haystack` string.
fn find_match_ranges_exact(haystack: &str, needle: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0usize;
    while start <= haystack.len() {
        let Some(rel) = haystack[start..].find(needle) else {
            break;
        };
        let abs = start + rel;
        let end = abs + needle.len();
        out.push(abs..end);
        start = end;
    }
    out
}

/// Apply highlighting styles to the provided `line`.
///
/// `ranges` is a list of `(byte_range, style)` pairs referring to byte offsets in the flattened
/// plain text of `line` (see [`line_plain_text`]). The implementation preserves existing span
/// styles by patching the requested highlight style onto each affected segment.
fn highlight_line(line: &Line<'_>, ranges: &[(Range<usize>, Style)]) -> Line<'static> {
    if ranges.is_empty() {
        return line_to_static(line);
    }

    // We treat `line` as a single flattened string (see `line_plain_text`) and map highlight byte
    // ranges back onto the individual styled spans. As we walk spans left-to-right we keep a
    // "global" byte cursor into the flattened text plus an index into `ranges`.
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut global_pos = 0usize;
    let mut range_idx = 0usize;

    for span in &line.spans {
        let text = span.content.as_ref();
        let span_start = global_pos;
        let span_end = span_start + text.len();
        global_pos = span_end;

        // Skip ranges that end before this span begins. This keeps `range_idx` pointing at the
        // first range that could possibly overlap the current span.
        while range_idx < ranges.len() && ranges[range_idx].0.end <= span_start {
            range_idx += 1;
        }

        // `local_pos` tracks how much of the current span we've emitted. We may need to split the
        // span into multiple segments as highlight ranges start/end within it.
        let mut local_pos = 0usize;
        while range_idx < ranges.len() {
            let (range, extra_style) = &ranges[range_idx];
            if range.start >= span_end {
                break;
            }

            // Clamp the global match range to this span's global extent.
            let start = range.start.max(span_start);
            let end = range.end.min(span_end);

            let start_local = start - span_start;
            // Emit any unhighlighted segment that appears before the match starts.
            if start_local > local_pos {
                out.push(Span::styled(
                    text[local_pos..start_local].to_string(),
                    span.style,
                ));
            }

            let end_local = end - span_start;
            // Emit the highlighted segment, preserving existing style by patching it with the
            // requested highlight style.
            out.push(Span::styled(
                text[start_local..end_local].to_string(),
                span.style.patch(*extra_style),
            ));
            local_pos = end_local;

            // Advance to the next range when we've consumed the entire range within this span.
            // When the range extends past this span, we keep `range_idx` pinned and continue the
            // highlighting into the next span.
            if range.end <= span_end {
                range_idx += 1;
            } else {
                break;
            }
        }

        // Emit any remaining tail of the span that doesn't intersect a highlight range.
        if local_pos < text.len() {
            out.push(Span::styled(text[local_pos..].to_string(), span.style));
        }
    }

    Line::from(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use pretty_assertions::assert_eq;

    /// Build `TranscriptLineMeta` for a single-cell transcript with `line_count` lines.
    fn meta_cell_lines(line_count: usize) -> Vec<TranscriptLineMeta> {
        (0..line_count)
            .map(|line_in_cell| TranscriptLineMeta::CellLine {
                cell_index: 0,
                line_in_cell,
            })
            .collect()
    }

    /// Convenience for a key press with no modifiers.
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// Convenience for a key press with the control modifier.
    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn smart_case_and_pending_jump() {
        let lines: Vec<Line<'static>> = vec![Line::from("hello World"), Line::from("second world")];
        let meta = vec![
            TranscriptLineMeta::CellLine {
                cell_index: 0,
                line_in_cell: 0,
            },
            TranscriptLineMeta::CellLine {
                cell_index: 0,
                line_in_cell: 1,
            },
        ];

        let mut find = TranscriptFind {
            query: "world".to_string(),
            ..Default::default()
        };
        assert_eq!(find.on_render(&lines, &meta, 80, 0), None);
        assert_eq!(find.matches.len(), 2);

        find.current_match = None;
        find.current_key = None;
        find.pending = Some(TranscriptFindPendingAction::Jump);
        let anchor = find.on_render(&lines, &meta, 80, 1);
        assert_eq!(anchor, Some((0, 1)));

        find.clear();
        find.query = "World".to_string();
        let _ = find.on_render(&lines, &meta, 80, 0);
        assert_eq!(
            find.matches
                .iter()
                .map(|m| m.line_index)
                .collect::<Vec<_>>(),
            vec![0]
        );
    }

    #[test]
    fn ctrl_f_enters_editing_and_renders_prompt() {
        let mut find = TranscriptFind::default();
        assert!(!find.is_active());

        assert!(find.handle_key_event(&ctrl(KeyCode::Char('f'))));
        assert!(find.is_active());
        assert!(find.is_visible());

        let prompt = find.render_prompt_line().expect("prompt line");
        assert_eq!(
            prompt
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<Vec<_>>(),
            vec!["/ ", ""]
        );
    }

    #[test]
    fn edit_keys_modify_query() {
        let mut find = TranscriptFind::default();
        let _ = find.handle_key_event(&ctrl(KeyCode::Char('f')));

        let _ = find.handle_key_event(&key(KeyCode::Char('a')));
        let _ = find.handle_key_event(&key(KeyCode::Char('b')));
        assert_eq!(find.query, "ab");

        let _ = find.handle_key_event(&key(KeyCode::Backspace));
        assert_eq!(find.query, "a");

        let _ = find.handle_key_event(&ctrl(KeyCode::Char('u')));
        assert_eq!(find.query, "");
    }

    #[test]
    fn enter_closes_prompt_and_requests_jump() {
        let lines: Vec<Line<'static>> = vec![Line::from("one two"), Line::from("two three")];
        let meta = meta_cell_lines(lines.len());

        let mut find = TranscriptFind::default();
        let _ = find.handle_key_event(&ctrl(KeyCode::Char('f')));
        for ch in "two".chars() {
            let _ = find.handle_key_event(&key(KeyCode::Char(ch)));
        }

        let _ = find.handle_key_event(&key(KeyCode::Enter));
        assert!(!find.is_visible());
        assert!(find.is_active());
        assert_eq!(find.render_prompt_line(), None);

        let anchor = find.on_render(&lines, &meta, 80, 0);
        assert_eq!(anchor, Some((0, 0)));
    }

    #[test]
    fn esc_closes_prompt_then_clears_query() {
        let mut find = TranscriptFind::default();
        let _ = find.handle_key_event(&ctrl(KeyCode::Char('f')));
        for ch in "two".chars() {
            let _ = find.handle_key_event(&key(KeyCode::Char(ch)));
        }

        let _ = find.handle_key_event(&key(KeyCode::Esc));
        assert!(!find.is_visible());
        assert!(find.is_active());
        assert_eq!(find.query, "two");

        let _ = find.handle_key_event(&key(KeyCode::Esc));
        assert!(!find.is_active());
        assert_eq!(find.query, "");
    }

    #[test]
    fn ctrl_g_cycles_matches_after_prompt_closed() {
        let lines: Vec<Line<'static>> =
            vec![Line::from("world world"), Line::from("another world")];
        let meta = meta_cell_lines(lines.len());

        let mut find = TranscriptFind::default();
        let _ = find.handle_key_event(&ctrl(KeyCode::Char('f')));
        for ch in "world".chars() {
            let _ = find.handle_key_event(&key(KeyCode::Char(ch)));
        }
        let _ = find.handle_key_event(&key(KeyCode::Enter));

        let a0 = find.on_render(&lines, &meta, 80, 0);
        assert_eq!(a0, Some((0, 0)));

        assert!(find.handle_key_event(&ctrl(KeyCode::Char('g'))));
        let a1 = find.on_render(&lines, &meta, 80, 0);
        assert_eq!(a1, Some((0, 0)));

        assert!(find.handle_key_event(&ctrl(KeyCode::Char('g'))));
        let a2 = find.on_render(&lines, &meta, 80, 0);
        assert_eq!(a2, Some((0, 1)));

        assert!(find.handle_key_event(&ctrl(KeyCode::Char('g'))));
        let a3 = find.on_render(&lines, &meta, 80, 0);
        assert_eq!(a3, Some((0, 0)));
    }

    #[test]
    fn note_lines_changed_forces_recompute() {
        let mut find = TranscriptFind {
            query: "hello".to_string(),
            ..Default::default()
        };

        let lines_v1 = vec![Line::from("hello")];
        let meta = meta_cell_lines(lines_v1.len());
        let _ = find.on_render(&lines_v1, &meta, 80, 0);
        assert_eq!(find.matches.len(), 1);

        find.note_lines_changed();
        assert_eq!(find.last_lines_len, None);

        let lines_v2 = vec![Line::from("world")];
        let _ = find.on_render(&lines_v2, &meta, 80, 0);
        assert_eq!(find.matches.len(), 0);
    }

    #[test]
    fn render_line_highlights_current_match_more_strongly() {
        let lines = vec![Line::from("aa")];
        let meta = meta_cell_lines(lines.len());

        let mut find = TranscriptFind {
            query: "a".to_string(),
            ..Default::default()
        };
        let _ = find.on_render(&lines, &meta, 80, 0);

        let rendered = find.render_line(0, &lines[0]);
        assert_eq!(
            rendered,
            Line::from(vec![
                Span::styled("a", Style::new().reversed().bold().underlined()),
                Span::styled("a", Style::new().underlined()),
            ])
        );
    }

    #[test]
    fn highlight_line_supports_ranges_across_span_boundaries() {
        let line = Line::from(vec!["hello ".into(), "world".into()]);
        let query = "o w";
        let ranges = find_match_ranges(&line_plain_text(&line), query);
        assert_eq!(ranges, vec![4..7]);

        let rendered = highlight_line(&line, &[(ranges[0].clone(), Style::new().underlined())]);
        assert_eq!(
            rendered,
            Line::from(vec![
                Span::styled("hell".to_string(), Style::default()),
                Span::styled("o ".to_string(), Style::new().underlined()),
                Span::styled("w".to_string(), Style::new().underlined()),
                Span::styled("orld".to_string(), Style::default()),
            ])
        );
    }

    #[test]
    fn cursor_position_clamps_to_prompt_line_width() {
        let mut find = TranscriptFind {
            query: "abcdef".to_string(),
            editing: true,
            ..Default::default()
        };

        let area = Rect::new(10, 5, 5, 5);
        let (x, y) = find.cursor_position(area, 8).expect("cursor");
        assert_eq!(y, 7);
        assert_eq!(x, area.right().saturating_sub(1));
        find.clear();
    }

    #[test]
    fn ctrl_g_is_ignored_when_inactive() {
        let mut find = TranscriptFind::default();
        assert!(!find.handle_key_event(&ctrl(KeyCode::Char('g'))));
    }
}
