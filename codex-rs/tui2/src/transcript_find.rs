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
pub(crate) struct TranscriptFind {
    query: String,
    editing: bool,
    last_width: Option<u16>,
    last_lines_len: Option<usize>,
    matches: Vec<TranscriptFindMatch>,
    line_match_indices: Vec<Vec<usize>>,
    current_match: Option<usize>,
    current_key: Option<(usize, usize)>,
    pending: Option<TranscriptFindPendingAction>,
}

#[derive(Debug, Clone, Copy)]
enum TranscriptFindPendingAction {
    Jump,
    Next,
}

#[derive(Debug, Clone)]
struct TranscriptFindMatch {
    line_index: usize,
    range: Range<usize>,
    anchor: Option<(usize, usize)>,
}

impl TranscriptFind {
    pub(crate) fn is_active(&self) -> bool {
        self.editing || !self.query.is_empty()
    }

    pub(crate) fn is_visible(&self) -> bool {
        self.editing
    }

    pub(crate) fn note_lines_changed(&mut self) {
        if self.is_active() {
            self.last_lines_len = None;
        }
    }

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

    fn begin_edit(&mut self) {
        if self.editing {
            return;
        }
        self.editing = true;
    }

    fn end_edit(&mut self) {
        self.editing = false;
    }

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

    fn clear_query(&mut self) {
        self.query.clear();
        self.last_width = None;
    }

    fn backspace(&mut self) {
        if self.query.pop().is_some() {
            self.last_width = None;
        }
    }

    fn push_char(&mut self, ch: char) {
        self.query.push(ch);
        self.last_width = None;
    }

    fn set_pending(&mut self, pending: TranscriptFindPendingAction) {
        self.pending = Some(pending);
    }

    fn ensure_up_to_date(
        &mut self,
        lines: &[Line<'static>],
        line_meta: &[TranscriptLineMeta],
        width: u16,
        preferred_line: usize,
    ) {
        if self.query.is_empty() {
            self.matches.clear();
            self.line_match_indices.clear();
            self.current_match = None;
            self.current_key = None;
            self.last_width = Some(width);
            self.last_lines_len = Some(lines.len());
            return;
        }

        if self.last_width == Some(width) && self.last_lines_len == Some(lines.len()) {
            return;
        }

        let current_key = self.current_key.take();
        self.matches.clear();
        self.line_match_indices = vec![Vec::new(); lines.len()];

        for (line_index, line) in lines.iter().enumerate() {
            let plain = line_plain_text(line);
            let ranges = find_match_ranges(&plain, &self.query);
            for range in ranges {
                let idx = self.matches.len();
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

    fn match_indices_for_line(&self, line_index: usize) -> &[usize] {
        self.line_match_indices
            .get(line_index)
            .map_or(&[], |v| v.as_slice())
    }

    fn match_summary(&self) -> (usize, usize) {
        let total = self.matches.len();
        let current = self.current_match.map(|i| i + 1).unwrap_or(0);
        (current, total)
    }
}

fn line_plain_text(line: &Line<'_>) -> String {
    let mut out = String::new();
    for span in &line.spans {
        out.push_str(span.content.as_ref());
    }
    out
}

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

fn highlight_line(line: &Line<'_>, ranges: &[(Range<usize>, Style)]) -> Line<'static> {
    if ranges.is_empty() {
        return line_to_static(line);
    }

    let mut out: Vec<Span<'static>> = Vec::new();
    let mut global_pos = 0usize;
    let mut range_idx = 0usize;

    for span in &line.spans {
        let text = span.content.as_ref();
        let span_start = global_pos;
        let span_end = span_start + text.len();
        global_pos = span_end;

        while range_idx < ranges.len() && ranges[range_idx].0.end <= span_start {
            range_idx += 1;
        }

        let mut local_pos = 0usize;
        while range_idx < ranges.len() {
            let (range, extra_style) = &ranges[range_idx];
            if range.start >= span_end {
                break;
            }

            let start = range.start.max(span_start);
            let end = range.end.min(span_end);

            let start_local = start - span_start;
            if start_local > local_pos {
                out.push(Span::styled(
                    text[local_pos..start_local].to_string(),
                    span.style,
                ));
            }

            let end_local = end - span_start;
            out.push(Span::styled(
                text[start_local..end_local].to_string(),
                span.style.patch(*extra_style),
            ));
            local_pos = end_local;

            if range.end <= span_end {
                range_idx += 1;
            } else {
                break;
            }
        }

        if local_pos < text.len() {
            out.push(Span::styled(text[local_pos..].to_string(), span.style));
        }
    }

    Line::from(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn transcript_find_smart_case_and_pending_jump() {
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
        assert_eq!(find.matches.len(), 1);
        assert_eq!(find.matches[0].line_index, 0);
    }
}
