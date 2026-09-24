use super::*;
use crate::clipboard_copy::CopyFormat;
use crate::history_cell::AgentMarkdownCell;
use crate::history_cell::HistoryCell;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::path::Path;

#[test]
fn selected_markdown_preserves_lists_and_inline_code() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(AgentMarkdownCell::new(
        "- setting `closeRequested_`,\n- removing the tailer from `tailers_`,\n- clearing its live/discovery state.".into(),
        Path::new("/"),
    ))];
    let mut view = TranscriptView::default();
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 32, /*height*/ 24,
    );
    view.render(area, &mut Buffer::empty(area), &cells);
    // Exercise the public gesture path rather than manufacturing metadata.
    use crossterm::event::KeyModifiers;
    use crossterm::event::MouseButton;
    use crossterm::event::MouseEvent;
    use crossterm::event::MouseEventKind;
    view.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: 0,
            modifiers: KeyModifiers::NONE,
        },
        &cells,
    );
    view.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 31,
            row: 23,
            modifiers: KeyModifiers::NONE,
        },
        &cells,
    );
    let plain = view.selected_text(&cells).unwrap();
    view.copy_selected_text_with(
        &cells,
        &plain,
        /*clear_selection*/ true,
        |text, format| {
            assert_eq!(format, CopyFormat::Markdown);
            insta::assert_snapshot!(text);
            insta::assert_snapshot!(
                "selected_list_html",
                crate::clipboard_html::render_markdown(text)
            );
            Ok(crate::clipboard_copy::CopyStatus::Confirmed)
        },
    )
    .unwrap();
}

fn markdown_layout(markdown: &str, width: u16) -> TextLayout {
    let cell = AgentMarkdownCell::new(markdown.into(), Path::new("/"));
    TextLayout::new(
        cell.retained_hyperlink_lines(width, /*detailed*/ false),
        width,
    )
}

fn payload(layout: &TextLayout, range: std::ops::Range<usize>) -> (String, CopyFormat) {
    let mut lines = Vec::new();
    layout.copy_lines(range.clone(), "", &mut lines);
    crate::markdown_copy::selection(&lines, &layout.text()[range])
}

#[test]
fn partial_markup_is_balanced_and_preserves_selected_characters() {
    for (source, selected, expected) in [
        ("before `closeRequested_` after", "Requested", "`Requested`"),
        ("before ``a`b`` after", "a`", "`` a` ``"),
        ("**hello café界**", "café界", "**café界**"),
        ("before * first *", "first", "first"),
        ("before **hello world** after", " world", "&#32;**world**"),
        (
            "**strong *nested* text**",
            "strong nested text",
            "**strong _nested_ text**",
        ),
        (
            r"escaped \![label](https://example.com)",
            "escaped !label",
            r"escaped \![label](<https://example.com>)",
        ),
        (
            "[documentation](https://example.com)",
            "ment",                          // codespell:ignore ment
            "[ment](<https://example.com>)", // codespell:ignore ment
        ),
        ("[**label**](/repo/file.rs)", "label", "**label**"),
        (
            "escaped \\*literal\\* &amp; `code`",
            "*literal* & code",
            "\\*literal\\* \\& `code`",
        ),
    ] {
        let layout = markdown_layout(source, /*width*/ 80);
        let start = layout.text().find(selected).expect(source);
        assert_eq!(
            payload(&layout, start..start + selected.len()),
            (expected.into(), CopyFormat::Markdown),
            "{source}"
        );
    }
}

