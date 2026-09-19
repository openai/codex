//! Display file completions while preserving the full path used for insertion.
//! Narrow rows retain the filename; fuzzy highlights refer to the displayed text.

use std::path::PathBuf;

use codex_file_search::FileMatch;
use codex_utils_fuzzy_match::fuzzy_match;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::WidgetRef;

use super::picker_rows::render_rows_single_line;
use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::text_formatting::center_truncate_path;
use crate::width::display_width;

/// Visual state for the file-search popup.
pub(crate) struct FileSearchPopup {
    /// Query corresponding to the `matches` currently shown.
    display_query: String,
    /// Latest query typed by the user. May differ from `display_query` when
    /// a search is still in-flight.
    pending_query: String,
    /// When `true` we are still waiting for results for `pending_query`.
    waiting: bool,
    /// Cached matches; paths relative to the search dir.
    matches: Vec<FileMatch>,
    /// Shared selection/scroll state.
    state: ScrollState,
}

impl FileSearchPopup {
    pub(crate) fn new() -> Self {
        Self {
            display_query: String::new(),
            pending_query: String::new(),
            waiting: true,
            matches: Vec::new(),
            state: ScrollState::new(),
        }
    }

    /// Update the query and reset state to *waiting*.
    pub(crate) fn set_query(&mut self, query: &str) {
        if query == self.pending_query {
            return;
        }

        self.pending_query.clear();
        self.pending_query.push_str(query);

        self.waiting = true; // waiting for new results
    }

    /// Put the popup into an "idle" state used for an empty query (just "@").
    /// Shows a hint instead of matches until the user types more characters.
    pub(crate) fn set_empty_prompt(&mut self) {
        self.display_query.clear();
        self.pending_query.clear();
        self.waiting = false;
        self.matches.clear();
        // Reset selection/scroll state when showing the empty prompt.
        self.state.reset();
    }

    /// Replace matches when a `FileSearchResult` arrives.
    /// Replace matches. Only applied when `query` matches `pending_query`.
    pub(crate) fn set_matches(&mut self, query: &str, matches: Vec<FileMatch>) {
        if query != self.pending_query {
            return; // stale
        }

        self.display_query = query.to_string();
        self.matches = matches.into_iter().take(MAX_POPUP_ROWS).collect();
        self.waiting = false;
        let len = self.matches.len();
        self.state.clamp_selection(len);
        self.state.ensure_visible(len, len.min(MAX_POPUP_ROWS));
    }

    /// Move selection cursor up.
    pub(crate) fn move_up(&mut self) {
        let len = self.matches.len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, len.min(MAX_POPUP_ROWS));
    }

    /// Move selection cursor down.
    pub(crate) fn move_down(&mut self) {
        let len = self.matches.len();
        self.state.move_down_wrap(len);
        self.state.ensure_visible(len, len.min(MAX_POPUP_ROWS));
    }

    pub(crate) fn selected_match(&self) -> Option<&PathBuf> {
        self.state
            .selected_idx
            .and_then(|idx| self.matches.get(idx))
            .map(|file_match| &file_match.path)
    }

    pub(crate) fn calculate_required_height(&self) -> u16 {
        // Row count depends on whether we already have matches. If no matches
        // yet (e.g. initial search or query with no results) reserve a single
        // row so the popup is still visible. When matches are present we show
        // up to MAX_RESULTS regardless of the waiting flag so the list
        // remains stable while a newer search is in-flight.

        self.matches.len().clamp(/*min*/ 1, MAX_POPUP_ROWS) as u16 + 2
    }
}

