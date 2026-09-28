//! Follow-up labels keep Markdown styling and stream offsets without exposing prompts.

use super::super::ListSpacing;
use super::super::render_streaming_markdown_lines_with_width_and_cwd;
use crate::markdown::render_markdown_agent_with_links_and_cwd;
use crate::terminal_hyperlinks::visible_lines;
use pretty_assertions::assert_eq;
use ratatui::text::Text;

#[test]
fn followup_labels_render_like_ordinary_markdown_snapshot() {
    let source = concat!(
        "Next actions:\n\n",
        "- :codex-followup[Add **revenue charts**]{prompt=\"Add a \\\"Q3\\\" chart with } labels\"}\n",
        "- :codex-followup[Format `Sheet1`]{prompt=\"Format the spreadsheet\"}\n\n",
        "Or :codex-followup[explain café totals]{prompt=\"Explain $x$ and *all* totals\"}.\n",
    );
    let plain =
        "Next actions:\n\n- Add **revenue charts**\n- Format `Sheet1`\n\nOr explain café totals.\n";
    let streamed = render_streaming_markdown_lines_with_width_and_cwd(
        source,
        Some(32),
        /*cwd*/ None,
        &|_| false,
        ListSpacing::AfterMultiline,
    );
    let rendered = Text::from(visible_lines(streamed.lines));
    assert_eq!(
        rendered,
        Text::from(visible_lines(render_markdown_agent_with_links_and_cwd(
            plain,
            Some(32),
            /*cwd*/ None
        )))
    );
    assert_eq!(streamed.last_top_level_block_start, source.find("Or "));
    insta::assert_snapshot!(rendered.to_string(), @"
    Next actions:

    • Add revenue charts
    • Format Sheet1

    Or explain café totals.
    ");
}

#[test]
fn followups_preserve_literal_and_malformed_source() {
    let directive = r#":codex-followup[Add charts]{prompt="Add charts"}"#;
    for source in [
        format!("`{directive}`"),
        format!("```text\n{directive}\n```"),
        format!(r"\{directive}"),
        format!("<!-- {directive} -->"),
        format!("[{directive}](https://example.com)"),
        ":codex-followup[Add charts]{prompt=\"unfinished".to_string(),
        ":codex-followup[Add charts".to_string(),
        ":codex-followup[Add charts]{prompt=one prompt=two}".to_string(),
    ] {
        assert_eq!(
            super::InlineDirectives::new(&source, pulldown_cmark::Options::empty()).markdown,
            source
        );
    }
}

#[test]
fn followups_do_not_consume_neighbors_or_nested_directives() {
    let source = concat!(
        "&amp; :codex-followup[**First**]{prompt=\"Use :codex-file-citation{path=\\\"/tmp/private\\\"}\"} ",
        ":codex-followup[*Second*]{prompt=\"Then continue\"} &copy;\n",
    );
    assert_eq!(
        render_markdown_agent_with_links_and_cwd(source, /*width*/ None, /*cwd*/ None),
        render_markdown_agent_with_links_and_cwd(
            "&amp; **First** *Second* &copy;\n",
            /*width*/ None,
            /*cwd*/ None
        ),
    );
    let source = format!("{source}\n:codex-file-citation{{path=\"/tmp/report.pdf\"}}\n");
    let rendered = crate::markdown::render_markdown_agent_with_links_and_cwd(
        &source, /*width*/ None, /*cwd*/ None,
    );
    assert_eq!(
        Text::from(visible_lines(rendered)).to_string(),
        "& First Second ©\n\n/tmp/report.pdf"
    );
}

#[test]
fn followup_labels_cannot_introduce_blocks() {
    for label in [
        "# Heading",
        "- List item",
        "---",
        "    Indented",
        "~~Old~~ new",
        "Inspect `items[0]`",
        "Inspect ``items[`key`]``",
        "Type a ` character",
        "Type a ` character and ``items]``",
        r"Type a \` character",
        "Inspect [nested [labels]]",
        r"Inspect \] and \[ brackets",
    ] {
        let source = format!("Before :codex-followup[{label}]{{prompt=action}} after.");
        let plain = format!("Before {label} after.");
        assert_eq!(
            render_markdown_agent_with_links_and_cwd(
                &source, /*width*/ None, /*cwd*/ None
            ),
            render_markdown_agent_with_links_and_cwd(
                &plain, /*width*/ None, /*cwd*/ None
            ),
        );
    }
}

#[test]
fn unmatched_label_backticks_render_and_copy_as_literal_text() {
    let source = ":codex-followup[Type a ` character]{prompt=action}\n";
    insta::assert_snapshot!(
        Text::from(visible_lines(render_markdown_agent_with_links_and_cwd(
            source, /*width*/ None, /*cwd*/ None
        ))).to_string(),
        @"Type a ` character"
    );
    assert_eq!(super::followup_labels(source), "Type a ` character\n");
}

#[test]
fn malformed_directives_leave_budget_for_followups() {
    for prefix in [":x0[:x1[", ":codex-followup[` "] {
        let source = format!("{prefix}:codex-followup[Open]{{prompt=secret}}");
        let expected = format!("{prefix}Open");
        assert_eq!(super::followup_labels(&source), expected);
        assert_eq!(
            render_markdown_agent_with_links_and_cwd(
                &source, /*width*/ None, /*cwd*/ None
            ),
            render_markdown_agent_with_links_and_cwd(
                &expected, /*width*/ None, /*cwd*/ None
            ),
        );
    }
}