#[test]
fn inline_boundaries_preserve_selected_text_and_formatting() {
    let mut copies = Vec::new();
    for (source, selected, expected_html) in [
        (
            "_a **b**_**c**",
            "a bc",
            "<p><em>a</em> <strong><em>b</em>c</strong></p>\n",
        ),
        (
            "_a **b**_**c**",
            "bc",
            "<p><strong><em>b</em>c</strong></p>\n",
        ),
        (
            "*a **b***[**c**](/repo)",
            "a bc",
            "<p><em>a</em> <strong><em>b</em>c</strong></p>\n",
        ),
        ("_*a*_[_b_](/repo)", "ab", "<p><em>ab</em></p>\n"),
        (
            "*[a **b**](https://example.com) c*",
            "a b",
            "<p><a href=\"https://example.com\"><em>a</em> <strong><em>b</em></strong></a></p>\n",
        ),
        (
            "a*b*c a**b**c",
            "abc abc",
            "<p>a<em>b</em>c a<strong>b</strong>c</p>\n",
        ),
        ("&#97;_b_&#99;", "abc", "<p>a<em>b</em>c</p>\n"),
        ("&#97;__b__&#99;", "abc", "<p>a<strong>b</strong>c</p>\n"),
        ("&#769;_b_", "\u{301}b", "<p>\u{301}<em>b</em></p>\n"),
        ("&#97;*!*&#98;", "a!b", "<p>a<em>!</em>b</p>\n"),
        ("&#97;*&#12290;*&#98;", "a。b", "<p>a<em>。</em>b</p>\n"),
        (
            "&#97;**_b_**&#99;",
            "abc",
            "<p>a<strong><em>b</em></strong>c</p>\n",
        ),
        (
            "&#97;**`b`**&#99;",
            "abc",
            "<p>a<strong><code>b</code></strong>c</p>\n",
        ),
        ("_a_[b](/repo)", "ab", "<p><em>a</em>b</p>\n"),
        (
            "&#32;&#32;&#32;&#32;**hello**",
            "    hello",
            "<p>    <strong>hello</strong></p>\n",
        ),
    ] {
        let layout = markdown_layout(source, /*width*/ 80);
        let start = layout.text().find(selected).expect(source);
        let (copied, format) = payload(&layout, start..start + selected.len());
        assert_eq!(format, CopyFormat::Markdown);
        let html = crate::clipboard_html::render_markdown(&copied);
        assert_eq!(html, expected_html, "{source:?} => {copied:?}");
        copies.push(format!(
            "{source}\nSelected: {selected:?}\nMarkdown: {copied}\n{html}"
        ));
    }
    insta::assert_snapshot!(copies.join("\n"));
}

#[test]
fn lists_keep_nesting_and_paragraphs_without_sibling_spacers() {
    let source = "3. first item long enough to wrap\n4. second item\n   - nested `code`\n\n     another paragraph\n   - next nested\n5. final item\n\nOutside.";
    let mut copies = Vec::new();
    for width in [24, 80] {
        let layout = markdown_layout(source, width);
        let copied = payload(&layout, 0..layout.text().len()).0;
        assert_eq!(
            payload(&layout.rewrap(/*width*/ 14), 0..layout.text().len()).0,
            copied
        );
        copies.push(copied);
    }
    assert_eq!(copies[0], copies[1]);
    insta::assert_snapshot!(copies[0]);
}

#[test]
fn code_only_is_literal_and_mixed_code_has_safe_fences() {
    let layout = markdown_layout(
        "Prose\n\n````text\n  *literal* <tag>\n```\n````\n\nAfter",
        /*width*/ 80,
    );
    let selected = "  *literal* <tag>\n```";
    let start = layout.text().find(selected).unwrap();
    assert_eq!(
        payload(&layout, start..start + selected.len()),
        (selected.into(), CopyFormat::PlainText)
    );
    let (copy, format) = payload(&layout, 0..layout.text().len());
    assert_eq!(format, CopyFormat::Markdown);
    assert!(copy.contains("````\n  *literal* <tag>\n```\n````"));
    insta::assert_snapshot!(crate::clipboard_html::render_markdown(&copy));
}

#[test]
fn mixed_plain_output_remains_literal_in_html() {
    let layout = markdown_layout("**Answer**", /*width*/ 40);
    let literal = "*log* <tag>\n- not a list";
    let output = TextLayout::new(
        literal.lines().map(HyperlinkLine::from).collect(),
        /*width*/ 40,
    );
    let mut lines = Vec::new();
    layout.copy_lines(0..layout.text().len(), "", &mut lines);
    output.copy_lines(0..output.text().len(), "\n", &mut lines);
    let (copy, format) = crate::markdown_copy::selection(&lines, "unused");
    assert_eq!(format, CopyFormat::Markdown);
    assert_eq!(
        crate::clipboard_html::render_markdown(&copy),
        "<p><strong>Answer</strong></p>\n<pre><code>*log* &lt;tag&gt;\n- not a list\n</code></pre>\n"
    );
    assert_eq!(
        payload(&output, 0..output.text().len()),
        (literal.into(), CopyFormat::PlainText)
    );
}

