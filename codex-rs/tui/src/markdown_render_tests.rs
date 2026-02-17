use pretty_assertions::assert_eq;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;

use crate::markdown_render::render_markdown_text;
use insta::assert_snapshot;

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
    assert_eq!(lines, vec!["> code".to_string()]);
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
    assert_eq!(lines, vec!["> first", "> second"]);
}

#[test]
fn nested_blockquote_with_inline_and_fenced_code() {
    /*
    let md = \"> Nested quote with code:\n\
    > > Inner quote and `inline code`\n\
    > >\n\
    > > ```\n\
    > > # fenced code inside a quote\n\
    > > echo \"hello from a quote\"\n\
    > > ```\n";
    */
    let md = r#"> Nested quote with code:
> > Inner quote and `inline code`
> >
> > ```
> > # fenced code inside a quote
> > echo "hello from a quote"
> > ```
"#;
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
            "> Nested quote with code:".to_string(),
            "> ".to_string(),
            "> > Inner quote and inline code".to_string(),
            "> > ".to_string(),
            "> > # fenced code inside a quote".to_string(),
            "> > echo \"hello from a quote\"".to_string(),
        ]
    );
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
    let expected = Line::from_iter(["Example of ".into(), "Inline code".cyan()]).into();
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
    let expected = Text::from_iter([Line::from_iter(["", "fn main() {}"])]);
    assert_eq!(text, expected);
}

#[test]
fn code_block_multiple_lines_root() {
    let md = "```\nfirst\nsecond\n```\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(["", "first"]),
        Line::from_iter(["", "second"]),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn code_block_indented() {
    let md = "    function greet() {\n      console.log(\"Hi\");\n    }\n";
    let text = render_markdown_text(md);
    let expected = Text::from_iter([
        Line::from_iter(["    ", "function greet() {"]),
        Line::from_iter(["    ", "  console.log(\"Hi\");"]),
        Line::from_iter(["    ", "}"]),
    ]);
    assert_eq!(text, expected);
}

#[test]
fn horizontal_rule_renders_em_dashes() {
    let md = "Before\n\n---\n\nAfter\n";
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
    assert_eq!(lines, vec!["Before", "", "———", "", "After"]);
}

#[test]
fn code_block_with_inner_triple_backticks_outer_four() {
    let md = r#"````text
Here is a code block that shows another fenced block:

```md
# Inside fence
- bullet
- `inline code`
```
````
"#;
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
            "Here is a code block that shows another fenced block:".to_string(),
            String::new(),
            "```md".to_string(),
            "# Inside fence".to_string(),
            "- bullet".to_string(),
            "- `inline code`".to_string(),
            "```".to_string(),
        ]
    );
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
    assert_eq!(lines, vec!["- Item", "", "  code line"]);
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
    assert_eq!(lines, vec!["- Item", "", "  first", "  second"]);
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
    assert_eq!(lines, vec!["- Item", "", "  first", "  second"]);
}

