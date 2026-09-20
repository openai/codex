//! Width-aware list policies preserve nested layout, paragraphs, and semantic link metadata.

use super::ListSpacing;
use crate::markdown::render_markdown_agent_with_list_spacing;
use pretty_assertions::assert_eq;

#[test]
fn list_spacing_is_uniform_per_list_and_preserves_item_paragraphs() {
    let sources = [
        "1. One\n2. This item has enough words to wrap\n3. Three\n4. Four",
        "- Parent\n  - Short\n  - Small\n- Other\n\n> - Quote\n> - This quoted item has enough words to wrap\n> - End",
        "- First paragraph\n\n  Second paragraph\n\n- Next\n\n- Last",
        "- One\n- ```text\n  code\n  ```\n- Three\n\n  ```text\n  after paragraph\n  ```",
        "- https://example.com/a-very-long-path-with-many-segments\n- Short\n- End",
    ];
    let mut stages = Vec::new();
    for source in sources {
        for spacing in [ListSpacing::Compact, ListSpacing::Uniform] {
            let lines = render_markdown_agent_with_list_spacing(
                source,
                Some(24),
                /*cwd*/ None,
                /*inline_visualization_context*/ None,
                spacing,
            );
            stages.push(format!(
                "{spacing:?}:\n{}",
                lines
                    .iter()
                    .map(|line| line.line.to_string().trim_end().to_owned())
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }
    }
    insta::assert_snapshot!(stages.join("\n\n---\n\n"));
}

#[test]
fn list_spacing_preserves_links_and_source_when_removing_separators() {
    let source = "- [One](https://example.com/one)\n- [Two](https://example.com/two)\n- Three";
    let render = |spacing| {
        render_markdown_agent_with_list_spacing(
            source,
            Some(100),
            /*cwd*/ None,
            /*inline_visualization_context*/ None,
            spacing,
        )
    };
    let compact = render(ListSpacing::Compact);
    let uniform = render(ListSpacing::Uniform);
    assert_eq!(&compact, &uniform);
    assert_eq!(
        compact.iter().map(|line| &line.source).collect::<Vec<_>>(),
        uniform.iter().map(|line| &line.source).collect::<Vec<_>>()
    );
}
