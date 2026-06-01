use crate::history_cell::HistoryCell;
use crate::history_cell::HistoryRenderMode;
use crate::wrapping::wrap_ranges_trim;
use ratatui::text::Line;
use unicode_width::UnicodeWidthChar;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SearchMatch {
    pub(crate) renderable_index: usize,
    pub(crate) line_index: usize,
    pub(crate) scroll_line_index: usize,
    pub(crate) start_col: u16,
    pub(crate) end_col: u16,
    pub(crate) owning_user_prompt_cell: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SearchCorpusKey {
    pub(crate) width: u16,
    pub(crate) render_mode: HistoryRenderMode,
    pub(crate) revision: u64,
}

pub(crate) struct SearchCorpus {
    key: SearchCorpusKey,
    lines: Vec<SearchLine>,
}

impl SearchCorpus {
    pub(crate) fn new(key: SearchCorpusKey, lines: Vec<SearchLine>) -> Self {
        Self { key, lines }
    }

    pub(crate) fn matches_key(&self, key: SearchCorpusKey) -> bool {
        self.key == key
    }

    pub(crate) fn find_matches(&self, query: &str) -> Vec<SearchMatch> {
        self.lines
            .iter()
            .flat_map(|line| line.find_matches(query))
            .collect()
    }
}

pub(crate) struct SearchLine {
    renderable_index: usize,
    line_index: usize,
    scroll_line_index: usize,
    width: u16,
    folded_text: String,
    plain_text: String,
    display_cols_by_byte: Vec<u16>,
    source_bytes_by_folded_byte: Vec<usize>,
    owning_user_prompt_cell: Option<usize>,
}

impl SearchLine {
    pub(crate) fn from_line(
        renderable_index: usize,
        line_index: usize,
        scroll_line_index: usize,
        width: u16,
        line: &Line<'static>,
        owning_user_prompt_cell: Option<usize>,
    ) -> Self {
        let plain_text = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        Self {
            renderable_index,
            line_index,
            scroll_line_index,
            width,
            folded_text: folded_text(&plain_text),
            display_cols_by_byte: display_cols_by_folded_byte(&plain_text),
            source_bytes_by_folded_byte: source_bytes_by_folded_byte(&plain_text),
            plain_text,
            owning_user_prompt_cell,
        }
    }

    fn find_matches(&self, query: &str) -> Vec<SearchMatch> {
        if query.is_empty() {
            return Vec::new();
        }
        let mut matches = Vec::new();
        let mut start = 0usize;
        while let Some(found) = self.folded_text[start..].find(query) {
            let match_start = start.saturating_add(found);
            let match_end = match_start.saturating_add(query.len());
            matches.push(SearchMatch {
                renderable_index: self.renderable_index,
                line_index: self.line_index,
                scroll_line_index: self
                    .scroll_line_index
                    .saturating_add(self.wrapped_row(match_start)),
                start_col: self.display_col(match_start),
                end_col: self.display_col(match_end),
                owning_user_prompt_cell: self.owning_user_prompt_cell,
            });
            let Some(ch) = self.folded_text[match_start..].chars().next() else {
                break;
            };
            start = match_start.saturating_add(ch.len_utf8());
        }
        matches
    }

    fn display_col(&self, byte_index: usize) -> u16 {
        self.display_cols_by_byte
            .get(byte_index)
            .copied()
            .unwrap_or_default()
    }

    fn wrapped_row(&self, folded_byte_index: usize) -> usize {
        let source_byte_index = self
            .source_bytes_by_folded_byte
            .get(folded_byte_index)
            .copied()
            .unwrap_or(self.plain_text.len());
        wrap_ranges_trim(&self.plain_text, usize::from(self.width.max(/*other*/ 1)))
            .iter()
            .position(|range| range.contains(&source_byte_index))
            .unwrap_or_default()
    }
}

pub(crate) fn transcript_search_lines(
    cells: &[std::sync::Arc<dyn HistoryCell>],
    live_tail_lines: Option<&[Line<'static>]>,
    render_mode: HistoryRenderMode,
    width: u16,
    rendered_line_height: impl Fn(&Line<'static>, u16) -> usize,
    live_tail_has_top_padding: bool,
) -> Vec<SearchLine> {
    let mut search_lines = Vec::new();
    let mut owner_user_prompt = None;
    for (idx, cell) in cells.iter().enumerate() {
        if cell.is_user_prompt() {
            owner_user_prompt = Some(idx);
        }
        let top_padding = usize::from(!cell.is_stream_continuation() && idx > 0);
        let lines = cell.transcript_lines_for_mode(width, render_mode);
        push_search_lines(
            &mut search_lines,
            idx,
            &lines,
            top_padding,
            owner_user_prompt,
            width,
            &rendered_line_height,
        );
    }

    if let Some(lines) = live_tail_lines {
        push_search_lines(
            &mut search_lines,
            cells.len(),
            lines,
            usize::from(live_tail_has_top_padding),
            owner_user_prompt,
            width,
            &rendered_line_height,
        );
    }

    search_lines
}

fn push_search_lines(
    search_lines: &mut Vec<SearchLine>,
    renderable_index: usize,
    lines: &[Line<'static>],
    top_padding: usize,
    owner_user_prompt: Option<usize>,
    width: u16,
    rendered_line_height: &impl Fn(&Line<'static>, u16) -> usize,
) {
    let mut scroll_line_index = top_padding;
    for (line_index, line) in lines.iter().enumerate() {
        search_lines.push(SearchLine::from_line(
            renderable_index,
            line_index,
            scroll_line_index,
            width,
            line,
            owner_user_prompt,
        ));
        scroll_line_index = scroll_line_index.saturating_add(rendered_line_height(line, width));
    }
}

fn folded_text(text: &str) -> String {
    text.chars().flat_map(char::to_lowercase).collect()
}

fn display_cols_by_folded_byte(text: &str) -> Vec<u16> {
    let mut cols = vec![0];
    let mut col = 0u16;
    for ch in text.chars() {
        let next_col =
            col.saturating_add(u16::try_from(ch.width().unwrap_or(0)).unwrap_or(u16::MAX));
        cols.extend(std::iter::repeat_n(
            next_col,
            ch.to_lowercase().map(char::len_utf8).sum(),
        ));
        col = next_col;
    }
    cols
}

fn source_bytes_by_folded_byte(text: &str) -> Vec<usize> {
    let mut source_bytes = vec![0];
    for (source_byte_index, ch) in text.char_indices() {
        let next_source_byte = source_byte_index.saturating_add(ch.len_utf8());
        source_bytes.extend(std::iter::repeat_n(
            next_source_byte,
            ch.to_lowercase().map(char::len_utf8).sum(),
        ));
    }
    source_bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn search_match_iteration_advances_on_utf8_boundaries() {
        let line = SearchLine::from_line(
            /*renderable_index*/ 0,
            /*line_index*/ 0,
            /*scroll_line_index*/ 0,
            /*width*/ 80,
            &Line::from("éé"),
            /*owning_user_prompt_cell*/ None,
        );

        assert_eq!(line.find_matches("é").len(), 2);
    }

    #[test]
    fn search_match_scroll_offset_uses_word_wrapping() {
        let line = SearchLine::from_line(
            /*renderable_index*/ 0,
            /*line_index*/ 0,
            /*scroll_line_index*/ 0,
            /*width*/ 10,
            &Line::from("aaaaa aaaaa needle"),
            /*owning_user_prompt_cell*/ None,
        );

        assert_eq!(line.find_matches("needle")[0].scroll_line_index, 2);
    }
}