#[test]
fn markdown_render_complex_snapshot() {
    let md = r#"# H1: Markdown Streaming Test
Intro paragraph with bold **text**, italic *text*, and inline code `x=1`.
Combined bold-italic ***both*** and escaped asterisks \*literal\*.
Auto-link: <https://example.com> and reference link [ref][r1].
Link with title: [hover me](https://example.com "Example") and mailto <mailto:test@example.com>.
Image: ![alt text](https://example.com/img.png "Title")
> Blockquote level 1
>> Blockquote level 2 with `inline code`
- Unordered list item 1
  - Nested bullet with italics _inner_
- Unordered list item 2 with ~~strikethrough~~
1. Ordered item one
2. Ordered item two with sublist:
   1) Alt-numbered subitem
- [ ] Task: unchecked
- [x] Task: checked with link [home](https://example.org)
---
Table below (alignment test):
| Left | Center | Right |
|:-----|:------:|------:|
| a    |   b    |     c |
Inline HTML: <sup>sup</sup> and <sub>sub</sub>.
HTML block:
<div style="border:1px solid #ccc;padding:2px">inline block</div>
Escapes: \_underscores\_, backslash \\, ticks ``code with `backtick` inside``.
Emoji shortcodes: :sparkles: :tada: (if supported).
Hard break test (line ends with two spaces)  
Next line should be close to previous.
Footnote reference here[^1] and another[^longnote].
Horizontal rule with asterisks:
***
Fenced code block (JSON):
```json
{ "a": 1, "b": [true, false] }
```
Fenced code with tildes and triple backticks inside:
~~~markdown
To close ``` you need tildes.
~~~
Indented code block:
    for i in range(3): print(i)
Definition-like list:
Term
: Definition with `code`.
Character entities: &amp; &lt; &gt; &quot; &#39;
[^1]: This is the first footnote.
[^longnote]: A longer footnote with a link to [Rust](https://www.rust-lang.org/).
Escaped pipe in text: a \| b \| c.
URL with parentheses: [link](https://example.com/path_(with)_parens).
[r1]: https://example.com/ref "Reference link title"
"#;

    let text = render_markdown_text(md);
    // Convert to plain text lines for snapshot (ignore styles)
    let rendered = text
        .lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.clone())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert_snapshot!(rendered);
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
            "   code".to_string(),
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
    let expected = Text::from_iter([
        Line::from_iter(["<div>"]),
        Line::from_iter(["  <span>hi</span>"]),
        Line::from_iter(["</div>"]),
    ]);
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

#[test]
fn table_renders_unicode_box() {
    let md = "| A | B |\n|---|---|\n| 1 | 2 |\n";
    let text = render_markdown_text(md);
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|span| span.content.clone()).collect())
        .collect();
    assert_eq!(
        lines,
        vec![
            "┌─────┬─────┐".to_string(),
            "│ A   │ B   │".to_string(),
            "├─────┼─────┤".to_string(),
            "│ 1   │ 2   │".to_string(),
            "└─────┴─────┘".to_string(),
        ]
    );
}

#[test]
fn table_alignment_respects_markers() {
    let md = "| Left | Center | Right |\n|:-----|:------:|------:|\n| a | b | c |\n";
    let text = render_markdown_text(md);
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|span| span.content.clone()).collect())
        .collect();
    assert_eq!(lines[1], "│ Left │ Center │ Right │");
    assert_eq!(lines[3], "│ a    │   b    │     c │");
}

#[test]
fn table_wraps_cell_content_when_width_is_narrow() {
    let md = "| Key | Description |\n| --- | --- |\n| -v | Enable very verbose logging output for debugging |\n";
    let text = crate::markdown_render::render_markdown_text_with_width(md, Some(30));
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|span| span.content.clone()).collect())
        .collect();
    assert!(lines[0].starts_with('┌') && lines[0].ends_with('┐'));
    assert!(
        lines
            .iter()
            .any(|line| line.contains("Enable very verbose"))
            && lines.iter().any(|line| line.contains("logging output")),
        "expected wrapped row content: {lines:?}"
    );
}

#[test]
fn table_inside_blockquote_has_quote_prefix() {
    let md = "> | A | B |\n> |---|---|\n> | 1 | 2 |\n";
    let text = render_markdown_text(md);
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|span| span.content.clone()).collect())
        .collect();
    assert!(lines.iter().all(|line| line.starts_with("> ")));
    assert!(lines.iter().any(|line| line.contains("┌─────┬─────┐")));
}

#[test]
fn escaped_pipes_render_in_table_cells() {
    let md = "| Col |\n| --- |\n| a \\| b |\n";
    let text = render_markdown_text(md);
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|span| span.content.clone()).collect())
        .collect();
    assert!(lines.iter().any(|line| line.contains("a | b")));
}

#[test]
fn table_falls_back_to_pipe_rendering_if_it_cannot_fit() {
    let md = "| c1 | c2 | c3 | c4 | c5 | c6 | c7 | c8 | c9 | c10 |\n|---|---|---|---|---|---|---|---|---|---|\n| 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 |
  10 |\n";
    let text = crate::markdown_render::render_markdown_text_with_width(md, Some(20));
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|span| span.content.clone()).collect())
        .collect();
    assert!(lines.first().is_some_and(|line| line.starts_with('|')));
    assert!(!lines.iter().any(|line| line.contains('┌')));
}

#[test]
fn table_pipe_fallback_escapes_literal_pipes_in_cell_content() {
    let md = "| c1 | c2 | c3 | c4 | c5 | c6 | c7 | c8 | c9 | c10 |\n|---|---|---|---|---|---|---|---|---|---|\n| keep | keep | keep | keep | keep | keep | keep | keep | a \\| b | keep |\n";
    let text = crate::markdown_render::render_markdown_text_with_width(md, Some(20));
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|span| span.content.clone()).collect())
        .collect();

    assert!(lines.first().is_some_and(|line| line.starts_with('|')));
    assert!(lines.iter().any(|line| line.contains("a \\| b")));
}

#[test]
fn table_link_keeps_url_suffix_inside_cell() {
    let md = "| Site |\n|---|\n| [OpenAI](https://openai.com) |\n";
    let text = render_markdown_text(md);
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|span| span.content.clone()).collect())
        .collect();

    assert!(
        lines
            .iter()
            .any(|line| line.contains("OpenAI (https://openai.com)")),
        "expected link suffix inside table cell: {lines:?}"
    );
    assert!(
        !lines.iter().any(|line| line.trim() == "(https://openai.com)"),
        "did not expect stray url suffix line outside table: {lines:?}"
    );
}

#[test]
fn table_does_not_absorb_trailing_html_block_label_line() {
    let md = "| Left | Center | Right |\n|:-----|:------:|------:|\n| a    |   b    |     c |\nInline HTML: <sup>sup</sup> and <sub>sub</sub>.\nHTML block:\n<div style=\"border:1px solid #ccc;padding:2px\">inline block</div>\n";
    let text = render_markdown_text(md);
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|span| span.content.clone()).collect())
        .collect();

    assert!(
        lines
            .iter()
            .any(|line| line.trim() == "HTML block:"),
        "expected 'HTML block:' as plain prose line: {lines:?}"
    );
    assert!(
        !lines.iter().any(|line| line.contains("│ HTML block:")),
        "did not expect 'HTML block:' inside table grid: {lines:?}"
    );
}

#[test]
fn table_spillover_prose_wraps_in_narrow_width() {
    let long_label = "This html spillover prose line should wrap on narrow widths to avoid clipping:";
    let md = format!(
        "| Left | Center | Right |\n|:-----|:------:|------:|\n| a    |   b    |     c |\n{long_label}\n<div style=\"border:1px solid #ccc;padding:2px\">inline block</div>\n"
    );
    let text = crate::markdown_render::render_markdown_text_with_width(&md, Some(40));
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|span| span.content.clone()).collect())
        .collect();

    assert!(
        lines.iter().any(|line| line.contains("This html spillover prose")),
        "expected spillover prose to be present: {lines:?}"
    );
    assert!(
        !lines.iter().any(|line| line.contains(long_label)),
        "did not expect spillover prose to remain as one long clipped line: {lines:?}"
    );
}

#[test]
fn table_keeps_sparse_comparison_row_inside_grid() {
    let md = "| A | B | C |\n|---|---|---|\n| x < y > z | | |\n";
    let text = render_markdown_text(md);
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|span| span.content.clone()).collect())
        .collect();

    assert!(
        lines
            .iter()
            .any(|line| line.contains("│ x < y > z") && line.ends_with('│')),
        "expected sparse comparison row to remain inside table grid: {lines:?}"
    );
    assert!(
        !lines.iter().any(|line| line.trim() == "x < y > z"),
        "did not expect sparse comparison row to spill outside table: {lines:?}"
    );
}

#[test]
fn table_keeps_sparse_rows_with_empty_trailing_cells() {
    let md = "| A | B | C |\n|---|---|---|\n| a | | |\n";
    let text = render_markdown_text(md);
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|span| span.content.clone()).collect())
        .collect();

    assert!(
        lines
            .iter()
            .any(|line| line.contains("│ a") && line.ends_with('│')),
        "expected sparse row to remain inside table grid: {lines:?}"
    );
    assert!(
        !lines.iter().any(|line| line == "a"),
        "did not expect sparse row content to spill outside the table: {lines:?}"
    );
}

#[test]
fn table_keeps_sparse_sentence_row_inside_grid() {
    let md = "| A | B | C |\n|---|---|---|\n| This is done. | | |\n";
    let text = render_markdown_text(md);
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|span| span.content.clone()).collect())
        .collect();

    assert!(
        lines
            .iter()
            .any(|line| line.contains("│ This is done.") && line.ends_with('│')),
        "expected sparse sentence row to remain inside table grid: {lines:?}"
    );
    assert!(
        !lines.iter().any(|line| line.trim() == "This is done."),
        "did not expect sparse sentence row to spill outside table: {lines:?}"
    );
}

#[test]
fn table_keeps_label_only_sparse_row_inside_grid() {
    let md = "| A | B | C |\n|---|---|---|\n| Status: | | |\n| ok | | |\n";
    let text = render_markdown_text(md);
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|span| span.content.clone()).collect())
        .collect();

    assert!(
        lines
            .iter()
            .any(|line| line.contains("│ Status:") && line.ends_with('│')),
        "expected label-only sparse row to remain inside table grid: {lines:?}"
    );
    assert!(
        !lines.iter().any(|line| line.trim() == "Status:"),
        "did not expect label-only sparse row to spill outside table: {lines:?}"
    );
}

#[test]
fn table_keeps_single_word_label_row_at_end_inside_grid() {
    let md = "| A | B | C |\n|---|---|---|\n| Status: | | |\n";
    let text = render_markdown_text(md);
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|span| span.content.clone()).collect())
        .collect();

    assert!(
        lines
            .iter()
            .any(|line| line.contains("│ Status:") && line.ends_with('│')),
        "expected single-word trailing label row to remain inside table grid: {lines:?}"
    );
    assert!(
        !lines.iter().any(|line| line.trim() == "Status:"),
        "did not expect single-word trailing label row to spill outside table: {lines:?}"
    );
}

#[test]
fn table_keeps_multi_word_label_row_at_end_inside_grid() {
    let md = "| A | B | C |\n|---|---|---|\n| Build status: | | |\n";
    let text = render_markdown_text(md);
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|span| span.content.clone()).collect())
        .collect();

    assert!(
        lines
            .iter()
            .any(|line| line.contains("│ Build status:") && line.ends_with('│')),
        "expected multi-word trailing label row to remain inside table grid: {lines:?}"
    );
    assert!(
        !lines.iter().any(|line| line.trim() == "Build status:"),
        "did not expect multi-word trailing label row to spill outside table: {lines:?}"
    );
}

#[test]
fn table_preserves_structured_leading_columns_when_last_column_is_long() {
    let md = "| Milestone | Planned Date | Outcome | Retrospective Summary |\n|---|---|---|---|\n| Canary rollout | 2026-01-10 | Completed | Canary
  traffic was held at 5% longer than planned due to latency regressions tied to cold cache behavior; after pre-warming and query plan hints, p95
  returned to baseline and rollout resumed safely. |\n| Full region cutover | 2026-01-24 | Completed | Cutover succeeded with no customer-visible
  downtime, though internal dashboards lagged for approximately 18 minutes because ingestion workers autoscaled slower than forecast under burst load.
  |\n";
    let text = crate::markdown_render::render_markdown_text_with_width(md, Some(160));
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|span| span.content.clone()).collect())
        .collect();

    assert!(
        lines.iter().any(|line| line.contains("Milestone")),
        "expected first structured header to remain readable: {lines:?}"
    );
    assert!(
        lines.iter().any(|line| line.contains("Planned Date")),
        "expected date header to remain readable: {lines:?}"
    );
    assert!(
        lines.iter().any(|line| line.contains("2026-01-10")),
        "expected date values to avoid forced mid-token wraps: {lines:?}"
    );
}

#[test]
fn table_preserves_status_column_with_long_notes() {
    let md = "| Service | Status | Notes |\n|---|---|---|\n| Auth API | Stable | Handles login and token refresh with no major incidents in the last
  30 days. |\n| Billing Worker | Monitoring | Throughput is good, but we still see occasional retry storms when upstream settlement providers return
  partial failures. |\n| Search Indexer | Tuning | Performance improved after shard balancing, yet memory usage remains elevated during full rebuild
  windows. |\n";
    let text = crate::markdown_render::render_markdown_text_with_width(md, Some(150));
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|span| span.content.clone()).collect())
        .collect();

    assert!(
        lines.iter().any(|line| line.contains("Status")),
        "expected status header to remain readable: {lines:?}"
    );
    assert!(
        lines.iter().any(|line| line.contains("Monitoring")),
        "expected status values to avoid mid-word wraps: {lines:?}"
    );
}

#[test]
fn table_keeps_long_body_rows_inside_grid_instead_of_spilling_raw_pipe_rows() {
    let md = "| Milestone | Planned Date | Outcome | Retrospective Summary |\n|---|---|---|---|\n| Canary rollout | 2026-01-10 | Completed | Canary
  traffic was held at 5% longer than planned due to latency regressions tied to cold cache behavior; after pre-warming and query plan hints, p95
  returned to baseline and rollout resumed safely. |\n| Full region cutover | 2026-01-24 | Completed | Cutover succeeded with no customer-visible
  downtime, though internal dashboards lagged for approximately 18 minutes because ingestion workers autoscaled slower than forecast under burst load.
  |\n| Legacy decommission | 2026-02-07 | In progress | Most workloads have been drained, but final decommission is blocked by one compliance export
  task that still depends on a deprecated storage path and requires legal sign-off before removal. |\n";
    let text = crate::markdown_render::render_markdown_text_with_width(md, Some(200));
    let lines: Vec<String> = text
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|span| span.content.clone()).collect())
        .collect();

    assert!(
        lines.iter().any(|line| line.starts_with('┌'))
            && lines.iter().any(|line| line.starts_with('└')),
        "expected boxed table output: {lines:?}"
    );
    assert!(
        lines.iter().any(|line| line.contains("│ Canary rollout")),
        "expected first body row to stay inside table grid: {lines:?}"
    );
    assert!(
        !lines
            .iter()
            .any(|line| line.trim_start().starts_with("| Canary rollout |")),
        "did not expect raw pipe-form body rows outside table: {lines:?}"
    );
}
