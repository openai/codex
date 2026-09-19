//! Render the live mention frame at full, clipped and narrow dimensions.

use super::*;
use pretty_assertions::assert_eq;

fn rows() -> Vec<SearchResult> {
    let mut rows = (0..10)
        .map(|index| SearchResult {
            display_name: format!("Plugin {index:02}"),
            description: Some("Tools and workflows".to_string()),
            mention_type: MentionType::Plugin,
            selection: Selection::Tool {
                insert_text: format!("plugin-{index}"),
                path: None,
            },
            match_indices: Some(vec![0]),
            score: 0,
        })
        .collect::<Vec<_>>();
    for name in ["src/界.rs", "src/plain.rs"] {
        rows.push(SearchResult {
            display_name: name.to_string(),
            description: None,
            mention_type: MentionType::File,
            selection: Selection::File(name.into()),
            match_indices: None,
            score: 0,
        });
    }
    rows
}

fn text(buffer: &Buffer) -> String {
    (buffer.area.y..buffer.area.bottom())
        .map(|y| {
            let mut line = String::new();
            let mut x = buffer.area.x;
            while x < buffer.area.right() {
                let symbol = buffer[(x, y)].symbol();
                line.push_str(symbol);
                x += UnicodeWidthStr::width(symbol).max(/*other*/ 1) as u16;
            }
            line.trim_end().to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn live_picker_frame_snapshots() {
    let rows = rows();
    for (name, width, height, selected) in [
        ("wide", 80, POPUP_HEIGHT, 0),
        ("narrow_scrolled", 40, POPUP_HEIGHT, rows.len() - 1),
        ("short", 40, 7, rows.len() - 1),
        ("one_row", 40, 1, rows.len() - 1),
    ] {
        let mut buffer = Buffer::empty(Rect::new(/*x*/ 0, /*y*/ 0, width, height));
        render_popup(
            buffer.area,
            &mut buffer,
            &rows,
            &ScrollState {
                selected_idx: Some(selected),
                scroll_top: 0,
            },
            "no matches",
            SearchMode::Results,
            "",
        );
        insta::assert_snapshot!(name, text(&buffer));
    }
}

#[test]
fn selection_covers_the_row_and_empty_filter_clears_previous_results() {
    let mut buffer = Buffer::empty(Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        /*width*/ 80,
        POPUP_HEIGHT,
    ));
    render_popup(
        buffer.area,
        &mut buffer,
        &rows(),
        &ScrollState {
            selected_idx: Some(0),
            scroll_top: 0,
        },
        "no matches",
        SearchMode::Results,
        "",
    );
    let selected_y = (0..buffer.area.height)
        .find(|y| buffer[(0, *y)].symbol() == "›")
        .expect("selected result");
    let selected_styles = (0..buffer.area.width)
        .map(|x| {
            let cell = &buffer[(x, selected_y)];
            (cell.fg, cell.bg, cell.modifier)
        })
        .collect::<Vec<_>>();
    let style = selection_style();
    let expected = (
        style.fg.unwrap_or(Color::Reset),
        style.bg.unwrap_or(Color::Reset),
        style.add_modifier,
    );
    assert_eq!(
        selected_styles,
        vec![expected; usize::from(buffer.area.width)]
    );

    render_popup(
        buffer.area,
        &mut buffer,
        &[],
        &ScrollState::new(),
        "loading...",
        SearchMode::FilesystemOnly,
        "new-query",
    );
    insta::assert_snapshot!("empty_filter", text(&buffer));
    assert!(buffer.content.iter().all(|cell| {
        cell.bg == Color::Reset
            || cell.bg
                == crate::bottom_pane::picker_style::active_tab_style()
                    .bg
                    .unwrap_or(Color::Reset)
    }));
}

#[test]
fn filesystem_columns_use_terminal_cell_widths() {
    let rows = rows()
        .into_iter()
        .filter(|row| row.mention_type == MentionType::File)
        .collect::<Vec<_>>();
    let mut buffer = Buffer::empty(Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 60, /*height*/ 2,
    ));
    render_rows(
        buffer.area,
        &mut buffer,
        &rows,
        &ScrollState::new(),
        "no matches",
    );
    let columns = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width - 3)
                .find(|x| {
                    (0..4)
                        .map(|offset| buffer[(*x + offset, y)].symbol())
                        .collect::<String>()
                        == "src/"
                })
                .expect("parent path")
        })
        .collect::<Vec<_>>();
    assert_eq!(columns, vec![12, 12]);
}

#[test]
fn long_mentions_leave_room_for_distinguishing_parent_paths() {
    let mut rows = rows();
    rows[0].display_name = "Long off-screen plugin name ".repeat(/*n*/ 16);
    for name in [
        "src/界.rs",
        "tests/界.rs",
        "deep/a_very_long_filename_that_exceeds_the_column.rs",
    ] {
        rows.push(SearchResult {
            display_name: name.to_string(),
            description: None,
            mention_type: MentionType::File,
            selection: Selection::File(name.into()),
            match_indices: None,
            score: 0,
        });
    }
    let first_file = rows.len() - 3;
    for (name, width) in [
        ("bounded_filesystem_columns_narrow", 40),
        ("bounded_filesystem_columns_wide", 80),
    ] {
        let mut buffer = Buffer::empty(Rect::new(
            /*x*/ 0, /*y*/ 0, width, /*height*/ 3,
        ));
        render_rows(
            buffer.area,
            &mut buffer,
            &rows,
            &ScrollState {
                selected_idx: Some(first_file),
                scroll_top: first_file,
            },
            "no matches",
        );
        insta::assert_snapshot!(name, text(&buffer));
    }
}