impl WidgetRef for &FileSearchPopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        // Convert matches to GenericDisplayRow, translating indices to usize at the UI boundary.
        let rows_all: Vec<GenericDisplayRow> = if self.matches.is_empty() {
            Vec::new()
        } else {
            self.matches
                .iter()
                .enumerate()
                .map(|(idx, m)| {
                    let path = m.path.to_string_lossy();
                    let width = usize::from(area.width.saturating_sub(/*rhs*/ 2));
                    let name = if display_width(&path) <= width {
                        path.to_string()
                    } else {
                        let filename = m
                            .path
                            .file_name()
                            .unwrap_or(m.path.as_os_str())
                            .to_string_lossy();
                        let filename_width = display_width(&filename);
                        if let Some(parent) = m.path.parent()
                            && filename_width + 2 <= width
                        {
                            let parent = truncate_line_with_ellipsis_if_overflow(
                                Line::from(parent.to_string_lossy().into_owned()),
                                width - filename_width - 1,
                            );
                            format!("{parent}{}{filename}", std::path::MAIN_SEPARATOR)
                        } else {
                            center_truncate_path(&filename, width)
                        }
                    };
                    let match_indices = if name == path {
                        m.indices
                            .as_ref()
                            .map(|indices| indices.iter().map(|&index| index as usize).collect())
                    } else {
                        fuzzy_match(&name, &self.display_query).map(|(indices, _)| indices)
                    };
                    GenericDisplayRow {
                        category_tag: None,
                        selection_style: Some(super::picker_style::selection_style()),
                        name,
                        name_prefix_spans: vec![
                            if self.state.selected_idx == Some(idx) {
                                "› "
                            } else {
                                "  "
                            }
                            .into(),
                        ],
                        match_indices,
                        display_shortcut: None,
                        description: None,
                        wrap_indent: None,
                        is_disabled: false,
                        disabled_reason: None,
                    }
                })
                .collect()
        };

        let empty_message = if self.waiting {
            "  loading..."
        } else {
            "  no matches"
        };

        render_rows_single_line(
            area,
            buf,
            &rows_all,
            &self.state,
            MAX_POPUP_ROWS,
            empty_message,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_file_search::MatchType;
    use pretty_assertions::assert_eq;

    fn file_match(index: usize) -> FileMatch {
        FileMatch {
            score: index as u32,
            path: PathBuf::from(format!("src/file_{index:02}.rs")),
            match_type: MatchType::File,
            root: PathBuf::from("/tmp/repo"),
            indices: None,
        }
    }

    #[test]
    fn long_paths_keep_distinguishing_filenames_and_original_selection() {
        let mut snapshots = Vec::new();
        for components in [
            vec!["src", "shared", "long_directory"],
            vec!["long_directory_name_that_fills_row"],
            vec!["長いディレクトリ名", "e\u{301}tudes"],
        ] {
            let parent: PathBuf = components.iter().collect();
            let paths: Vec<PathBuf> = ["parser_alpha.rs", "parser_beta.rs"]
                .into_iter()
                .map(|name| parent.join(name))
                .collect();
            let mut popup = FileSearchPopup::new();
            popup.set_query("parser");
            popup.set_matches(
                "parser",
                paths
                    .iter()
                    .enumerate()
                    .map(|(index, path)| {
                        let mut matched = file_match(index);
                        matched.path = path.clone();
                        matched.indices =
                            fuzzy_match(&path.to_string_lossy(), "parser").map(|(indices, _)| {
                                indices.into_iter().map(|index| index as u32).collect()
                            });
                        matched
                    })
                    .collect(),
            );
            for (selected, path) in paths.iter().enumerate() {
                for width in [28, 40, 80] {
                    let area = Rect::new(
                        /*x*/ 0,
                        /*y*/ 0,
                        width,
                        popup.calculate_required_height(),
                    );
                    let mut buf = Buffer::empty(area);
                    (&popup).render_ref(area, &mut buf);
                    let text = buf
                        .content
                        .chunks(usize::from(width))
                        .map(|row| {
                            row.iter()
                                .map(ratatui::buffer::Cell::symbol)
                                .collect::<String>()
                                .trim_end()
                                .replace(std::path::MAIN_SEPARATOR, "/")
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    assert!(
                        text.contains("parser_alpha.rs") && text.contains("parser_beta.rs"),
                        "{text}"
                    );
                    assert_eq!(popup.selected_match(), Some(path));
                    let unselected_y = if selected == 0 { 2 } else { 1 };
                    let parser_x = (0..width.saturating_sub(/*rhs*/ 5))
                        .find(|&x| {
                            (x..x + 6)
                                .map(|column| buf[(column, unselected_y)].symbol())
                                .eq(["p", "a", "r", "s", "e", "r"])
                        })
                        .expect("visible filename match");
                    assert!(
                        buf[(parser_x, unselected_y)]
                            .modifier
                            .contains(ratatui::style::Modifier::BOLD)
                    );
                    if selected == 0 {
                        snapshots.push(format!("{components:?}, width {width}\n{text}"));
                    }
                }
                popup.move_down();
            }
        }
        insta::assert_snapshot!(snapshots.join("\n\n"));
    }

    #[test]
    fn set_matches_keeps_only_the_first_page_of_results() {
        let mut popup = FileSearchPopup::new();
        popup.set_query("file");
        popup.set_matches("file", (0..(MAX_POPUP_ROWS + 2)).map(file_match).collect());

        assert_eq!(
            popup.matches,
            (0..MAX_POPUP_ROWS).map(file_match).collect::<Vec<_>>()
        );
        assert_eq!(popup.calculate_required_height(), MAX_POPUP_ROWS as u16 + 2);
    }
}