#[test]
fn mixed_user_prose_preserves_literal_text_without_code_fences() {
    let cell = crate::history_cell::new_user_prompt(
        "    Explain *literal*\nnext <tag>".into(),
        Vec::new(),
        Vec::new(),
        vec!["https://example.com/image.png".into()],
    );
    let user = TextLayout::new(
        cell.retained_hyperlink_lines(/*width*/ 24, /*detailed*/ false),
        /*width*/ 24,
    );
    assert_eq!(
        payload(&user, 0..user.text().len()),
        (user.text().into(), CopyFormat::PlainText)
    );
    let user = user.rewrap(/*width*/ 12);
    let answer = markdown_layout("**Answer**", /*width*/ 24);
    let tool = TextLayout::new(vec![HyperlinkLine::from("*log* <tag>")], /*width*/ 24);
    let mut copies = Vec::new();
    for layouts in [[&user, &answer, &tool], [&answer, &tool, &user]] {
        let mut lines = Vec::new();
        for layout in layouts {
            layout.copy_lines(0..layout.text().len(), "\n\n", &mut lines);
        }
        let (text, format) = crate::markdown_copy::selection(&lines, "unused");
        assert_eq!(format, CopyFormat::Markdown);
        copies.push(crate::clipboard_html::render_markdown(&text));
    }
    insta::assert_snapshot!(copies.join("\n---\n"));
}

#[test]
fn copied_selection_keeps_its_revision_and_format_across_resize() {
    use crate::clipboard_copy::CopyStatus;
    let cell: Arc<dyn HistoryCell> = Arc::new(AgentMarkdownCell::new(
        "before `selected` after".into(),
        Path::new("/"),
    ));
    let mut cells = vec![cell];
    let mut view = TranscriptView::default();
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 4,
    );
    view.render(area, &mut Buffer::empty(area), &cells);
    let layout = view.layout(&cells, /*index*/ 0).unwrap();
    let start = layout.text().find("selected").unwrap();
    view.begin_selection(
        &cells,
        layout.column_for_offset(start + 8),
        /*row*/ 0,
        /*clicks*/ 1,
    );
    view.extend_selection(layout.column_for_offset(start), /*row*/ 0);
    view.end_drag();
    assert_eq!(view.selected_text(&cells).as_deref(), Some("selected"));
    cells[0] = Arc::new(AgentMarkdownCell::new("replacement".into(), Path::new("/")));
    let narrow = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 12, /*height*/ 4,
    );
    view.render(narrow, &mut Buffer::empty(narrow), &cells);
    assert_eq!(view.selected_text(&cells).as_deref(), Some("selected"));
    for clear_selection in [false, true] {
        view.copy_selected_text_with(&cells, "selected", clear_selection, |text, format| {
            assert_eq!((text, format), ("`selected`", CopyFormat::Markdown));
            Ok(CopyStatus::Pending(1))
        })
        .unwrap();
        assert_eq!(view.selected_text(&cells).as_deref(), Some("selected"));
        assert_eq!(
            view.finish_copy(
                &cells,
                &(1, Ok(CopyStatus::Confirmed)),
                /*current*/ true
            ),
            Some(false)
        );
        assert_eq!(view.has_selection_range(), !clear_selection);
    }
}

#[test]
fn literal_punctuation_is_not_reinterpreted_and_selected_breaks_survive() {
    for (source, expected_html) in [
        ("&amp;amp;", "<p>&amp;amp;</p>\n"),
        (r"1\) literal", "<p>1) literal</p>\n"),
        ("foo\n\\=\\=\\=", "<p>foo\n===</p>\n"),
    ] {
        let layout = markdown_layout(source, /*width*/ 80);
        let copy = payload(&layout, 0..layout.text().len()).0;
        assert_eq!(crate::clipboard_html::render_markdown(&copy), expected_html);
    }
    let layout = markdown_layout("first\n\nsecond", /*width*/ 80);
    assert_eq!(payload(&layout, 5..layout.text().len()).0, "\n\nsecond");
}

#[test]
fn explicit_hard_breaks_survive_only_when_selected() {
    let layout = markdown_layout("first  \nsecond", /*width*/ 80);
    assert_eq!(payload(&layout, 0..5).0, "first");
    assert_eq!(payload(&layout, 0..6).0, "first  \n");
    assert_eq!(
        crate::clipboard_html::render_markdown(&payload(&layout, 0..layout.text().len()).0),
        "<p>first<br />\nsecond</p>\n"
    );
}

