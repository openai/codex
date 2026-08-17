use super::super::MAX_HIGHLIGHT_BYTES;
use super::super::MAX_HIGHLIGHT_LINE_BYTES;
use super::super::MAX_HIGHLIGHT_LINES;
use super::super::highlight_code_to_lines;
use super::super::syntax_theme_revision;
use super::StreamingCodeHighlighter;
use pretty_assertions::assert_eq;

#[test]
fn appended_lines_preserve_multiline_syntax_state() {
    let first = "fn main() {\n    /* comment\n";
    let appended = "       continues */\n    println!(\"界 e\u{301}\");\n}\n";
    let code = format!("{first}{appended}");
    let highlighter =
        StreamingCodeHighlighter::new(first, "rust", syntax_theme_revision()).unwrap();
    let (_, lines) = highlighter.append(appended).unwrap();

    assert_eq!(
        lines,
        highlight_code_to_lines(&code, "rust")
            .into_iter()
            .skip(first.lines().count())
            .collect::<Vec<_>>(),
    );
}

#[test]
fn empty_and_unknown_language_fences_append() {
    for lang in ["rust", "unknown-language"] {
        let highlighter = StreamingCodeHighlighter::new("", lang, syntax_theme_revision()).unwrap();
        let (_, lines) = highlighter.append("fn main() {}\n\n").unwrap();
        assert_eq!(lines, highlight_code_to_lines("fn main() {}\n\n", lang));
    }
}

#[test]
fn incomplete_lines_and_theme_changes_discard_state() {
    let highlighter =
        StreamingCodeHighlighter::new("fn first() {}\n", "rust", syntax_theme_revision()).unwrap();
    assert!(highlighter.append("fn partial()").is_none());

    let mut highlighter =
        StreamingCodeHighlighter::new("fn first() {}\n", "rust", syntax_theme_revision()).unwrap();
    let state = highlighter.state.as_mut().unwrap();
    state.theme_revision = state.theme_revision.wrapping_sub(1);
    assert!(highlighter.append("fn second() {}\n").is_none());
}

#[test]
fn crossing_each_highlighting_limit_requires_a_canonical_render() {
    let cases = [
        (
            String::new(),
            format!("{}\n", "x".repeat(MAX_HIGHLIGHT_LINE_BYTES + 1)),
        ),
        ("x\n".repeat(MAX_HIGHLIGHT_LINES), "x\n".to_string()),
        (
            format!("{}\n", "x".repeat(1023)).repeat(MAX_HIGHLIGHT_BYTES / 1024),
            "x\n".to_string(),
        ),
    ];
    for (initial, appended) in cases {
        let highlighter =
            StreamingCodeHighlighter::new(&initial, "rust", syntax_theme_revision()).unwrap();
        assert!(highlighter.state.is_some());
        assert!(highlighter.append(&appended).is_none());

        let complete = format!("{initial}{appended}");
        let highlighter =
            StreamingCodeHighlighter::new(&complete, "rust", syntax_theme_revision()).unwrap();
        assert!(highlighter.state.is_none());
        let (_, lines) = highlighter.append("next\n").unwrap();
        assert_eq!(lines, highlight_code_to_lines("next\n", "unknown-language"));
    }
}
