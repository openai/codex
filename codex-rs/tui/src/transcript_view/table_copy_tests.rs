//! Selection regressions through the real Markdown renderer, layout, and clipboard serializer.

use super::*;
use crate::transcript_view::tests::render;
use pretty_assertions::assert_eq;

const TABLE: &str = "| Task | Status | Next step |\n|---|---|---|\n| Task 1 | Complete | Review results |\n| Task 2 | In progress | Finish implementation |\n| Task 3 | Planned | Confirm requirements |";

fn symbol_position(buffer: &Buffer, symbol: &str) -> (u16, u16) {
    let index = buffer
        .content()
        .iter()
        .position(|cell| cell.symbol() == symbol)
        .unwrap();
    buffer.pos_of(index)
}

fn table_cells(markdown: impl Into<String>) -> [Arc<dyn HistoryCell>; 1] {
    [Arc::new(AgentMarkdownCell::new(
        markdown.into(),
        Path::new("/"),
    ))]
}

#[test]
fn diagonal_cell_drag_in_stacked_records_copies_both_complete_rows() {
    let first_two = "| Left | Right |\n|---|---|\n| alpha | Beta |\n| Gamma | delta |";
    let source = format!("{first_two}\n| omega | last |");
    let expected = "|  |  |\n|---|---|\n| alpha | Beta |\n| Gamma | delta |";
    let cells = table_cells(&source);
    let mut view = TranscriptView::default();
    let buffer = render(&mut view, &cells, /*width*/ 10, /*height*/ 24);
    let (right, left) = (symbol_position(&buffer, "B"), symbol_position(&buffer, "G"));
    for ((x, y), (to_x, to_y)) in [(right, left), (left, right)] {
        view.begin_selection(&cells, x, y, /*clicks*/ 3);
        view.extend_selection(to_x, to_y);
        assert_eq!(view.selected_text(&cells).as_deref(), Some(expected));
    }

    let first = symbol_position(&buffer, "R");
    let last_label = buffer
        .content()
        .iter()
        .rposition(|cell| cell.symbol() == "R");
    let third = buffer.pos_of(last_label.unwrap());
    assert_ne!(first, third);
    view.begin_selection(&cells, left.0, left.1, /*clicks*/ 3);
    for ((x, y), expected) in [
        (first, first_two),
        (
            third,
            "| Left | Right |\n|---|---|\n| Gamma | delta |\n| omega | last |",
        ),
    ] {
        view.extend_selection(x, y);
        assert_eq!(view.selected_text(&cells).as_deref(), Some(expected));
    }
    let (x, y) = symbol_position(&buffer, "a");
    view.begin_selection(&cells, x, y, /*clicks*/ 1);
    view.extend_selection(third.0, third.1);
    assert_eq!(view.selected_text(&cells).as_deref(), Some(source.as_str()));
}

#[test]
fn table_drag_falls_back_to_text_and_keeps_the_release_endpoint() {
    let cells =
        table_cells("| A | B |\n|---|---|\n| alpha | Beta |\n| Gamma | delta |\n\nParagraph.");
    let mut view = TranscriptView::default();
    let buffer = render(&mut view, &cells, /*width*/ 40, /*height*/ 14);
    let (x, y) = symbol_position(&buffer, "d");
    let (period, last) = symbol_position(&buffer, ".");
    view.begin_selection(&cells, x, y, /*clicks*/ 1);
    view.extend_selection(period + 1, last);
    let text = view.selected_text(&cells).unwrap();
    assert!(
        text.starts_with("delta") && text.ends_with("Paragraph."),
        "{text:?}"
    );

    let (left, top) = symbol_position(&buffer, "a");
    view.begin_selection(&cells, x, y, /*clicks*/ 1);
    view.extend_selection(left, top);
    view.end_drag();
    assert!(view.selection_key(&cells, crossterm::event::KeyCode::Right));
    let text = view.selected_text(&cells).unwrap();
    assert!(text.starts_with("lpha"));
    assert!(text.ends_with("delta"));
}