#[test]
fn task_lists_and_transformed_tables_keep_their_meaning() {
    for (source, expected) in [
        ("- [x] done\n- [ ] next", "- [x] done\n- [ ] next"),
        ("1. [x] done\n2. [ ] next", "1. [x] done\n2. [ ] next"),
    ] {
        let layout = markdown_layout(source, /*width*/ 80);
        assert_eq!(payload(&layout, 0..layout.text().len()).0, expected);
    }
    let layout = markdown_layout(
        "Prose\n\n| A | B |\n| - | - |\n| x | y |",
        /*width*/ 80,
    );
    let copied = payload(&layout, 0..layout.text().len()).0;
    let html = crate::clipboard_html::render_markdown(&copied);
    assert!(html.contains("<pre><code>"), "{copied}\n{html}");
}

#[test]
fn link_entities_are_not_decoded_twice() {
    let layout = markdown_layout(
        "[label](https://example.com/?x=&amp;amp;)",
        /*width*/ 80,
    );
    let copy = payload(&layout, 0..5).0;
    assert_eq!(
        crate::clipboard_html::render_markdown(&copy),
        "<p><a href=\"https://example.com/?x=&amp;amp;\">label</a></p>\n"
    );
}

#[test]
fn selected_local_link_keeps_one_visible_destination() {
    let layout = markdown_layout("Open [**label**](/repo/file.rs).", /*width*/ 80);
    let copied = payload(&layout, 0..layout.text().len()).0;
    assert_eq!(copied, "Open **label** (repo/file.rs).");
    assert_eq!(
        crate::clipboard_html::render_markdown(&copied),
        "<p>Open <strong>label</strong> (repo/file.rs).</p>\n"
    );
}

#[test]
fn complete_rules_keep_structure_and_partial_rules_keep_selected_text() {
    let mut copies = Vec::new();
    for (source, partial) in [
        ("Before\n\n---\n\nAfter", "—"),
        ("Before\n\n> ---\n\nAfter", "> —"),
    ] {
        let layout = markdown_layout(source, /*width*/ 80);
        let copied = payload(&layout, 0..layout.text().len()).0;
        copies.push(format!(
            "{copied}\n{}",
            crate::clipboard_html::render_markdown(&copied)
        ));
        let start = layout.text().find('—').unwrap();
        assert_eq!(payload(&layout, start..start + "—".len()).0, partial);
    }
    insta::assert_snapshot!(copies.join("\n---\n"));
}

#[test]
fn table_spillover_keeps_prose_outside_the_literal_table() {
    let layout = markdown_layout(
        "**Before**\n\n| A | B |\n| --- | --- |\n| one | two |\nHTML block:\n<div>Prose after the table.</div>\n\nAfter",
        /*width*/ 80,
    );
    let copied = payload(&layout, 0..layout.text().len()).0;
    insta::assert_snapshot!(format!(
        "{copied}\n{}",
        crate::clipboard_html::render_markdown(&copied)
    ));
}

#[test]
fn partial_prose_keeps_leading_whitespace_outside_code_blocks() {
    let mut copies = Vec::new();
    for source in [
        "prefix    suffix",
        "prefix\tsuffix",
        "**prefix    suffix**",
        "> prefix    suffix",
    ] {
        let layout = markdown_layout(source, /*width*/ 80);
        let start = layout.text().find("prefix").unwrap() + "prefix".len();
        let copied = payload(&layout, start..layout.text().len()).0;
        copies.push(format!(
            "{copied}\n{}",
            crate::clipboard_html::render_markdown(&copied)
        ));
    }
    insta::assert_snapshot!(copies.join("\n---\n"));
}

#[test]
fn nested_tables_keep_their_copy_containers() {
    let mut copies = Vec::new();
    for source in [
        "Before\n\n> | A | B |\n> | --- | --- |\n> | one | two |",
        "Before\n\n- intro\n\n  | A | B |\n  | --- | --- |\n  | one | two |",
    ] {
        let layout = markdown_layout(source, /*width*/ 80);
        let copied = payload(&layout, 0..layout.text().len()).0;
        copies.push(format!(
            "{copied}\n{}",
            crate::clipboard_html::render_markdown(&copied)
        ));
    }
    insta::assert_snapshot!(copies.join("\n---\n"));
}

