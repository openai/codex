use pretty_assertions::assert_eq;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;

use crate::markdown_render::render_markdown_text;

#[test]
fn empty() {
    assert_eq!(render_markdown_text(""), Text::default());
}

#[test]
fn paragraph_single() {
    assert_eq!(
        render_markdown_text("Hello, world!"),
        Text::from("Hello, world!")
    );
}

#[test]
fn paragraph_soft_break() {
    assert_eq!(
        render_markdown_text("Hello\nWorld"),
        Text::from_iter(["Hello", "World"])
    );
}

#[test]
fn paragraph_multiple() {
    assert_eq!(
        render_markdown_text("Paragraph 1\n\nParagraph 2"),
        Text::from_iter(["Paragraph 1", "", "Paragraph 2"])
    );
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
    assert_eq!(text, expected);
}

#[test]
fn blockquote_single() {
    let text = render_markdown_text("> Blockquote");
    let expected = Text::from(Line::from_iter(["> ", "Blockquote"]).green());
    assert_eq!(text, expected);
}

#[test]
fn blockquote_soft_break() {
    // Soft break via lazy continuation should render as a new line in blockquotes.
    let text = render_markdown_text("> This is a blockquote\nwith a soft break\n");
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
    assert_eq!(
        lines,
        vec![
            "> This is a blockquote".to_string(),
            "> with a soft break".to_string()
        ]
    );
}