#[test]
fn rectangular_clipboard_text_escapes_literal_pipes() {
    let cells = table_cells("| A | B |\n|---|---|\n| a\\|b | Value |");
    let mut view = TranscriptView::default();
    let buffer = render(&mut view, &cells, /*width*/ 40, /*height*/ 10);
    let (x, y) = symbol_position(&buffer, "a");
    let (right, row) = symbol_position(&buffer, "V");
    view.begin_selection(&cells, x, y, /*clicks*/ 1);
    view.extend_selection(right, row);
    let text = view.selected_text(&cells).unwrap();
    let expected = "|  |  |\n|---|---|\n| a\\|b | Value |";
    assert_eq!(text, expected);
    view.copy_selected_text_with(
        &cells,
        &text,
        /*clear_selection*/ false,
        |plain, format| {
            assert_eq!(
                (plain, format),
                (expected, CopyFormat::MarkdownSelection(expected.into()))
            );
            Ok(crate::clipboard_copy::CopyStatus::Confirmed)
        },
    )
    .unwrap();
}

#[test]
fn table_copy_preserves_structure_across_grid_and_record_layouts() {
    let table = TABLE.replace("Finish implementation", "Finish my_long_file_name!.txt");
    let source = format!(
        "The project is moving forward, with one task complete and two still to finish.\n\n{table}\n\nThe next priority is to finish Task 2 while gathering the requirements needed to begin Task 3."
    );
    let markdown = source.replace("my_long_file_name!.txt", r"my\_long\_file\_name\!.txt");
    for width in [12, 24, 40, 80, 120] {
        let layout = markdown_layout(&source, width);
        let mut lines = Vec::new();
        layout.copy_lines(0..layout.text().len(), "", &[], &mut lines);
        assert_eq!(
            payload(&layout, 0..layout.text().len()),
            (markdown.clone(), CopyFormat::Markdown),
            "width {width}"
        );
        assert_eq!(
            crate::markdown_copy::literal_selection(&lines, "unused", /*rectangle_rows*/ None),
            source
        );
        let rewrapped = layout.rewrap(/*width*/ 17);
        assert_eq!(
            payload(&rewrapped, 0..rewrapped.text().len()).0,
            markdown.clone(),
            "rewrapped {width}"
        );
    }
}

#[test]
fn partial_table_selection_never_adds_unselected_cell_text() {
    let code = markdown_layout(
        "| A | B |\n|---|---|\n| `a\\|b` | value |",
        /*width*/ 80,
    );
    let start = code.text().find("a|b").unwrap();
    assert_eq!(
        payload(&code, start..start + 3),
        ("a|b".into(), CopyFormat::PlainText)
    );
    assert_eq!(
        payload(&code, start + 1..start + 3),
        ("|b".into(), CopyFormat::PlainText)
    );
    let mixed = markdown_layout(
        "Before\n\n| `a\\|b` | B |\n|---|---|\n| left | right |",
        /*width*/ 80,
    );
    let end = mixed.text().find("a|b").unwrap() + "a|b".len();
    assert_eq!(
        payload(&mixed, 0..end),
        ("Before\n\n`a|b`".into(), CopyFormat::Markdown)
    );
    let controls = markdown_layout(
        "| A | B |\n|---|---|\n| `a\u{7}\\|b` | value |",
        /*width*/ 80,
    );
    let selected = "a\u{7}|b";
    let start = controls.text().find(selected).unwrap();
    assert_eq!(
        payload(&controls, start..start + selected.len()),
        ("a|b".into(), CopyFormat::PlainText)
    );
    let repeated = markdown_layout(
        "| alpha SECRET omega |\n|---|\n| long_value_one |\n| long_value_two |",
        /*width*/ 12,
    );
    let start = repeated.text().find("omega").unwrap();
    let end = repeated.text().rfind("alpha").unwrap() + "alpha".len();
    assert!(!payload(&repeated, start..end).0.contains("SECRET"));
    assert!(
        payload(&repeated, start..repeated.text().len())
            .0
            .starts_with("| alpha SECRET omega |")
    );
    let layout = markdown_layout(
        "| Label | Value |\n|---|---|\n| **alpha** | beta |\n| gamma | delta |",
        /*width*/ 80,
    );
    let start = layout.text().find("alpha").unwrap();
    assert_eq!(
        payload(&layout, start + 1..start + 4),
        ("**lph**".into(), CopyFormat::Markdown)
    );
    let end = layout.text().find("delta").unwrap() + "delta".len();
    let copied = payload(&layout, start..end).0;
    assert_eq!(
        copied,
        "|  |  |\n|---|---|\n| **alpha** | beta |\n| gamma | delta |"
    );
}