#[test]
fn selected_hard_breaks_survive_surrounding_blocks() {
    let source = "First paragraph.\n\nSecond paragraph with a hard break:  \nnext line.\n\n> quoted **words**  \n> next quote line\n\nFinal paragraph.";
    let mut stream = crate::streaming::controller::StreamController::new(
        Some(78),
        Path::new("/"),
        crate::history_cell::HistoryRenderMode::Rich,
    );
    let mut cells: Vec<Arc<dyn HistoryCell>> = Vec::new();
    for chunk in source.split_inclusive('\n') {
        stream.push(chunk);
        while let Some(cell) = stream.on_commit_tick_batch(/*max_lines*/ 1).0 {
            cells.push(cell.into());
        }
    }
    if let Some(cell) = stream.finalize().0 {
        cells.push(cell.into());
    }
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 40,
    );
    let mut view = TranscriptView::default();
    view.render(area, &mut Buffer::empty(area), &cells);
    view.begin_selection(&cells, /*column*/ 2, /*row*/ 0, /*clicks*/ 1);
    view.extend_selection(/*column*/ 79, /*row*/ 39);
    let plain = view.selected_text(&cells).unwrap();
    view.copy_selected_text_with(&cells, &plain, /*clear_selection*/ true, |text, format| {
        assert_eq!(format, CopyFormat::Markdown);
        assert_eq!(
            crate::clipboard_html::render_markdown(text),
            "<p>First paragraph.</p>\n<p>Second paragraph with a hard break:<br />\nnext line.</p>\n<blockquote>\n<p>quoted <strong>words</strong><br />\nnext quote line</p>\n</blockquote>\n<p>Final paragraph.</p>\n"
        );
        Ok(crate::clipboard_copy::CopyStatus::Confirmed)
    }).unwrap();
}

#[test]
fn selected_open_code_fence_keeps_streamed_rows_in_one_block() {
    let mut stream = crate::streaming::controller::StreamController::new(
        /*width*/ Some(78),
        Path::new("/"),
        crate::history_cell::HistoryRenderMode::Rich,
    );
    let mut cells: Vec<Arc<dyn HistoryCell>> = Vec::new();
    for chunk in ["Prose\n\n```rust\nlet a = 1;\n", "let b = 2;\n"] {
        stream.push(chunk);
        while let Some(cell) = stream.on_commit_tick_batch(/*max_lines*/ 1).0 {
            cells.push(cell.into());
        }
    }
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 10,
    );
    let mut view = TranscriptView::default();
    view.render(area, &mut Buffer::empty(area), &cells);
    view.begin_selection(&cells, /*column*/ 2, /*row*/ 0, /*clicks*/ 1);
    view.extend_selection(/*column*/ 79, /*row*/ 9);
    let plain = view.selected_text(&cells).unwrap();
    view.copy_selected_text_with(
        &cells,
        &plain,
        /*clear_selection*/ true,
        |text, format| {
            assert_eq!(format, CopyFormat::Markdown);
            insta::assert_snapshot!(crate::clipboard_html::render_markdown(text), @r#"
        <p>Prose</p>
        <pre><code>let a = 1;
        let b = 2;
        </code></pre>
        "#);
            Ok(crate::clipboard_copy::CopyStatus::Confirmed)
        },
    )
    .unwrap();
}

#[test]
fn cross_cell_selection_keeps_boundary_breaks() {
    let first = markdown_layout("first", /*width*/ 40);
    let second = markdown_layout("**second**", /*width*/ 40);
    for (left, right, expected) in [(5..5, 0..6, "\n**second**"), (0..5, 0..0, "first\n")] {
        let mut lines = Vec::new();
        first.copy_lines(left, "", &mut lines);
        second.copy_lines(right, "\n", &mut lines);
        assert_eq!(
            crate::markdown_copy::selection(&lines, "unused").0,
            expected
        );
    }
    let rule = markdown_layout("---", /*width*/ 40);
    let mut lines = Vec::new();
    first.copy_lines(0..first.text().len(), "", &mut lines);
    rule.copy_lines(0..rule.text().len(), "\n", &mut lines);
    let copied = crate::markdown_copy::selection(&lines, "unused").0;
    assert_eq!(
        crate::clipboard_html::render_markdown(&copied),
        "<p>first</p>\n<hr />\n"
    );
}

