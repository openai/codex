use codex_core::config::Config;
use codex_core::config_types::UriBasedFileOpener;
use ratatui::text::Line;
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct MarkdownCitationContext {
    file_opener: UriBasedFileOpener,
    cwd: PathBuf,
}

impl MarkdownCitationContext {
    pub(crate) fn new(file_opener: UriBasedFileOpener, cwd: PathBuf) -> Self {
        Self { file_opener, cwd }
    }
}

impl From<&Config> for MarkdownCitationContext {
    fn from(config: &Config) -> Self {
        MarkdownCitationContext::new(config.file_opener, config.cwd.clone())
    }
}

pub(crate) fn append_markdown<C>(
    markdown_source: &str,
    width: Option<usize>,
    lines: &mut Vec<Line<'static>>,
    citation_context: C,
) where
    C: Into<MarkdownCitationContext>,
{
    let citation_context: MarkdownCitationContext = citation_context.into();
    append_markdown_with_opener_and_cwd(
        markdown_source,
        width,
        lines,
        citation_context.file_opener,
        &citation_context.cwd,
    );
}

pub(crate) fn append_markdown_with_opener_and_cwd(
    markdown_source: &str,
    width: Option<usize>,
    lines: &mut Vec<Line<'static>>,
    file_opener: UriBasedFileOpener,
    cwd: &Path,
) {
    // Render via pulldown-cmark and rewrite citations during traversal (outside code blocks).
    let rendered = crate::markdown_render::render_markdown_text_with_citations(
        markdown_source,
        width,
        file_opener.get_scheme(),
        cwd,
    );
    crate::render::line_utils::push_owned_lines(&rendered.lines, lines);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn citations_not_rewritten_inside_code_blocks() {
        let src = "Before 【F:/x.rs†L1】\n```\nInside 【F:/x.rs†L2】\n```\nAfter 【F:/x.rs†L3】\n";
        let cwd = Path::new("/");
        let mut out = Vec::new();
        append_markdown_with_opener_and_cwd(src, None, &mut out, UriBasedFileOpener::VsCode, cwd);
        let rendered: Vec<String> = out
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<String>()
            })
            .collect();
        // Expect a line containing the inside text unchanged.
        assert!(rendered.iter().any(|s| s.contains("Inside 【F:/x.rs†L2】")));
        // And first/last sections rewritten.
        assert!(
            rendered
                .first()
                .map(|s| s.contains("vscode://file"))
                .unwrap_or(false)
        );
        assert!(
            rendered
                .last()
                .map(|s| s.contains("vscode://file"))
                .unwrap_or(false)
        );
    }

    #[test]
    fn indented_code_blocks_preserve_leading_whitespace() {
        // Basic sanity: indented code with surrounding blank lines should produce the indented line.
        let src = "Before\n\n    code 1\n\nAfter\n";
        let cwd = Path::new("/");
        let mut out = Vec::new();
        append_markdown_with_opener_and_cwd(src, None, &mut out, UriBasedFileOpener::None, cwd);
        let lines: Vec<String> = out
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<String>()
            })
            .collect();
        assert_eq!(lines, vec!["Before", "", "    code 1", "", "After"]);
    }

    #[test]
    fn citations_not_rewritten_inside_indented_code_blocks() {
        let src = "Start 【F:/x.rs†L1】\n\n    Inside 【F:/x.rs†L2】\n\nEnd 【F:/x.rs†L3】\n";
        let cwd = Path::new("/");
        let mut out = Vec::new();
        append_markdown_with_opener_and_cwd(src, None, &mut out, UriBasedFileOpener::VsCode, cwd);
        let rendered: Vec<String> = out
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<String>()
            })
            .collect();
        assert!(
            rendered
                .iter()
                .any(|s| s.contains("Start") && s.contains("vscode://file"))
        );
        assert!(
            rendered
                .iter()
                .any(|s| s.contains("End") && s.contains("vscode://file"))
        );
        assert!(rendered.iter().any(|s| s.contains("Inside 【F:/x.rs†L2】")));
    }

    #[test]
    fn append_markdown_preserves_full_text_line() {
        use codex_core::config_types::UriBasedFileOpener;
        use std::path::Path;
        let src = "Hi! How can I help with codex-rs today? Want me to explore the repo, run tests, or work on a specific change?\n";
        let cwd = Path::new("/");
        let mut out = Vec::new();
        append_markdown_with_opener_and_cwd(src, None, &mut out, UriBasedFileOpener::None, cwd);
        assert_eq!(
            out.len(),
            1,
            "expected a single rendered line for plain text"
        );
        let rendered: String = out
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.clone())
            .collect::<Vec<_>>()
            .join("");
        assert_eq!(
            rendered,
            "Hi! How can I help with codex-rs today? Want me to explore the repo, run tests, or work on a specific change?"
        );
    }

    #[test]
    fn append_markdown_matches_tui_markdown_for_ordered_item() {
        use codex_core::config_types::UriBasedFileOpener;
        use std::path::Path;
        let cwd = Path::new("/");
        let mut out = Vec::new();
        append_markdown_with_opener_and_cwd(
            "1. Tight item\n",
            None,
            &mut out,
            UriBasedFileOpener::None,
            cwd,
        );
        let lines: Vec<String> = out
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<String>()
            })
            .collect();
        assert_eq!(lines, vec!["1. Tight item".to_string()]);
    }

    #[test]
    fn append_markdown_keeps_ordered_list_line_unsplit_in_context() {
        use codex_core::config_types::UriBasedFileOpener;
        use std::path::Path;
        let src = "Loose vs. tight list items:\n1. Tight item\n";
        let cwd = Path::new("/");
        let mut out = Vec::new();
        append_markdown_with_opener_and_cwd(src, None, &mut out, UriBasedFileOpener::None, cwd);

        let lines: Vec<String> = out
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<String>()
            })
            .collect();

        // Expect to find the ordered list line rendered as a single line,
        // not split into a marker-only line followed by the text.
        assert!(
            lines.iter().any(|s| s == "1. Tight item"),
            "expected '1. Tight item' rendered as a single line; got: {lines:?}"
        );
        assert!(
            !lines
                .windows(2)
                .any(|w| w[0].trim_end() == "1." && w[1] == "Tight item"),
            "did not expect a split into ['1.', 'Tight item']; got: {lines:?}"
        );
    }
}