#[test]
fn table_cells_keep_inline_formatting_alignment_and_literal_pipes() {
    let source = "| Left | Center | Right |\n|:---|:---:|---:|\n| **bold** _italic_ ~~strike~~ | `a\\|b` | [link](<https://example.com/?a=1&b=2>) |\n| café 界 | escaped \\*star\\* | last |";
    let expected_html = crate::clipboard_html::render_markdown(
        &source.replace("[link]", "[link (https://example.com/?a=1&b=2)]"),
    );
    for width in [16, 48, 120] {
        let layout = markdown_layout(source, width);
        let copied = payload(&layout, 0..layout.text().len()).0;
        assert_eq!(
            crate::clipboard_html::render_markdown(&copied),
            expected_html,
            "width {width}: {copied}"
        );
    }
}

#[test]
fn triple_click_below_a_short_grid_header_selects_its_text() {
    let source = "| ID | A longer heading that wraps across many display lines |\n|---|---|";
    let cells = table_cells(source);
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 30, /*height*/ 8);
    view.begin_selection(&cells, /*column*/ 3, /*row*/ 2, /*clicks*/ 3);
    assert_eq!(view.selected_text(&cells).as_deref(), Some("ID"));
}

#[test]
fn header_only_and_empty_cells_keep_their_table_positions() {
    for source in [
        "| A | B |\n|---|---|",
        "| A |\n|---|",
        "| A | B |\n|---|---|\n|  | right |\n|  |  |\n| left |  |",
        "| A | B |\n|---|---|\n| `   ` | value |",
    ] {
        for width in [4, 80] {
            let layout = markdown_layout(source, width);
            let copied = payload(&layout, 0..layout.text().len()).0;
            assert_eq!(
                crate::clipboard_html::render_markdown(&copied),
                crate::clipboard_html::render_markdown(source),
                "width {width}: {copied}"
            );
        }
    }
}

#[test]
fn separate_tables_and_code_remain_separate_blocks() {
    let source = "Before\n\n| A | B |\n|---|---|\n| one | two |\n\n```text\n| literal | code |\n```\n\n| C | D |\n|---|---|\n| three | four |\n\nAfter";
    let layout = markdown_layout(source, /*width*/ 80);
    let copied = payload(&layout, 0..layout.text().len()).0;
    assert_eq!(copied, source.replace("```text", "```"));
}

#[test]
fn table_separator_only_selection_keeps_visible_text() {
    for width in [12, 80] {
        let layout = markdown_layout(TABLE, width);
        let mut offset = 0;
        let mut checked = false;
        for line in layout.text().split('\n') {
            if line.contains('─') || line.contains('━') {
                assert_eq!(payload(&layout, offset..offset + line.len()).0, line);
                checked = true;
            }
            offset += line.len() + 1;
        }
        assert!(checked, "no separator at width {width}");
    }
}

