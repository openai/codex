use pretty_assertions::assert_eq;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Text;

use crate::markdown_render::render_markdown_text;

#[test]
fn empty() {
}

#[test]
fn paragraph_soft_break() {
}

#[test]
fn paragraph_multiple() {
}

#[test]
fn headings() {
    let md = "# Heading 1\n## Heading 2\n### Heading 3\n#### Heading 4\n##### Heading 5\n###### Heading 6\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(["# ".bold().underlined(), "Heading 1".bold().underlined()]),
        Line::default(),
        Line::from_iter(["## ".bold(), "Heading 2".bold()]),
        Line::default(),
        Line::from_iter(["### ".bold().italic(), "Heading 3".bold().italic()]),
        Line::default(),
        Line::from_iter(["#### ".italic(), "Heading 4".italic()]),
        Line::default(),
        Line::from_iter(["##### ".italic(), "Heading 5".italic()]),
        Line::default(),
        Line::from_iter(["###### ".italic(), "Heading 6".italic()]),
    ]);
}

#[test]
fn blockquote_multiple_with_break() {
    let text = render_markdown_text("> Blockquote 1\n\n> Blockquote 2\n");
    let expected = Text::from_iter([
        Line::from_iter(["> ", "Blockquote 1"]).green(),
        Line::default(),
        Line::from_iter(["> ", "Blockquote 2"]).green(),
    ]);
}

#[test]
fn blockquote_in_ordered_list_on_next_line() {
    // Blockquote begins on a new line within an ordered list item; it should
    // render inline on the same marker line.
    let md = "1.\n   > quoted\n";
    let text = render_markdown_text(md);
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.clone())
                .collect::<String>()
        })
        .collect();
}

#[test]
fn blockquote_inside_nested_list() {
    let md = "1. A\n    - B\n      > inner\n";
    let text = render_markdown_text(md);
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.clone())
                .collect::<String>()
        })
        .collect();
}

#[test]
fn blockquote_heading_inherits_heading_style() {
    let text = render_markdown_text("> # test header\n> in blockquote\n");
}

#[test]
fn blockquote_with_code_block() {
    let md = "> ```\n> code\n> ```\n";
    let text = render_markdown_text(md);
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.clone())
                .collect::<String>()
        })
        .collect();
}

#[test]
fn list_unordered_single() {
    let text = render_markdown_text("- List item 1\n");
    let expected = Text::from_iter([Line::from_iter(["- ", "List item 1"])]);
}

#[test]
fn emphasis() {
}

#[test]
fn strikethrough() {
}

#[test]
fn strong_emphasis() {
    let text = render_markdown_text("**Strong *emphasis***");
    let expected = Text::from(Line::from_iter([
        "Strong ".bold(),
        "emphasis".bold().italic(),
    ]));
}

#[test]
fn code_block_inside_unordered_list_item_is_indented() {
    let md = "- Item\n\n  ```\n  code line\n  ```\n";
    let text = render_markdown_text(md);
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.clone())
                .collect::<String>()
        })
        .collect();
}

#[test]
fn nested_five_levels_mixed_lists() {
    let md = "1. First\n   - Second level\n     1. Third level (ordered)\n        - Fourth level (bullet)\n          - Fifth level to test indent consistency\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(["1. ".light_blue(), "First".into()]),
        Line::from_iter(["    - ", "Second level"]),
        Line::from_iter(["        1. ".light_blue(), "Third level (ordered)".into()]),
        Line::from_iter(["            - ", "Fourth level (bullet)"]),
        Line::from_iter([
            "                - ",
            "Fifth level to test indent consistency",
        ]),
    ]);
}

#[test]
fn ordered_item_continuation_paragraph_is_indented() {
    let md = "1. Intro\n\n   More details about intro\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(["1. ".light_blue(), "Intro".into()]),
        Line::default(),
        Line::from_iter(["   ", "More details about intro"]),
    ]);
    assert_eq!(text, expected);
}
