//! Tab display and source-coordinate regressions for owned transcript rows.

use super::TextLayout;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::prefix_hyperlink_lines;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Stylize;
use ratatui::text::Line;

#[test]
fn styled_tabs_use_row_stops_including_the_gutter() {
    let mut line = HyperlinkLine::new(Line::from("\ttabs:\t".green()));
    line.push_span("one".cyan(), Some("https://example.com/one"));
    line.push_span("\t".magenta().underlined(), /*destination*/ None);
    line.push_span("two".cyan(), Some("https://example.com/two"));
    let lines = prefix_hyperlink_lines(vec![line], "› ".bold(), "  ".into());
    let layout = TextLayout::new(lines, /*width*/ 32);
    let area = Rect::new(
        /*x*/ 3, /*y*/ 2, /*width*/ 32, /*height*/ 1,
    );
    let mut buffer = Buffer::empty(area);
    layout.render(area, &mut buffer, /*start_row*/ 0);
    assert_eq!(layout.text(), "\ttabs:\tone\ttwo");
    assert_eq!(
        [0, 1, 6, 7, 10, 11].map(|offset| layout.column_for_offset(offset)),
        [2, 8, 13, 16, 19, 24],
    );
    assert_eq!(
        [2, 7, 13, 15, 19, 23].map(|column| layout.position_at(/*row*/ 0, column)),
        [0, 0, 6, 6, 10, 10],
    );
    assert_eq!(
        (
            layout.link_at(/*row*/ 0, /*column*/ 16).as_deref(),
            layout.link_at(/*row*/ 0, /*column*/ 24).as_deref(),
            layout.link_at(/*row*/ 0, /*column*/ 13).as_deref()
        ),
        (
            Some("https://example.com/one"),
            Some("https://example.com/two"),
            None
        ),
    );
    assert_eq!(
        (
            buffer[(area.x + 3, area.y)].fg,
            buffer[(area.x + 20, area.y)].fg
        ),
        (Color::Green, Color::Magenta),
    );
    layout.highlight(6..7, area, &mut buffer, /*start_row*/ 0);
    assert_eq!(
        (0..32)
            .filter(|column| buffer[(area.x + column, area.y)]
                .modifier
                .contains(Modifier::REVERSED))
            .collect::<Vec<_>>(),
        vec![13, 14, 15],
    );
    assert_eq!(&layout.text()[6..7], "\t");
    insta::assert_snapshot!("styled_tab_stops_and_selection", format!("{buffer:?}"));
}

#[test]
fn tab_stops_restart_after_wrapping_and_resize_preserves_source_and_links() {
    let mut line = HyperlinkLine::new(Line::from("abcdefghi\t".green()));
    line.push_span("X".cyan(), Some("https://example.com/x"));
    line.push_span("\tYZ".magenta(), /*destination*/ None);
    let lines = prefix_hyperlink_lines(vec![line], "› ".bold(), "  ".into());
    let layout = TextLayout::new(lines, /*width*/ 10);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 10, /*height*/ 3,
    );
    let mut buffer = Buffer::empty(area);
    layout.render(area, &mut buffer, /*start_row*/ 0);
    assert_eq!(layout.text(), "abcdefghi\tX\tYZ");
    assert_eq!(layout.row_count(), 3);
    assert_eq!(
        [9, 10, 12].map(|offset| (
            layout.row_for_offset(offset),
            layout.column_for_offset(offset)
        )),
        [(1, 3), (1, 8), (2, 2)],
    );
    assert_eq!(
        layout.link_at(/*row*/ 1, /*column*/ 8).as_deref(),
        Some("https://example.com/x")
    );
    let wide = layout.rewrap(/*width*/ 40);
    assert_eq!((wide.text(), wide.row_count()), (layout.text(), 1));
    let restored = wide.rewrap(/*width*/ 10);
    let mut restored_buffer = Buffer::empty(area);
    restored.render(area, &mut restored_buffer, /*start_row*/ 0);
    assert_eq!(restored_buffer, buffer);
}

#[test]
fn leading_and_repeated_tabs_remain_selectable_below_one_tab_stop() {
    for width in 1..8 {
        let layout = TextLayout::new(vec!["\t\t界e\u{301}\tX".into()], width);
        assert_eq!(layout.text(), "\t\t界e\u{301}\tX");
        assert_eq!(layout.position_at(/*row*/ 0, width - 1), 0);
        assert_eq!(layout.position_at(/*row*/ 1, width - 1), 1);
        let offset = layout.text().find('X').unwrap();
        assert_eq!(
            layout.position_at(
                layout.row_for_offset(offset),
                layout.column_for_offset(offset)
            ),
            offset
        );
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, layout.row_count() as u16);
        let mut buffer = Buffer::empty(area);
        layout.render(area, &mut buffer, /*start_row*/ 0);
        let visible = buffer
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        assert!(visible.contains('X'), "width {width}: {buffer:?}");
        layout.highlight(0..2, area, &mut buffer, /*start_row*/ 0);
        assert!((0..width).all(|column| buffer[(column, 0)].modifier.contains(Modifier::REVERSED)));
    }
}

#[test]
fn ordinary_spaces_before_tabs_affect_the_stop_without_entering_the_gutter() {
    let layout = TextLayout::new(vec!["ab   \tX\t\tY".into()], /*width*/ 32);
    assert_eq!(layout.text(), "ab   \tX\t\tY");
    assert_eq!(
        [5, 6, 7, 8, 9].map(|offset| layout.column_for_offset(offset)),
        [5, 8, 9, 16, 24]
    );
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 32, /*height*/ 1,
    );
    let mut buffer = Buffer::empty(area);
    layout.render(area, &mut buffer, /*start_row*/ 0);
    assert_eq!(
        (buffer[(8, 0)].symbol(), buffer[(24, 0)].symbol()),
        ("X", "Y")
    );
}

#[test]
fn hard_wrapped_tabs_keep_source_characters_and_actual_row_stops() {
    let mut line = HyperlinkLine::new(Line::from("abcde\tX\tY"));
    let mut source = crate::terminal_hyperlinks::LogicalLineSource::from_line(&line.line);
    source.wrap_policy = crate::terminal_hyperlinks::LineWrapPolicy::Hard;
    line.source = Some(source);
    let layout = TextLayout::new(vec![line], /*width*/ 4);
    assert_eq!(layout.text(), "abcde\tX\tY");
    assert_eq!(
        (layout.row_count(), layout.row_for_offset(/*offset*/ 8)),
        (4, 3)
    );
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 4, /*height*/ 4,
    );
    let mut buffer = Buffer::empty(area);
    layout.render(area, &mut buffer, /*start_row*/ 0);
    insta::assert_snapshot!("hard_wrapped_tabs", format!("{buffer:?}"));
}
