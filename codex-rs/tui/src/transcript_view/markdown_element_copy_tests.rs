//! Coverage of the Markdown grammar and visible transformations used by selection copy.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn supported_elements_preserve_structure_at_narrow_and_wide_widths() {
    let samples = [
        "# One\n\n## Two\n\n### Three\n\n#### Four\n\n##### Five\n\n###### Six",
        "Setext heading\n===\n\nSubheading\n---",
        "**strong** _emphasis_ ~~deleted~~ and `inline_code` with ``a`b``.",
        "Escaped \\*stars\\*, &amp; entity, café 界, and 👩‍💻.",
        "- first\n- second\n  - nested\n\n3. third\n4. fourth",
        "- [ ] pending\n- [x] complete",
        "Before\n\n> quoted **text**\n>\n> - item\n>   - nested\n\nAfter",
        "First paragraph.\n\nHard break:  \nnext line.\n\n---\n\nLast paragraph.",
        "Prose\n\n```\nlet x = \"*literal* <tag>\";\n```\n\nAfter",
        "Image ![**alt** text](https://example.com/image.png).",
        "Literal <b>HTML</b> and unsupported [^footnote].",
    ];
    for source in samples {
        for width in [18, 100] {
            let layout = markdown_layout(source, width);
            let copied = payload(&layout, 0..layout.text().len()).0;
            assert_eq!(
                crate::clipboard_html::render_markdown(&copied),
                crate::clipboard_html::render_markdown(source),
                "width {width}: {source}\n{copied}"
            );
        }
    }
}

#[test]
fn transformed_content_copies_visible_text_without_active_html_or_images() {
    for (source, expected) in [
        ("Soft break\nsame paragraph.", "Soft break\nsame paragraph."),
        ("[**local**](/repo/file.rs)", "**local** (repo/file.rs)"),
        (
            "[label][reference]\n\n[reference]: https://example.com",
            "[label (https://example.com)](https://example.com)",
        ),
        (
            "<https://example.com>",
            "[https://example.com (https://example.com)](https://example.com)",
        ),
        ("![image alt](https://example.com/image.png)", "image alt"),
    ] {
        let layout = markdown_layout(source, /*width*/ 40);
        let copied = payload(&layout, 0..layout.text().len()).0;
        assert_eq!(
            crate::clipboard_html::render_markdown(&copied),
            crate::clipboard_html::render_markdown(expected),
            "{source}\n{copied}"
        );
    }
}

#[test]
fn math_and_mermaid_copy_the_visible_rendering() {
    for source in [
        "Inline $\\alpha_1$ and \\(x^{2}\\).",
        "\\[\n\\frac{a}{b}\n\\]",
        "```mermaid\nflowchart LR\nA[Start] --> B[Finish]\n```",
    ] {
        let layout = markdown_layout(source, /*width*/ 80);
        let (copied, format) = payload(&layout, 0..layout.text().len());
        if format == CopyFormat::PlainText {
            assert_eq!(copied, layout.text());
        } else {
            let rendered = markdown_layout(&copied, /*width*/ 80);
            assert_eq!(rendered.text(), layout.text());
        }
    }
}

