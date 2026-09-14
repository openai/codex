//! Citation rendering, literal Markdown boundaries, and filesystem-path compatibility.

use crate::markdown::render_markdown_agent_with_links_and_cwd;
use crate::markdown_render::render_markdown_lines_with_width_and_cwd;
use itertools::Itertools;
use pretty_assertions::assert_eq;
use std::path::Path;

fn rendered_text(markdown: &str, cwd: Option<&Path>) -> String {
    render_markdown_agent_with_links_and_cwd(markdown, /*width*/ None, cwd)
        .into_iter()
        .map(|line| line.line.to_string())
        .join("\n")
}

#[test]
fn file_citation_paths_preserve_markdown_significant_characters() {
    for path in [
        "/tmp/a*b*.txt",
        "/tmp/$x$/report.md",
        "/tmp/a`b`.txt",
        "/tmp/a<b>.txt",
        "/tmp/report#L10",
        "/tmp/report:10",
        "/tmp/report%20final.xlsx",
        "/tmp/report?final.xlsx",
    ] {
        let markdown = format!(":codex-file-citation{{path=\"{path}\"}}");
        assert_eq!(rendered_text(&markdown, /*cwd*/ None), path);
    }
}

#[test]
fn file_url_citation_preserves_literal_query_delimiters_snapshot() {
    let cwd = std::env::temp_dir();
    let directory = url::Url::from_directory_path(&cwd).unwrap();
    for (cwd, directory) in [
        (cwd.as_path(), directory.as_str()),
        (Path::new("C:/repo"), "file:///C:/repo/"),
        (Path::new("C:/repo"), "file://localhost/C:/repo/"),
    ] {
        let markdown = format!(":codex-file-citation{{path=\"{directory}report?final.xlsx\"}}");
        insta::allow_duplicates! {
            insta::assert_snapshot!(rendered_text(&markdown, Some(cwd)), @"report?final.xlsx");
        }
    }
}

#[test]
fn file_citations_inside_code_existing_links_and_html_remain_literal() {
    let citation = r#":codex-file-citation{path="/tmp/report.xlsx" purpose="output"}"#;

    for markdown in [
        format!("`{citation}`"),
        format!("```text\n{citation}\n```\n"),
    ] {
        assert_eq!(rendered_text(&markdown, /*cwd*/ None), citation);
    }
    for markdown in [
        format!("<span title='{citation}'>"),
        format!("<!-- {citation} -->"),
    ] {
        assert_eq!(rendered_text(&markdown, /*cwd*/ None), markdown);
    }
    assert_eq!(
        rendered_text(
            &format!("[{citation}](https://example.com)"),
            /*cwd*/ None
        ),
        format!("{citation} (https://example.com)"),
    );
}

#[test]
fn file_citations_accept_unquoted_paths_and_trailing_windows_separators() {
    for (citation, expected) in [
        (
            r#":codex-file-citation{path="C:\Users\me\.codex\report.xlsx"}"#,
            "C:/Users/me/.codex/report.xlsx",
        ),
        (
            ":codex-file-citation{path=/tmp/a*b*.txt purpose=output}",
            "/tmp/a*b*.txt",
        ),
        (
            r#":codex-file-citation{path="C:\repo\" purpose="output" sheet="Q1=Actual"}"#,
            "C:/repo/",
        ),
        (
            r#":codex-file-citation{path="/tmp/a\"b" label="team's \"report\""}"#,
            "/tmp/a\"b",
        ),
    ] {
        assert_eq!(rendered_text(citation, /*cwd*/ None), expected);
    }
}