#[test]
fn adjacent_top_level_code_blocks_keep_blank_lines_and_boundaries() {
    let layout = markdown_layout(
        "Prose\n\n```\none\n\ninside\n```\n\n```\ntwo\n```",
        /*width*/ 80,
    );
    let copied = payload(&layout, 0..layout.text().len()).0;
    insta::assert_snapshot!(format!(
        "Markdown:\n{copied}\n\nHTML:\n{}",
        crate::clipboard_html::render_markdown(&copied)
    ));
}

#[test]
fn adjacent_code_containers_remain_separate() {
    for (source, expected_html) in [
        (
            "Prose\n\n> ```\n> quoted\n> ```\n\n```\nunquoted\n```",
            "<p>Prose</p>\n<blockquote>\n<pre><code>quoted\n</code></pre>\n</blockquote>\n<pre><code>unquoted\n</code></pre>\n",
        ),
        (
            "Prose\n\n- ```\n  one\n  ```\n- ```\n  two\n  ```",
            "<p>Prose</p>\n<ul>\n<li>\n<pre><code>one\n</code></pre>\n</li>\n<li>\n<pre><code>two\n</code></pre>\n</li>\n</ul>\n",
        ),
        (
            "- before\n\n  ```text\n  let value = 1;\n  ```\n\n  after\n\n> quoted  \n> next",
            "<ul>\n<li>\n<p>before</p>\n<pre><code>let value = 1;\n</code></pre>\n<p>after</p>\n</li>\n</ul>\n<blockquote>\n<p>quoted<br />\nnext</p>\n</blockquote>\n",
        ),
    ] {
        let layout = markdown_layout(source, /*width*/ 80);
        let copied = payload(&layout, 0..layout.text().len()).0;
        assert_eq!(
            crate::clipboard_html::render_markdown(&copied),
            expected_html,
            "{copied}"
        );
    }
}

#[test]
fn selected_list_and_code_containers_keep_their_structure() {
    for (source, start, end, expected) in [
        (
            "- parent\n  - nested `code`",
            "nested",
            "code",
            "- nested `code`",
        ),
        (
            "1. first\n2. **second**",
            "irst",
            "second",
            "1. irst\n2. **second**",
        ),
        (
            "10. first\n\n    second paragraph\n11. next",
            "first",
            "next",
            "10. first\n\n    second paragraph\n11. next",
        ),
        (
            "1. intro\n\n   more words\n2. next",
            "words",
            "next",
            "1. words\n2. next",
        ),
        (
            "- outer A\n  - inner A\n- outer B\n  - inner B",
            "inner A",
            "inner B",
            "- inner A\n- outer B\n  - inner B",
        ),
        (
            "> - outer\n>   > - nested `code`",
            "nested",
            "code",
            "> > - nested `code`",
        ),
        (
            "10. [x] first\n\n    paragraph\n11. [ ] next",
            "first",
            "next",
            "10. [x] first\n\n    paragraph\n11. [ ] next",
        ),
        (
            "10. before\n\n    ```\n    code\n    ```\n\n    after",
            "before",
            "after",
            "10. before\n\n    ```\n    code\n    ```\n\n    after",
        ),
        (
            "- outer\n  - before\n\n    ```\n    code\n    ```\n\n    after",
            "before",
            "after",
            "- before\n\n  ```\n  code\n  ```\n\n  after",
        ),
        (
            "Before\n\n    indented_code()\n\nAfter",
            "Before",
            "After",
            "Before\n\n```\nindented_code()\n```\n\nAfter",
        ),
        (
            "10. intro\n\n    ```\n    code\n    ```\n\n    after",
            "code",
            "after",
            "10. ```\n    code\n    ```\n\n    after",
        ),
        (
            "- [x] intro\n\n      code\n\n  after",
            "code",
            "after",
            "- [x]\n  ```\n  code\n  ```\n\n  after",
        ),
        (
            "> - intro\n>\n>   ```\n>   code\n>   ```\n>\n>   after",
            "code",
            "after",
            "> - ```\n>   code\n>   ```\n>\n>   after",
        ),
    ] {
        let layout = markdown_layout(source, /*width*/ 80);
        let start = layout.text().find(start).expect(source);
        let end = layout.text().rfind(end).expect(source) + end.len();
        let copied = payload(&layout, start..end).0;
        assert_eq!(
            crate::clipboard_html::render_markdown(&copied),
            crate::clipboard_html::render_markdown(expected),
            "{copied:?} from {source:?}"
        );
    }
}
