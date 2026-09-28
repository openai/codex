//! Shared directive grammar and caller-specific quote compatibility.

use super::AssistantDirective;
use super::QuoteEscaping;
use super::parse_assistant_directive;
use super::parse_assistant_directive_with_budget;
use pretty_assertions::assert_eq;
use std::borrow::Cow;
use std::collections::BTreeMap;

#[test]
fn parses_git_receipts_with_unquoted_attributes() {
    let raw = r#"::git-create-pr{cwd="/repo\" branch="feature/report" isDraft=true}"#;

    assert_eq!(
        parse_assistant_directive(raw, QuoteEscaping::Literal),
        Some(AssistantDirective {
            name: "git-create-pr",
            label: None,
            attributes: BTreeMap::from([
                ("branch", Cow::Borrowed("feature/report")),
                ("cwd", Cow::Borrowed(r"/repo\")),
                ("isDraft", Cow::Borrowed("true")),
            ]),
            raw,
        }),
    );
}

#[test]
fn parses_triple_colon_comments_with_escaped_quotes_and_closing_braces() {
    let raw = r#":::code-comment{title="Fix class" body="Keep \"px-${size}\" literal." file="C:\Users\me\app.ts" start=10 priority=2}"#;
    let source = format!("{raw} remaining text");

    assert_eq!(
        parse_assistant_directive(&source, QuoteEscaping::Backslash),
        Some(AssistantDirective {
            name: "code-comment",
            label: None,
            attributes: BTreeMap::from([
                (
                    "body",
                    Cow::Owned(r#"Keep "px-${size}" literal."#.to_string())
                ),
                ("file", Cow::Borrowed(r"C:\Users\me\app.ts")),
                ("priority", Cow::Borrowed("2")),
                ("start", Cow::Borrowed("10")),
                ("title", Cow::Borrowed("Fix class")),
            ]),
            raw,
        }),
    );
}

#[test]
fn preserves_artifact_metadata_and_single_quoted_values() {
    let raw = ":artifact{path='Quarterly Report.xlsx' label='team\\'s report' sheet='Revenue' range='A1:D8'}";

    assert_eq!(
        parse_assistant_directive(raw, QuoteEscaping::Backslash),
        Some(AssistantDirective {
            name: "artifact",
            label: None,
            attributes: BTreeMap::from([
                ("label", Cow::Owned("team's report".to_string())),
                ("path", Cow::Borrowed("Quarterly Report.xlsx")),
                ("range", Cow::Borrowed("A1:D8")),
                ("sheet", Cow::Borrowed("Revenue")),
            ]),
            raw,
        }),
    );
}

#[test]
fn rejects_ambiguous_or_incomplete_directives() {
    for source in [
        r#"::git-push{cwd="/repo" cwd="/other"}"#,
        r#"::git-push{cwd="/repo" branch="feature""#,
        "::git-push[anything]{cwd=/repo branch=main}",
        "::code-comment[anything]{body=comment file=foo}",
        ":codex-file-citation[caption]{path=foo}",
        "::::code-comment{title=comment}",
        ":artifact{path=/tmp/a path=:artifact{path=/tmp/b}}",
        ":artifact{invalid:key=:artifact{path=/tmp/a}}",
        ":artifact{path=/tmp/a\nnext=value}",
    ] {
        assert_eq!(
            parse_assistant_directive(source, QuoteEscaping::Backslash),
            None
        );
    }
}

#[test]
fn malformed_retries_exhaust_the_shared_scan_budget() {
    let source = format!(
        "codex-file-citation {} x bad}}",
        ":a{k=".repeat(/*n*/ 16_000)
    );
    let mut remaining = source.len() * 4;
    let mut attempts = 0;
    for (offset, _) in source.match_indices(':') {
        if remaining == 0 {
            break;
        }
        assert_eq!(
            parse_assistant_directive_with_budget(
                &source[offset..],
                QuoteEscaping::Literal,
                &mut remaining,
            ),
            None,
        );
        attempts += 1;
    }
    assert!(attempts <= 8, "retried {attempts} long malformed values");
    assert_eq!(remaining, 0);
}

#[test]
fn scan_budget_counts_inspected_values_not_the_unread_suffix() {
    let tail = "x".repeat(/*n*/ 16_000);
    let raw = ":artifact{path=report.xlsx}";
    let source = format!("{raw}{tail}");
    let mut remaining = 64;
    assert_eq!(
        parse_assistant_directive_with_budget(&source, QuoteEscaping::Literal, &mut remaining),
        parse_assistant_directive(raw, QuoteEscaping::Literal),
    );
    for source in [
        format!(":artifact{{path=\"{tail}"),
        format!(":artifact{{path={tail}"),
    ] {
        let mut remaining = 64;
        assert_eq!(
            parse_assistant_directive_with_budget(&source, QuoteEscaping::Literal, &mut remaining),
            None,
        );
        assert_eq!(remaining, 0);
    }
}