#[test]
fn blockquote_multiple_with_break() {
    let text = render_markdown_text("> Blockquote 1\n\n> Blockquote 2\n");
    let expected = Text::from_iter([
        Line::from_iter(["> ", "Blockquote 1"]).green(),
        Line::default(),
        Line::from_iter(["> ", "Blockquote 2"]).green(),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn blockquote_three_paragraphs_short_lines() {
    let md = "> one\n>\n> two\n>\n> three\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(["> ", "one"]).green(),
        Line::from_iter(["> "]).green(),
        Line::from_iter(["> ", "two"]).green(),
        Line::from_iter(["> "]).green(),
        Line::from_iter(["> ", "three"]).green(),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn blockquote_nested_two_levels() {
    let md = "> Level 1\n>> Level 2\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(["> ", "Level 1"]).green(),
        Line::from_iter(["> "]).green(),
        Line::from_iter(["> ", "> ", "Level 2"]).green(),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn blockquote_with_list_items() {
    let md = "> - item 1\n> - item 2\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(["> ", "- ", "item 1"]).green(),
        Line::from_iter(["> ", "- ", "item 2"]).green(),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn blockquote_with_ordered_list() {
    let md = "> 1. first\n> 2. second\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(vec![
            Span::from("> "),
            "1. ".light_blue(),
            Span::from("first"),
        ])
        .green(),
        Line::from_iter(vec![
            Span::from("> "),
            "2. ".light_blue(),
            Span::from("second"),
        ])
        .green(),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn blockquote_list_then_nested_blockquote() {
    let md = "> - parent\n>   > child\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(["> ", "- ", "parent"]).green(),
        Line::from_iter(["> ", "  ", "> ", "child"]).green(),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn list_item_with_inline_blockquote_on_same_line() {
    let md = "1. > quoted\n";
    let text = render_markdown_text(md);
    let mut lines = text.lines.iter();
    let first = lines.next().expect("one line");
    // Expect content to include the ordered marker, a space, "> ", and the text
    let s: String = first.spans.iter().map(|sp| sp.content.clone()).collect();
    assert_eq!(s, "1. > quoted");
}

#[test]
fn blockquote_surrounded_by_blank_lines() {
    let md = "foo\n\n> bar\n\nbaz\n";
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
    assert_eq!(
        lines,
        vec![
            "foo".to_string(),
            "".to_string(),
            "> bar".to_string(),
            "".to_string(),
            "baz".to_string(),
        ]
    );
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
    assert_eq!(lines, vec!["1. > quoted".to_string()]);
}

#[test]
fn blockquote_in_unordered_list_on_next_line() {
    // Blockquote begins on a new line within an unordered list item; it should
    // render inline on the same marker line.
    let md = "-\n  > quoted\n";
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
    assert_eq!(lines, vec!["- > quoted".to_string()]);
}

#[test]
fn blockquote_two_paragraphs_inside_ordered_list_has_blank_line() {
    // Two blockquote paragraphs inside a list item should be separated by a blank line.
    let md = "1.\n   > para 1\n   >\n   > para 2\n";
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
    assert_eq!(
        lines,
        vec![
            "1. > para 1".to_string(),
            "   > ".to_string(),
            "   > para 2".to_string(),
        ],
        "expected blockquote content to stay aligned after list marker"
    );
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
    assert_eq!(lines, vec!["1. A", "    - B", "      > inner"]);
}

#[test]
fn list_item_text_then_blockquote() {
    let md = "1. before\n   > quoted\n";
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
    assert_eq!(lines, vec!["1. before", "   > quoted"]);
}

#[test]
fn list_item_blockquote_then_text() {
    let md = "1.\n   > quoted\n   after\n";
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
    assert_eq!(lines, vec!["1. > quoted", "   > after"]);
}

#[test]
fn list_item_text_blockquote_text() {
    let md = "1. before\n   > quoted\n   after\n";
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
    assert_eq!(lines, vec!["1. before", "   > quoted", "   > after"]);
}

#[test]
fn blockquote_with_heading_and_paragraph() {
    let md = "> # Heading\n> paragraph text\n";
    let text = render_markdown_text(md);
    // Validate on content shape; styling is handled elsewhere
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
    assert_eq!(
        lines,
        vec![
            "> # Heading".to_string(),
            "> ".to_string(),
            "> paragraph text".to_string(),
        ]
    );
}

#[test]
fn blockquote_heading_inherits_heading_style() {
    let text = render_markdown_text("> # test header\n> in blockquote\n");
    assert_eq!(
        text.lines,
        [
            Line::from_iter([
                "> ".into(),
                "# ".bold().underlined(),
                "test header".bold().underlined(),
            ])
            .green(),
            Line::from_iter(["> "]).green(),
            Line::from_iter(["> ", "in blockquote"]).green(),
        ]
    );
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
    assert_eq!(
        lines,
        vec![
            "> ```".to_string(),
            "> code".to_string(),
            "> ```".to_string()
        ]
    );
}

#[test]
fn blockquote_with_multiline_code_block() {
    let md = "> ```\n> first\n> second\n> ```\n";
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
    assert_eq!(lines, vec!["> ```", "> first", "> second", "> ```"]);
}

#[test]
fn list_unordered_single() {
    let text = render_markdown_text("- List item 1\n");
    let expected = Text::from_iter([Line::from_iter(["- ", "List item 1"])]);
    assert_eq!(text, expected);
}

#[test]
fn list_unordered_multiple() {
    let text = render_markdown_text("- List item 1\n- List item 2\n");
    let expected = Text::from_iter([
        Line::from_iter(["- ", "List item 1"]),
        Line::from_iter(["- ", "List item 2"]),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn list_ordered() {
    let text = render_markdown_text("1. List item 1\n2. List item 2\n");
    let expected = Text::from_iter([
        Line::from_iter(["1. ".light_blue(), "List item 1".into()]),
        Line::from_iter(["2. ".light_blue(), "List item 2".into()]),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn list_nested() {
    let text = render_markdown_text("- List item 1\n  - Nested list item 1\n");
    let expected = Text::from_iter([
        Line::from_iter(["- ", "List item 1"]),
        Line::from_iter(["    - ", "Nested list item 1"]),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn list_ordered_custom_start() {
    let text = render_markdown_text("3. First\n4. Second\n");
    let expected = Text::from_iter([
        Line::from_iter(["3. ".light_blue(), "First".into()]),
        Line::from_iter(["4. ".light_blue(), "Second".into()]),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn nested_unordered_in_ordered() {
    let md = "1. Outer\n    - Inner A\n    - Inner B\n2. Next\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(["1. ".light_blue(), "Outer".into()]),
        Line::from_iter(["    - ", "Inner A"]),
        Line::from_iter(["    - ", "Inner B"]),
        Line::from_iter(["2. ".light_blue(), "Next".into()]),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn nested_ordered_in_unordered() {
    let md = "- Outer\n    1. One\n    2. Two\n- Last\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(["- ", "Outer"]),
        Line::from_iter(["    1. ".light_blue(), "One".into()]),
        Line::from_iter(["    2. ".light_blue(), "Two".into()]),
        Line::from_iter(["- ", "Last"]),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn loose_list_item_multiple_paragraphs() {
    let md = "1. First paragraph\n\n   Second paragraph of same item\n\n2. Next item\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(["1. ".light_blue(), "First paragraph".into()]),
        Line::default(),
        Line::from_iter(["   ", "Second paragraph of same item"]),
        Line::from_iter(["2. ".light_blue(), "Next item".into()]),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn tight_item_with_soft_break() {
    let md = "- item line1\n  item line2\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(["- ", "item line1"]),
        Line::from_iter(["  ", "item line2"]),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn deeply_nested_mixed_three_levels() {
    let md = "1. A\n    - B\n        1. C\n2. D\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(["1. ".light_blue(), "A".into()]),
        Line::from_iter(["    - ", "B"]),
        Line::from_iter(["        1. ".light_blue(), "C".into()]),
        Line::from_iter(["2. ".light_blue(), "D".into()]),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn loose_items_due_to_blank_line_between_items() {
    let md = "1. First\n\n2. Second\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(["1. ".light_blue(), "First".into()]),
        Line::from_iter(["2. ".light_blue(), "Second".into()]),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn mixed_tight_then_loose_in_one_list() {
    let md = "1. Tight\n\n2.\n   Loose\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(["1. ".light_blue(), "Tight".into()]),
        Line::from_iter(["2. ".light_blue(), "Loose".into()]),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn ordered_item_with_indented_continuation_is_tight() {
    let md = "1. Foo\n   Bar\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(["1. ".light_blue(), "Foo".into()]),
        Line::from_iter(["   ", "Bar"]),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn inline_code() {
    let text = render_markdown_text("Example of `Inline code`");
    let expected = Line::from_iter(["Example of ".into(), "Inline code".dim()]).into();
    assert_eq!(text, expected);
}

#[test]
fn strong() {
    assert_eq!(
        render_markdown_text("**Strong**"),
        Text::from(Line::from("Strong".bold()))
    );
}

#[test]
fn emphasis() {
    assert_eq!(
        render_markdown_text("*Emphasis*"),
        Text::from(Line::from("Emphasis".italic()))
    );
}

#[test]
fn strikethrough() {
    assert_eq!(
        render_markdown_text("~~Strikethrough~~"),
        Text::from(Line::from("Strikethrough".crossed_out()))
    );
}

#[test]
fn strong_emphasis() {
    let text = render_markdown_text("**Strong *emphasis***");
    let expected = Text::from(Line::from_iter([
        "Strong ".bold(),
        "emphasis".bold().italic(),
    ]));
    assert_eq!(text, expected);
}

#[test]
fn link() {
    let text = render_markdown_text("[Link](https://example.com)");
    let expected = Text::from(Line::from_iter([
        "Link".into(),
        " (".into(),
        "https://example.com".cyan().underlined(),
        ")".into(),
    ]));
    assert_eq!(text, expected);
}

#[test]
fn code_block_unhighlighted() {
    let text = render_markdown_text("```rust\nfn main() {}\n```\n");
    let expected = Text::from_iter([
        Line::from("```rust"),
        Line::from("fn main() {}"),
        Line::from("```"),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn code_block_multiple_lines_root() {
    let md = "```\nfirst\nsecond\n```\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from("```"),
        Line::from("first"),
        Line::from("second"),
        Line::from("```"),
    ]);
    assert_eq!(text, expected);
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
    assert_eq!(lines, vec!["- Item", "", "  ```", "  code line", "  ```"]);
}

#[test]
fn code_block_multiple_lines_inside_unordered_list() {
    let md = "- Item\n\n  ```\n  first\n  second\n  ```\n";
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
    assert_eq!(
        lines,
        vec!["- Item", "", "  ```", "  first", "  second", "  ```"]
    );
}

#[test]
fn code_block_inside_unordered_list_item_multiple_lines() {
    let md = "- Item\n\n  ```\n  first\n  second\n  ```\n";
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
    assert_eq!(
        lines,
        vec!["- Item", "", "  ```", "  first", "  second", "  ```"]
    );
}

#[test]
fn ordered_item_with_code_block_and_nested_bullet() {
    let md = "1. **item 1**\n\n2. **item 2**\n   ```\n   code\n   ```\n   - `PROCESS_START` (a `OnceLock<Instant>`) keeps the start time for the entire process.\n";
    let text = render_markdown_text(md);
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.clone())
                .collect::<String>()
        })
        .collect();
    assert_eq!(
        lines,
        vec![
            "1. item 1".to_string(),
            "2. item 2".to_string(),
            String::new(),
            "   ```".to_string(),
            "   code".to_string(),
            "   ```".to_string(),
            "    - PROCESS_START (a OnceLock<Instant>) keeps the start time for the entire process.".to_string(),
        ]
    );
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
    assert_eq!(text, expected);
}

#[test]
fn html_inline_is_verbatim() {
    let md = "Hello <span>world</span>!";
    let text = render_markdown_text(md);
    let expected: Text = Line::from_iter(["Hello ", "<span>", "world", "</span>", "!"]).into();
    assert_eq!(text, expected);
}

#[test]
fn html_block_is_verbatim_multiline() {
    let md = "<div>\n  <span>hi</span>\n</div>\n";
    let text = render_markdown_text(md);
    let expected = Text::from(Line::from_iter(["<div>", "  <span>hi</span>", "</div>"]));
    assert_eq!(text, expected);
}

#[test]
fn html_in_tight_ordered_item_soft_breaks_with_space() {
    let md = "1. Foo\n   <i>Bar</i>\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(["1. ".light_blue(), "Foo".into()]),
        Line::from_iter(["   ", "<i>", "Bar", "</i>"]),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn html_continuation_paragraph_in_unordered_item_indented() {
    let md = "- Item\n\n  <em>continued</em>\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(["- ", "Item"]),
        Line::default(),
        Line::from_iter(["  ", "<em>", "continued", "</em>"]),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn unordered_item_continuation_paragraph_is_indented() {
    let md = "- Intro\n\n  Continuation paragraph line 1\n  Continuation paragraph line 2\n";
    let text = render_markdown_text(md);
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.clone())
                .collect::<String>()
        })
        .collect();
    assert_eq!(
        lines,
        vec![
            "- Intro".to_string(),
            String::new(),
            "  Continuation paragraph line 1".to_string(),
            "  Continuation paragraph line 2".to_string(),
        ]
    );
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

#[test]
fn nested_item_continuation_paragraph_is_indented() {
    let md = "1. A\n    - B\n\n      Continuation for B\n2. C\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(["1. ".light_blue(), "A".into()]),
        Line::from_iter(["    - ", "B"]),
        Line::default(),
        Line::from_iter(["      ", "Continuation for B"]),
        Line::from_iter(["2. ".light_blue(), "C".into()]),
    ]);
    assert_eq!(text, expected);
}