#[test]
fn file_citations_preserve_escaped_nested_and_reference_directives() {
    let citation = ":codex-file-citation{path=/tmp/report.xlsx}";

    assert_eq!(
        rendered_text(&format!(r"\{citation}"), /*cwd*/ None),
        citation,
    );
    assert_eq!(
        rendered_text(&format!(r"\{citation} and {citation}"), /*cwd*/ None),
        format!("{citation} and /tmp/report.xlsx"),
    );

    for literal in [
        format!(r#":unsupported{{value="{citation}"}}"#),
        format!(":::{citation}"),
        format!("codex-file-citation {}", ":x{v=".repeat(16_000)),
        format!("codex-file-citation {}}}", ":x{v=a v=".repeat(16_000)),
        format!(
            "codex-file-citation {} x bad}}",
            ":a{k=".repeat(/*n*/ 16_000)
        ),
    ] {
        assert_eq!(rendered_text(&literal, /*cwd*/ None), literal);
    }

    // Normal Markdown escaping still applies, but the nested citation is not rendered.
    let literal = format!(r#":unsupported{{path="C:\repo\" value="{citation}"}}"#);
    let rendered = rendered_text(&literal, /*cwd*/ None);
    insta::assert_snapshot!(rendered, @r#":unsupported{path="C:\repo" value=":codex-file-citation{path=/tmp/report.xlsx}"}"#);

    assert_eq!(
        rendered_text(
            &format!("`{citation}`\n\n[report][file]\n\n[file]: {citation}\n\n{citation}"),
            /*cwd*/ None,
        ),
        format!("{citation}\n\nreport ({citation})\n\n/tmp/report.xlsx"),
    );
}

#[test]
fn multiple_file_citations_render_without_interpreting_encoded_source() {
    let cwd = std::env::temp_dir();
    let second = cwd.join("second.xlsx");
    let markdown = format!(
        r#"&#58;codex-file-citation{{path="ignored.xlsx"}} :codex-file-citation{{artifact_kind="workbook" path="reports/final%20report.xlsx" purpose="output" sheet="Dashboard" range="A1:D8"}} and ::codex-file-citation{{path="{}"}}"#,
        second.display(),
    );

    assert_eq!(
        rendered_text(&markdown, Some(&cwd)),
        r#":codex-file-citation{path="ignored.xlsx"} reports/final%20report.xlsx and second.xlsx"#,
    );

    // A work budget must not become a fixed count limit on legitimate citations.
    let many = [":codex-file-citation{path=/tmp/report.xlsx}"; 512].join(" ");
    assert_eq!(
        rendered_text(&many, /*cwd*/ None),
        ["/tmp/report.xlsx"; 512].join(" "),
    );
}

#[test]
fn file_citations_preserve_adjacent_entities_and_escaped_punctuation() {
    assert_eq!(
        rendered_text(
            r"$\alpha$:codex-file-citation{path=/tmp/$x$/report.md}$\beta$",
            /*cwd*/ None
        ),
        "α/tmp/$x$/report.mdβ",
    );
    let cwd = std::env::temp_dir();
    for (markdown, expected) in [
        (
            r#"Préface &amp;:codex-file-citation{path="first&amp;.txt"}&#x1F642;:codex-file-citation{path="second.txt"}&lt;fin&gt;"#,
            "Préface &first&amp;.txt🙂second.txt<fin>",
        ),
        (
            r#"\[:codex-file-citation{path="first.txt"}\]\*:codex-file-citation{path="second.txt"}\!"#,
            "[first.txt]*second.txt!",
        ),
    ] {
        assert_eq!(rendered_text(markdown, Some(&cwd)), expected);
    }
}

#[test]
fn file_citation_after_local_link_soft_break_starts_a_new_line_snapshot() {
    let cwd = std::env::temp_dir();
    let markdown = format!(
        "[first](<{}>)\n:codex-file-citation{{path=\"second.txt\"}}",
        cwd.join("first.txt").display(),
    );
    let rendered = rendered_text(&markdown, Some(&cwd));

    insta::assert_snapshot!(rendered, @r"
    first (first.txt)
    second.txt
    ");
}

#[test]
fn generic_markdown_keeps_assistant_directives_literal() {
    let citation = r#":codex-file-citation{path="/tmp/report.xlsx"}"#;

    assert_eq!(
        render_markdown_lines_with_width_and_cwd(citation, /*width*/ None, /*cwd*/ None)
            .into_iter()
            .map(|line| line.line.to_string())
            .join("\n"),
        citation,
    );
}