#[test]
fn empty_selected_rows_keep_copy_spacing_without_newline_highlights() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(AgentMarkdownCell::new(
        format!("Before\n\n{TABLE}\n\nAfter"),
        Path::new("/"),
    ))];
    let mut view = TranscriptView::default();
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 20,
    );
    let mut buffer = Buffer::empty(area);
    view.render(area, &mut buffer, &cells);
    view.begin_selection(&cells, /*column*/ 2, /*row*/ 0, /*clicks*/ 1);
    view.extend_selection(/*column*/ 79, /*row*/ 19);
    view.render(area, &mut buffer, &cells);
    let mut highlighted = Vec::new();
    for row in 0..area.height {
        let text = (0..area.width)
            .map(|column| buffer[(column, row)].symbol())
            .collect::<String>();
        let selected = (0..area.width)
            .map(|column| {
                if buffer[(column, row)]
                    .modifier
                    .contains(ratatui::style::Modifier::REVERSED)
                {
                    '^'
                } else {
                    ' '
                }
            })
            .collect::<String>();
        if text.trim().is_empty() {
            assert!(selected.trim().is_empty(), "blank row {row}");
        }
        highlighted.push(format!("{}\n{}", text.trim_end(), selected.trim_end()));
    }
    let plain = view.selected_text(&cells).unwrap();
    view.copy_selected_text_with(
        &cells,
        &plain,
        /*clear_selection*/ false,
        |text, format| {
            assert_eq!(
                (text, format),
                (
                    format!("Before\n\n{TABLE}\n\nAfter").as_str(),
                    CopyFormat::MarkdownSelection(format!("Before\n\n{TABLE}\n\nAfter").into())
                )
            );
            Ok(crate::clipboard_copy::CopyStatus::Confirmed)
        },
    )
    .unwrap();
    insta::assert_snapshot!(highlighted.join("\n"));
}

