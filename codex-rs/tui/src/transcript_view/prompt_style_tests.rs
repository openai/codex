//! Full-row prompt styles and source-free wrapping margins survive viewport painting and resize.

use super::TextLayout;
use crate::history_cell::HistoryCell;
use crate::history_cell::new_user_prompt;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::HyperlinkParagraph;
use crate::terminal_hyperlinks::LogicalLineSource;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Widget;

#[test]
fn visible_rows_fill_inherited_styles_without_painting_outside_the_viewport() {
    let style = Style::default().green().on_dark_gray().italic();
    let layout = TextLayout::new(
        vec![
            HyperlinkLine::from("offscreen"),
            HyperlinkLine::new(Line::from("").style(style)),
            HyperlinkLine::new(Line::from(vec!["A".bold(), "界".cyan()]).style(style)),
            HyperlinkLine::new(Line::from("").style(style)),
        ],
        /*width*/ 9,
    );
    let outer = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 13, /*height*/ 6,
    );
    let viewport = Rect::new(
        /*x*/ 2, /*y*/ 1, /*width*/ 9, /*height*/ 4,
    );
    let mut actual = Buffer::empty(outer);
    layout.render(viewport, &mut actual, /*start_row*/ 1);

    let mut expected = Buffer::empty(outer);
    expected.set_style(
        Rect::new(
            /*x*/ 2, /*y*/ 1, /*width*/ 9, /*height*/ 3,
        ),
        style,
    );
    expected[(2, 2)].set_symbol("A").set_style(style.bold());
    expected[(3, 2)].set_symbol("界").set_style(style.cyan());
    assert_eq!(actual, expected);
}

#[test]
fn prompt_margin_and_blank_background_survive_resize_without_changing_copy() {
    let message =
        "1234567890123456789012345678901234567890123456789012345678\n\n  界e\u{301}👩‍💻 trailing";
    let cell = new_user_prompt(message.into(), Vec::new(), Vec::new(), Vec::new());
    // Fix the inherited style without changing the process-wide terminal palette.
    let style = Style::default().on_dark_gray();
    let initial_lines = styled_prompt_lines(&cell, /*width*/ 60, style);
    let mut layout = TextLayout::new(initial_lines, /*width*/ 60);
    let expected_copy = format!("\n{message}\n");

    for (name, width) in [
        ("prompt_margin_wide", 60),
        ("prompt_margin_narrow", 20),
        ("prompt_margin_restored", 60),
    ] {
        layout = layout.rewrap(width);
        let fresh_lines = styled_prompt_lines(&cell, width, style);
        let area = Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            width,
            fresh_lines.len().try_into().expect("short prompt"),
        );
        let mut actual = Buffer::empty(area);
        layout.render(area, &mut actual, /*start_row*/ 0);
        let mut expected = Buffer::empty(area);
        HyperlinkParagraph::new(&fresh_lines, style).render(area, &mut expected);

        assert_eq!(
            (layout.text(), layout.row_count()),
            (expected_copy.as_str(), fresh_lines.len()),
        );
        assert_eq!(actual, expected);
        assert_eq!(
            (0..area.height)
                .map(|row| actual[(width - 1, row)].symbol())
                .collect::<Vec<_>>(),
            vec![" "; fresh_lines.len()],
        );
        if width == 20 {
            insta::assert_snapshot!(name, format!("{actual:?}"));
        }
    }

    // The last source character on the first body row precedes the reserved column.
    assert_eq!(
        (
            layout.position_at(/*row*/ 1, /*column*/ 58),
            layout.position_at(/*row*/ 1, /*column*/ 59),
            layout.position_at(/*row*/ 2, /*column*/ 2),
        ),
        (57, 58, 58),
    );
}

#[test]
fn reserved_margin_uses_the_same_alignment_for_painting_and_selection() {
    let style = Style::default().on_dark_gray();
    let mut line = HyperlinkLine::new(Line::from("ab").right_aligned().style(style));
    let mut source = LogicalLineSource::from_line(&line.line);
    source.right_reserve = 2;
    line.source = Some(source);
    let layout = TextLayout::new(vec![line], /*width*/ 8);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 8, /*height*/ 1,
    );
    let mut actual = Buffer::empty(area);
    layout.render(area, &mut actual, /*start_row*/ 0);
    layout.highlight(0..2, area, &mut actual, /*start_row*/ 0);

    let mut expected = Buffer::empty(area);
    expected.set_style(area, style);
    expected.set_string(/*x*/ 4, /*y*/ 0, "ab", style.reversed());
    assert_eq!(actual, expected);
    assert_eq!(
        (0..8)
            .map(|column| layout.position_at(/*row*/ 0, column))
            .collect::<Vec<_>>(),
        vec![0, 0, 0, 0, 0, 1, 2, 2],
    );
}

fn styled_prompt_lines(cell: &dyn HistoryCell, width: u16, style: Style) -> Vec<HyperlinkLine> {
    cell.display_hyperlink_lines(width)
        .into_iter()
        .map(|mut line| {
            line.line.style = style;
            line
        })
        .collect()
}
