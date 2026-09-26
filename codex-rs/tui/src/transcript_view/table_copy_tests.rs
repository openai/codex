//! Selection regressions through the real Markdown renderer, layout, and clipboard serializer.

use super::*;
use pretty_assertions::assert_eq;

const TABLE: &str = "| Task | Status | Next step |\n|---|---|---|\n| Task 1 | Complete | Review results |\n| Task 2 | In progress | Finish implementation |\n| Task 3 | Planned | Confirm requirements |";

#[test]
fn table_copy_preserves_structure_across_grid_and_record_layouts() {
    let source = format!(
        "The project is moving forward, with one task complete and two still to finish.\n\n{TABLE}\n\nThe next priority is to finish Task 2 while gathering the requirements needed to begin Task 3."
    );
    for width in [12, 24, 40, 80, 120] {
        let layout = markdown_layout(&source, width);
        assert_eq!(
            payload(&layout, 0..layout.text().len()),
            (source.clone(), CopyFormat::Markdown),
            "width {width}"
        );
        let rewrapped = layout.rewrap(/*width*/ 17);
        assert_eq!(
            payload(&rewrapped, 0..rewrapped.text().len()).0,
            source,
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
    assert_eq!(payload(&code, start..start + 3).0, "`a|b`");
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
                    CopyFormat::Markdown
                )
            );
            Ok(crate::clipboard_copy::CopyStatus::Confirmed)
        },
    )
    .unwrap();
    insta::assert_snapshot!(highlighted.join("\n"));
}
