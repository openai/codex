use super::init;
use crate::markdown::render_streaming_markdown_agent_with_links_and_cwd;
use crate::markdown_render::render_markdown_text;
use crate::terminal_hyperlinks::visible_lines;
use codex_config::types::TuiRendering;
use pretty_assertions::assert_eq;
use ratatui::text::Text;

#[test]
fn disabled_renderers_preserve_source_independently() {
    let samples = [
        "```mermaid\nflowchart LR\nA[Request] --> B[Reply]\n```\n",
        "Inline $\\alpha_1$ and \\(x^{2}\\).\n\n\\[\n\\frac{a}{b}\n\\]\n",
        "| **Name** | Value |\n|---|---|\n| `a` | path\\_1 &amp; \\* |\n",
        "- [ ] Pending\n- [x] Done\n- Ordinary\n",
    ];
    let defaults = samples.map(render_markdown_text);
    let mut snapshot = Vec::new();
    for rendering in [
        TuiRendering {
            mermaid: false,
            ..Default::default()
        },
        TuiRendering {
            math: false,
            ..Default::default()
        },
        TuiRendering {
            tables: false,
            ..Default::default()
        },
        TuiRendering {
            lists: false,
            ..Default::default()
        },
    ] {
        init(rendering);
        for (index, source) in samples.iter().enumerate() {
            let rendered = render_markdown_text(source);
            if [
                rendering.mermaid,
                rendering.math,
                rendering.tables,
                rendering.lists,
            ][index]
            {
                assert_eq!(rendered, defaults[index]);
            } else {
                snapshot.push(format!("{rendering:?}\n{rendered}"));
            }
        }
    }
    insta::assert_snapshot!(snapshot.join("\n\n---\n\n"));
}

#[test]
fn disabled_mermaid_keeps_unsupported_source_without_notice() {
    init(TuiRendering {
        mermaid: false,
        ..Default::default()
    });
    let source = "```mermaid\npie\n\"Cats\": 2\n```\n";
    let rendered = render_markdown_text(source);
    init(TuiRendering::default());
    assert_eq!(
        rendered.to_string(),
        render_markdown_text(&source.replacen("mermaid", "unknown", /*count*/ 1)).to_string(),
    );
}

#[test]
fn disabled_tables_keep_markdown_fences_and_cell_markup() {
    init(TuiRendering {
        tables: false,
        ..Default::default()
    });
    for fence in [
        "```md",
        "```markdown",
        "~~~markdown",
        "```MARKDOWN title=example",
    ] {
        let closer = &fence[..3];
        let source = format!("{fence}\n| A | B |\n|---|---|\n| **a** | $x^2$ |\n{closer}\n");
        assert_eq!(render_markdown_text(&source).to_string(), source.trim_end());
    }
    let quoted =
        "> ```md\n> Intro\n>\n>| A | B |\n> |---|---|\n> | a | b |\n>\n>> literal\n> ```\n";
    let rendered = render_markdown_text(quoted).to_string();
    insta::assert_snapshot!("disabled_tables_keep_mixed_quote_spacing", rendered);
    let streamed = render_streaming_markdown_agent_with_links_and_cwd(
        quoted,
        /*width*/ None,
        /*cwd*/ None,
        crate::markdown_render::ListSpacing::AfterMultiline,
    );
    assert_eq!(
        Text::from(visible_lines(streamed.lines)).to_string(),
        rendered
    );
    // Four-space continuation indentation must not hide the table inside a list fence.
    for (marker, indent) in [("1.  ", "    "), ("> > 1.  ", ">>     ")] {
        let listed = format!(
            "{marker}```md\n{indent}| A | B |\n{indent}|---|---|\n{indent}| a | b |\n{indent}```\n"
        );
        let text = render_markdown_text(&listed).to_string();
        assert!(text.contains("```md"));
        assert_eq!(text.matches("```").count(), 2);
        assert!(text.contains("| a | b |"));
    }
    let prose = "```markdown\n**Not a table**\n```\n";
    assert_eq!(render_markdown_text(prose).to_string(), "**Not a table**");
}