#[test]
fn blockquote_selection_copies_only_selected_content() {
    let mut copies = Vec::new();
    for (source, selected) in [
        ("> quoted text", "quoted text"),
        ("> quoted text", "quoted"),
        ("> quoted text", "text"),
        ("> first line\n> second line", "first line\nsecond line"),
        (
            "> first paragraph\n>\n> second paragraph",
            "first paragraph\n\nsecond paragraph",
        ),
        (
            "> outer\n>> nested café界\n>\n> outer again",
            "outer\n\nnested café界\n\nouter again",
        ),
        ("> **bold** and `code`", "bold and code"),
        ("> **bold**\n>\n> - [x] task", "bold\n\n"),
        ("> **bold**\n>\n> | A |\n> |---|\n> | cell |", "bold\n\n"),
        (
            "> literal \\> marker and a > b",
            "literal > marker and a > b",
        ),
        ("Before\n\n> quoted text\n\nAfter", "quoted text"),
        ("Before\n\n> quoted text\n\nAfter", "quoted text\n"),
        ("Before\n\n> quoted text\n\nAfter", "\nquoted text"),
        (
            "> - first\n>   continuation\n> - second",
            "first\ncontinuation\n\nsecond",
        ),
    ] {
        for width in [18, 80] {
            let layout = markdown_layout(source, width);
            let start = layout.text().find(selected).expect(source);
            let copied = payload(&layout, start..start + selected.len());
            assert_eq!(copied, (selected.into(), CopyFormat::PlainText), "{source}");
            assert_eq!(
                payload(&layout.rewrap(/*width*/ 12), start..start + selected.len()),
                copied
            );
            if width == 80 && copies.len() < 3 {
                copies.push(format!("{source}\nSelected {selected:?} -> {copied:?}"));
            }
        }
    }
    insta::assert_snapshot!(copies.join("\n"), @r#"
    > quoted text
    Selected "quoted text" -> ("quoted text", PlainText)
    > quoted text
    Selected "quoted" -> ("quoted", PlainText)
    > quoted text
    Selected "text" -> ("text", PlainText)
    "#);
}

#[test]
fn quoted_tables_reconstruct_wrapped_cell_values() {
    let table = "| Path |\n|---|\n| `/long/path/to/a/file/with_a_long_name.rs` |";
    for prefix in ["> ", "> > "] {
        let source = table
            .lines()
            .map(|line| format!("{prefix}{line}"))
            .collect::<Vec<_>>()
            .join("\n");
        for width in [18, 80] {
            let layout = markdown_layout(&source, width);
            let copied = payload(&layout, 0..layout.text().len());
            assert_eq!(copied, (table.into(), CopyFormat::Markdown));
            let start = layout.text().find("/long").unwrap();
            let end = layout.text().rfind(".rs").unwrap() + 3;
            let cell = payload(&layout, start..end);
            assert_eq!(
                cell,
                (
                    "/long/path/to/a/file/with_a_long_name.rs".into(),
                    CopyFormat::PlainText
                )
            );
            let rewrapped = layout.rewrap(/*width*/ 12);
            assert_eq!(payload(&rewrapped, start..end), cell);
            assert_eq!(payload(&rewrapped, 0..rewrapped.text().len()), copied);
        }
    }
}

#[test]
fn quoted_task_lists_preserve_checkbox_states() {
    for (source, expected) in [
        (
            "> - [x] Tests passed\n> - [ ] Deploy",
            "- [x] Tests passed\n- [ ] Deploy",
        ),
        (
            "> > 1. [ ] Tests passed\n> > 2. [x] Deploy",
            "1. [ ] Tests passed\n2. [x] Deploy",
        ),
        ("> - [x] \n> - ordinary", "- [x]\n- ordinary"),
        ("> - ordinary\n> - [ ] ", "- ordinary\n- [ ]"),
    ] {
        for width in [18, 80] {
            let layout = markdown_layout(source, width);
            let copied = payload(&layout, 0..layout.text().len());
            assert_eq!(copied, (expected.into(), CopyFormat::Markdown));
            let rewrapped = layout.rewrap(/*width*/ 12);
            assert_eq!(payload(&rewrapped, 0..rewrapped.text().len()), copied);
            if width == 80 && source.starts_with("> - [x] Tests") {
                insta::assert_snapshot!(copied.0, @"
                - [x] Tests passed
                - [ ] Deploy
                ");
            }
        }
    }
}

#[test]
fn mouse_selected_blockquote_copies_without_markers() {
    use crossterm::event::KeyModifiers;
    use crossterm::event::MouseButton;
    use crossterm::event::MouseEvent;
    use crossterm::event::MouseEventKind;

    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(AgentMarkdownCell::new(
        "> quoted text".into(),
        Path::new("/"),
    ))];
    let mut view = TranscriptView::default();
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 32, /*height*/ 10,
    );
    view.render(area, &mut Buffer::empty(area), &cells);
    for (kind, column) in [
        (MouseEventKind::Down(MouseButton::Left), 0),
        (MouseEventKind::Drag(MouseButton::Left), 31),
    ] {
        view.handle_mouse(
            MouseEvent {
                kind,
                column,
                row: 0,
                modifiers: KeyModifiers::NONE,
            },
            &cells,
        );
    }
    let plain = view.selected_text(&cells).unwrap();
    view.copy_selected_text_with(
        &cells,
        &plain,
        /*clear_selection*/ true,
        |text, format| {
            assert_eq!((text, format), ("quoted text", CopyFormat::PlainText));
            Ok(crate::clipboard_copy::CopyStatus::Confirmed)
        },
    )
    .unwrap();
}