#[test]
fn mouse_selects_wrapped_cell_text_and_rectangles() {
    use crate::transcript_view::tests::text;
    use crossterm::event::KeyModifiers;
    use crossterm::event::MouseButton::Left;
    use crossterm::event::MouseEvent;
    use crossterm::event::MouseEventKind;

    let rich =
        "Adds **cell selection** for a viewer who reads the room history with 界 characters.";
    let full = rich.replace("**", "");
    let table_header = "|  |  |  |\n|---|---|---:|";
    let markdown = format!(
        "| PR | What it adds | Size |\n|---|---|---:|\n| First change | {rich} | +332 |\n| Pending |  |  |\n| Awaiting |  | TBD |\n| Next change | Connects the reader. | +434 |"
    );
    let cells = table_cells(markdown);
    let mut view = TranscriptView {
        copy_on_select: true,
        ..Default::default()
    };
    let buffer = render(&mut view, &cells, /*width*/ 60, /*height*/ 12);
    let position = |symbol| symbol_position(&buffer, symbol);
    let start @ (first, first_row) = position("A");
    let end @ (period, last_row) = position(".");
    let highlighted_rows = |view: &mut TranscriptView, last: u16| {
        let buffer = render(view, &cells, /*width*/ 60, /*height*/ 12);
        let display = text(&buffer);
        let mut rows = Vec::new();
        for (row, line) in (first_row..=last).zip(display.lines().skip(first_row.into())) {
            let mut mask = String::new();
            for x in 0..buffer.area.width {
                let selected = buffer[(x, row)].modifier.contains(Modifier::REVERSED);
                mask.push(if selected { '^' } else { ' ' });
            }
            rows.push(format!("{line}\n{}", mask.trim_end()));
        }
        rows.join("\n")
    };
    let mouse = |view: &mut TranscriptView, kind, (column, row)| {
        view.handle_mouse(
            MouseEvent {
                kind,
                column,
                row,
                modifiers: KeyModifiers::NONE,
            },
            &cells,
        )
    };
    let two_cells = format!("{table_header}\n|  | {full} | +332 |");
    let suffix = full.strip_prefix("Adds ").unwrap();
    let partial = ((first + 5, first_row), end, suffix);
    for (from, to, expected) in [
        (position("+"), end, two_cells.as_str()),
        partial,
        partial, // Repeating a drag from the same spot starts a fresh selection.
        ((first - 2, first_row), end, full.as_str()),
        (end, start, full.as_str()),
        ((period + 5, last_row), start, full.as_str()),
    ] {
        mouse(&mut view, MouseEventKind::Down(Left), from);
        mouse(&mut view, MouseEventKind::Drag(Left), to);
        let Some(ViewAction::CopyOnSelect(copied)) = mouse(&mut view, MouseEventKind::Up(Left), to)
        else {
            panic!("expected a copy on release for {from:?} → {to:?}");
        };
        assert_eq!(copied, expected, "{from:?} → {to:?}");
    }
    let highlight = highlighted_rows(&mut view, last_row);
    insta::assert_snapshot!("mouse_drag_selects_only_the_wrapped_table_cell", highlight);

    // The short size cell owns its column beside the wrapped description.
    view.begin_selection(&cells, position("+").0, last_row, /*clicks*/ 3);
    assert_eq!(view.selected_text(&cells).as_deref(), Some("+332"));

    // The third press selects the cell immediately. Hold it to grow or shrink the rectangle.
    let down = MouseEventKind::Down(Left);
    let up = MouseEventKind::Up(Left);
    for kind in [down, up, down, up, down] {
        mouse(&mut view, kind, (period + 1, last_row));
    }
    assert_eq!(view.selected_text(&cells).as_deref(), Some(full.as_str()));
    assert_eq!(highlighted_rows(&mut view, last_row), highlight);
    let size_column = position("+").0;
    let final_row = position("4").1;
    let pending_row = text(&buffer)
        .lines()
        .position(|line| line.contains("Pending"))
        .unwrap() as u16;
    let pending_description = format!("{table_header}\n|  | {full} |  |\n|  |  |  |");
    let pending_size = format!("{two_cells}\n|  |  |  |");
    let rectangle =
        format!("{two_cells}\n|  |  |  |\n|  |  | TBD |\n|  | Connects the reader. | +434 |");
    let with_empty_endpoint = format!("{table_header}\n|  | {full} |  |\n|  |  |  |\n|  |  |  |");
    let description_column = format!("{with_empty_endpoint}\n|  | Connects the reader. |  |");
    let blank = (first, position("T").1);
    for (to, expected) in [
        ((size_column, last_row), two_cells.as_str()), // Blank space below +332.
        ((first, pending_row), pending_description.as_str()),
        ((size_column, pending_row), pending_size.as_str()),
        ((size_column, final_row), rectangle.as_str()),
        ((first, final_row), description_column.as_str()),
        (blank, with_empty_endpoint.as_str()),
        (end, full.as_str()),
    ] {
        mouse(&mut view, MouseEventKind::Drag(Left), to);
        assert_eq!(view.selected_text(&cells).as_deref(), Some(expected));
        if expected == rectangle {
            let rectangle_highlight = highlighted_rows(&mut view, final_row);
            insta::assert_snapshot!(rectangle_highlight);
        }
    }
    mouse(&mut view, up, end);

    render(&mut view, &cells, /*width*/ 32, /*height*/ 12);
    assert_eq!(view.selected_text(&cells).as_deref(), Some(full.as_str()));
    view.copy_selected_text_with(
        &cells,
        &full,
        /*clear_selection*/ false,
        |text, format| {
            assert_eq!(
                (text, format),
                (full.as_str(), CopyFormat::MarkdownSelection(rich.into()))
            );
            Ok(crate::clipboard_copy::CopyStatus::Confirmed)
        },
    )
    .unwrap();

    render(&mut view, &cells, /*width*/ 60, /*height*/ 12);
    view.begin_selection(&cells, first, final_row, /*clicks*/ 3);
    view.extend_selection(blank.0, blank.1);
    assert_eq!(
        view.selected_text(&cells),
        Some(format!(
            "{table_header}\n|  |  |  |\n|  | Connects the reader. |  |"
        ))
    );

    view.begin_selection(&cells, first, pending_row, /*clicks*/ 3);
    view.extend_selection(size_column, pending_row);
    assert_eq!(
        view.selected_text(&cells),
        Some(format!("{table_header}\n|  |  |  |"))
    );

    view.begin_selection(&cells, first, first_row, /*clicks*/ 1);
    view.extend_selection(first, pending_row);
    assert_eq!(
        view.selected_text(&cells).as_deref(),
        Some(pending_description.as_str())
    );
}
