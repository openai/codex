use pretty_assertions::assert_eq;

use super::render_markdown_selection_text;

#[test]
fn rich_text_omits_markdown_syntax_and_keeps_visible_web_destination() {
    assert_eq!(
        render_markdown_selection_text(
            "**bold** and [label](https://example.com)",
            /*cwd*/ None,
        ),
        "bold and label (https://example.com)"
    );
}

#[test]
fn code_block_preserves_contents_without_fences() {
    assert_eq!(
        render_markdown_selection_text("```text\n  one\n\ttwo\n```", /*cwd*/ None),
        "  one\n\ttwo"
    );
}

#[test]
fn authored_blank_line_is_preserved_between_paragraphs() {
    assert_eq!(
        render_markdown_selection_text("first\n\nsecond", /*cwd*/ None),
        "first\n\nsecond"
    );
}

#[test]
fn blockquote_paragraph_breaks_keep_the_renderers_quote_prefix() {
    assert_eq!(
        render_markdown_selection_text("> one\n>\n> two\n>\n> three\n", /*cwd*/ None),
        "> one\n> \n> two\n> \n> three"
    );
}

#[test]
fn blockquote_after_list_text_keeps_list_and_quote_prefixes() {
    assert_eq!(
        render_markdown_selection_text("1. before\n   > quoted\n", /*cwd*/ None),
        "1. before\n   > quoted"
    );
}

#[test]
fn blockquote_as_first_list_content_shares_the_marker_line() {
    assert_eq!(
        render_markdown_selection_text("1.\n   > quoted\n", /*cwd*/ None),
        "1. > quoted"
    );
}

#[test]
fn blockquote_paragraph_break_inside_list_keeps_composed_prefix() {
    assert_eq!(
        render_markdown_selection_text("1.\n   > para 1\n   >\n   > para 2\n", /*cwd*/ None,),
        "1. > para 1\n   > \n   > para 2"
    );
}

#[test]
fn local_link_description_soft_break_stays_inline_like_the_renderer() {
    assert_eq!(
        render_markdown_selection_text(
            "[ignored](./src/main.rs)\n: description",
            /*cwd*/ Some(std::path::Path::new("/tmp/project")),
        ),
        "./src/main.rs: description"
    );
}

#[test]
fn local_link_soft_break_inside_label_does_not_split_target() {
    assert_eq!(
        render_markdown_selection_text(
            "[ignored\nlabel](./src/main.rs)",
            /*cwd*/ Some(std::path::Path::new("/tmp/project")),
        ),
        "./src/main.rs"
    );
}

#[test]
fn local_link_hard_break_inside_label_does_not_split_target() {
    assert_eq!(
        render_markdown_selection_text(
            "[ignored  \nlabel](./src/main.rs)",
            /*cwd*/ Some(std::path::Path::new("/tmp/project")),
        ),
        "./src/main.rs"
    );
}

#[test]
fn indented_code_preserves_each_lines_authored_indent() {
    assert_eq!(
        render_markdown_selection_text("    one\n    two", /*cwd*/ None),
        "    one\n    two"
    );
}

#[test]
fn nested_ordered_markers_match_the_renderers_number_alignment() {
    let mut markdown = "- parent\n".to_string();
    for index in 1..=10 {
        markdown.push_str(&format!("  {index}. item {index}\n"));
    }

    let rendered = render_markdown_selection_text(&markdown, /*cwd*/ None);
    let lines = rendered.lines().collect::<Vec<_>>();
    assert_eq!(lines[1], "    1. item 1");
    assert_eq!(lines[10], "   10. item 10");
}

#[test]
fn tight_list_continuation_keeps_the_renderers_indent() {
    assert_eq!(
        render_markdown_selection_text("- item line1\n  item line2\n", /*cwd*/ None),
        "- item line1\n  item line2"
    );
}

#[test]
fn loose_list_paragraphs_keep_marker_and_continuation_indent() {
    assert_eq!(
        render_markdown_selection_text(
            "- Intro\n\n  Continuation paragraph line 1\n  Continuation paragraph line 2\n",
            /*cwd*/ None,
        ),
        "- Intro\n\n  Continuation paragraph line 1\n  Continuation paragraph line 2"
    );
}

#[test]
fn nested_loose_list_keeps_indents_and_sibling_separator() {
    assert_eq!(
        render_markdown_selection_text(
            "1. A\n    - B\n\n      Continuation for B\n2. C\n",
            /*cwd*/ None,
        ),
        "1. A\n    - B\n\n      Continuation for B\n\n2. C"
    );
}

#[test]
fn agent_markdown_table_fence_uses_the_same_normalization_as_display() {
    let normalized = crate::markdown::unwrap_markdown_fences(
        "```markdown\n| Name | Value |\n| --- | --- |\n| alpha | one |\n```",
    );
    assert!(super::selection_text_contains_table(&normalized));
}
