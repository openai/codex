use crate::citation_regex::CITATION_REGEX;
use codex_core::config::Config;
use codex_core::config_types::UriBasedFileOpener;
use ratatui::text::Line;
use ratatui::text::Span;
use std::borrow::Cow;
use std::path::Path;

pub(crate) fn append_markdown(
    markdown_source: &str,
    lines: &mut Vec<Line<'static>>,
    config: &Config,
) {
    append_markdown_with_opener_and_cwd(markdown_source, lines, config.file_opener, &config.cwd);
}

fn append_markdown_with_opener_and_cwd(
    markdown_source: &str,
    lines: &mut Vec<Line<'static>>,
    file_opener: UriBasedFileOpener,
    cwd: &Path,
) {
    // Historically, we fed the entire `markdown_source` into the renderer in
    // one pass. However, fenced code blocks sometimes lost leading whitespace
    // when formatted by the markdown renderer/highlighter. To preserve code
    // block content exactly, split the source into "text" and "code" segments:
    // - Render non-code text through `tui_markdown` (with citation rewrite).
    // - Render code block content verbatim as plain lines without additional
    //   formatting, preserving leading spaces.
    for seg in split_text_and_fences(markdown_source) {
        match seg {
            Segment::Text(s) => {
                let processed = rewrite_file_citations(&s, file_opener, cwd);
                let rendered = tui_markdown::from_str(&processed);
                crate::render::line_utils::push_owned_lines(&rendered.lines, lines);
            }
            Segment::Code { content, .. } => {
                // Emit the code content exactly as-is, line by line.
                // We don't attempt syntax highlighting to avoid whitespace bugs.
                for line in content.split_inclusive('\n') {
                    // split_inclusive keeps the trailing \n; we want lines without it.
                    let line = if let Some(stripped) = line.strip_suffix('\n') {
                        stripped
                    } else {
                        line
                    };
                    let owned_line: Line<'static> = Line::from(Span::raw(line.to_string()));
                    lines.push(owned_line);
                }
            }
        }
    }
}

/// Rewrites file citations in `src` into markdown hyperlinks using the
/// provided `scheme` (`vscode`, `cursor`, etc.). The resulting URI follows the
/// format expected by VS Code-compatible file openers:
///
/// ```text
/// <scheme>://file<ABS_PATH>:<LINE>
/// ```
fn rewrite_file_citations<'a>(
    src: &'a str,
    file_opener: UriBasedFileOpener,
    cwd: &Path,
) -> Cow<'a, str> {
    // Map enum values to the corresponding URI scheme strings.
    let scheme: &str = match file_opener.get_scheme() {
        Some(scheme) => scheme,
        None => return Cow::Borrowed(src),
    };

    CITATION_REGEX.replace_all(src, |caps: &regex_lite::Captures<'_>| {
        let file = &caps[1];
        let start_line = &caps[2];

        // Resolve the path against `cwd` when it is relative.
        let absolute_path = {
            let p = Path::new(file);
            let absolute_path = if p.is_absolute() {
                path_clean::clean(p)
            } else {
                path_clean::clean(cwd.join(p))
            };
            // VS Code expects forward slashes even on Windows because URIs use
            // `/` as the path separator.
            absolute_path.to_string_lossy().replace('\\', "/")
        };

        // Render as a normal markdown link so the downstream renderer emits
        // the hyperlink escape sequence (when supported by the terminal).
        //
        // In practice, sometimes multiple citations for the same file, but with a
        // different line number, are shown sequentially, so we:
        // - include the line number in the label to disambiguate them
        // - add a space after the link to make it easier to read
        format!("[{file}:{start_line}]({scheme}://file{absolute_path}:{start_line}) ")
    })
}

// use shared helper from `line_utils`

// Minimal code block splitting.
// - Recognizes fenced blocks opened by ``` or ~~~ (allowing leading whitespace).
//   The opening fence may include a language string which we ignore.
//   The closing fence must be on its own line (ignoring surrounding whitespace).
// - Additionally recognizes indented code blocks that begin after a blank line
//   with a line starting with at least 4 spaces or a tab, and continue for
//   consecutive lines that are blank or also indented by >= 4 spaces or a tab.
enum Segment {
    Text(String),
    Code {
        _lang: Option<String>,
        content: String,
    },
}

fn split_text_and_fences(src: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut curr_text = String::new();
    #[derive(Copy, Clone, PartialEq)]
    enum CodeMode {
        None,
        Fenced,
        Indented,
    }
    let mut code_mode = CodeMode::None;
    let mut fence_token = "";
    let mut code_lang: Option<String> = None;
    let mut code_content = String::new();
    // We intentionally do not require a preceding blank line for indented code blocks,
    // since streamed model output often omits it. This favors preserving indentation.

    for line in src.split_inclusive('\n') {
        let line_no_nl = line.strip_suffix('\n');
        let trimmed_start = match line_no_nl {
            Some(l) => l.trim_start(),
            None => line.trim_start(),
        };
        if code_mode == CodeMode::None {
            let open = if trimmed_start.starts_with("```") {
                Some("```")
            } else if trimmed_start.starts_with("~~~") {
                Some("~~~")
            } else {
                None
            };
            if let Some(tok) = open {
                // Flush pending text segment.
                if !curr_text.is_empty() {
                    segments.push(Segment::Text(curr_text.clone()));
                    curr_text.clear();
                }
                fence_token = tok;
                // Capture language after the token on this line (before newline).
                let after = &trimmed_start[tok.len()..];
                let lang = after.trim();
                code_lang = if lang.is_empty() {
                    None
                } else {
                    Some(lang.to_string())
                };
                code_mode = CodeMode::Fenced;
                code_content.clear();
                // Do not include the opening fence line in output.
                continue;
            }
            // Check for start of an indented code block: only after a blank line
            // (or at the beginning), and the line must start with >=4 spaces or a tab.
            let raw_line = match line_no_nl {
                Some(l) => l,
                None => line,
            };
            let leading_spaces = raw_line.chars().take_while(|c| *c == ' ').count();
            let starts_with_tab = raw_line.starts_with('\t');
            // Consider any line that begins with >=4 spaces or a tab to start an
            // indented code block. This favors preserving indentation even when a
            // preceding blank line is omitted (common in streamed model output).
            let starts_indented_code = (leading_spaces >= 4) || starts_with_tab;
            if starts_indented_code {
                // Flush pending text and begin an indented code block.
                if !curr_text.is_empty() {
                    segments.push(Segment::Text(curr_text.clone()));
                    curr_text.clear();
                }
                code_mode = CodeMode::Indented;
                code_content.clear();
                code_content.push_str(line);
                // Inside code now; do not treat this line as normal text.
                continue;
            }
            // Normal text line.
            curr_text.push_str(line);
        } else {
            match code_mode {
                CodeMode::Fenced => {
                    // inside fenced code: check for closing fence on its own line
                    let trimmed = match line_no_nl {
                        Some(l) => l.trim(),
                        None => line.trim(),
                    };
                    if trimmed == fence_token {
                        // End code block: emit segment without fences
                        segments.push(Segment::Code {
                            _lang: code_lang.take(),
                            content: code_content.clone(),
                        });
                        code_content.clear();
                        code_mode = CodeMode::None;
                        fence_token = "";
                        continue;
                    }
                    // Accumulate code content exactly as-is.
                    code_content.push_str(line);
                }
                CodeMode::Indented => {
                    // Continue while the line is blank, or starts with >=4 spaces, or a tab.
                    let raw_line = match line_no_nl {
                        Some(l) => l,
                        None => line,
                    };
                    let is_blank = raw_line.trim().is_empty();
                    let leading_spaces = raw_line.chars().take_while(|c| *c == ' ').count();
                    let starts_with_tab = raw_line.starts_with('\t');
                    if is_blank || leading_spaces >= 4 || starts_with_tab {
                        code_content.push_str(line);
                    } else {
                        // Close the indented code block and reprocess this line as normal text.
                        segments.push(Segment::Code {
                            _lang: None,
                            content: code_content.clone(),
                        });
                        code_content.clear();
                        code_mode = CodeMode::None;
                        // Now handle current line as text.
                        curr_text.push_str(line);
                    }
                }
                CodeMode::None => unreachable!(),
            }
        }
    }

    if code_mode != CodeMode::None {
        // Unterminated code fence: treat accumulated content as a code segment.
        segments.push(Segment::Code {
            _lang: code_lang.take(),
            content: code_content.clone(),
        });
    } else if !curr_text.is_empty() {
        segments.push(Segment::Text(curr_text.clone()));
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn citation_is_rewritten_with_absolute_path() {
        let markdown = "See 【F:/src/main.rs†L42-L50】 for details.";
        let cwd = Path::new("/workspace");
        let result = rewrite_file_citations(markdown, UriBasedFileOpener::VsCode, cwd);

        assert_eq!(
            "See [/src/main.rs:42](vscode://file/src/main.rs:42)  for details.",
            result
        );
    }

    #[test]
    fn citation_is_rewritten_with_relative_path() {
        let markdown = "Refer to 【F:lib/mod.rs†L5】 here.";
        let cwd = Path::new("/home/user/project");
        let result = rewrite_file_citations(markdown, UriBasedFileOpener::Windsurf, cwd);

        assert_eq!(
            "Refer to [lib/mod.rs:5](windsurf://file/home/user/project/lib/mod.rs:5)  here.",
            result
        );
    }

    #[test]
    fn citation_followed_by_space_so_they_do_not_run_together() {
        let markdown = "References on lines 【F:src/foo.rs†L24】【F:src/foo.rs†L42】";
        let cwd = Path::new("/home/user/project");
        let result = rewrite_file_citations(markdown, UriBasedFileOpener::VsCode, cwd);

        assert_eq!(
            "References on lines [src/foo.rs:24](vscode://file/home/user/project/src/foo.rs:24) [src/foo.rs:42](vscode://file/home/user/project/src/foo.rs:42) ",
            result
        );
    }

    #[test]
    fn citation_unchanged_without_file_opener() {
        let markdown = "Look at 【F:file.rs†L1】.";
        let cwd = Path::new("/");
        let unchanged = rewrite_file_citations(markdown, UriBasedFileOpener::VsCode, cwd);
        // The helper itself always rewrites – this test validates behaviour of
        // append_markdown when `file_opener` is None.
        let mut out = Vec::new();
        append_markdown_with_opener_and_cwd(markdown, &mut out, UriBasedFileOpener::None, cwd);
        // Convert lines back to string for comparison.
        let rendered: String = out
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.clone())
            .collect::<Vec<_>>()
            .join("");
        assert_eq!(markdown, rendered);
        // Ensure helper rewrites.
        assert_ne!(markdown, unchanged);
    }

    #[test]
    fn fenced_code_blocks_preserve_leading_whitespace() {
        let src = "```\n  indented\n\t\twith tabs\n    four spaces\n```\n";
        let cwd = Path::new("/");
        let mut out = Vec::new();
        append_markdown_with_opener_and_cwd(src, &mut out, UriBasedFileOpener::None, cwd);
        let rendered: Vec<String> = out
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<String>()
            })
            .collect();
        assert_eq!(
            rendered,
            vec![
                "  indented".to_string(),
                "\t\twith tabs".to_string(),
                "    four spaces".to_string()
            ]
        );
    }

    #[test]
    fn citations_not_rewritten_inside_code_blocks() {
        let src = "Before 【F:/x.rs†L1】\n```\nInside 【F:/x.rs†L2】\n```\nAfter 【F:/x.rs†L3】\n";
        let cwd = Path::new("/");
        let mut out = Vec::new();
        append_markdown_with_opener_and_cwd(src, &mut out, UriBasedFileOpener::VsCode, cwd);
        let rendered: Vec<String> = out
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<String>()
            })
            .collect();
        // Expect first and last lines rewritten, middle line unchanged.
        assert!(rendered[0].contains("vscode://file"));
        assert_eq!(rendered[1], "Inside 【F:/x.rs†L2】");
        assert!(matches!(rendered.last(), Some(s) if s.contains("vscode://file")));
    }

    #[test]
    fn indented_code_blocks_preserve_leading_whitespace() {
        let src = "Before\n    code 1\n\tcode with tab\n        code 2\nAfter\n";
        let cwd = Path::new("/");
        let mut out = Vec::new();
        append_markdown_with_opener_and_cwd(src, &mut out, UriBasedFileOpener::None, cwd);
        let rendered: Vec<String> = out
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<String>()
            })
            .collect();
        assert_eq!(
            rendered,
            vec![
                "Before".to_string(),
                "    code 1".to_string(),
                "\tcode with tab".to_string(),
                "        code 2".to_string(),
                "After".to_string()
            ]
        );
    }

    #[test]
    fn citations_not_rewritten_inside_indented_code_blocks() {
        let src = "Start 【F:/x.rs†L1】\n\n    Inside 【F:/x.rs†L2】\n\nEnd 【F:/x.rs†L3】\n";
        let cwd = Path::new("/");
        let mut out = Vec::new();
        append_markdown_with_opener_and_cwd(src, &mut out, UriBasedFileOpener::VsCode, cwd);
        let rendered: Vec<String> = out
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<String>()
            })
            .collect();
        // Expect first and last lines rewritten, and the indented code line present
        // unchanged (citations inside not rewritten). We do not assert on blank
        // separator lines since the markdown renderer may normalize them.
        assert!(rendered.iter().any(|s| s.contains("vscode://file")));
        assert!(rendered.iter().any(|s| s == "    Inside 【F:/x.rs†L2】"));
    }

    #[test]
    fn append_markdown_preserves_full_text_line() {
        use codex_core::config_types::UriBasedFileOpener;
        use std::path::Path;
        let src = "Hi! How can I help with codex-rs today? Want me to explore the repo, run tests, or work on a specific change?\n";
        let cwd = Path::new("/");
        let mut out = Vec::new();
        append_markdown_with_opener_and_cwd(src, &mut out, UriBasedFileOpener::None, cwd);
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
    fn tui_markdown_splits_ordered_marker_and_text() {
        // With marker and content on the same line, tui_markdown keeps it as one line
        // even in the surrounding section context.
        let rendered = tui_markdown::from_str("Loose vs. tight list items:\n1. Tight item\n");
        let lines: Vec<String> = rendered
            .lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<String>()
            })
            .collect();
        assert!(
            lines.iter().any(|w| w == "1. Tight item"),
            "expected single line '1. Tight item' in context: {lines:?}"
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
    fn tui_markdown_shape_for_loose_tight_section() {
        // Use the exact source from the session deltas used in tests.
        let source = r#"
Loose vs. tight list items:
1. Tight item
2. Another tight item

3.
   Loose item
"#;

        let rendered = tui_markdown::from_str(source);
        let lines: Vec<String> = rendered
            .lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<String>()
            })
            .collect();
        // Join into a single string and assert the exact shape we observe
        // from tui_markdown in this larger context (marker and content split).
        let joined = {
            let mut s = String::new();
            for (i, l) in lines.iter().enumerate() {
                s.push_str(l);
                if i + 1 < lines.len() {
                    s.push('\n');
                }
            }
            s
        };
        let expected = r#"Loose vs. tight list items:

1. 
Tight item
2. 
Another tight item
3. 
Loose item"#;
        assert_eq!(
            joined, expected,
            "unexpected tui_markdown shape: {joined:?}"
        );
    }

    #[test]
    fn split_text_and_fences_keeps_ordered_list_line_as_text() {
        // No fences here; expect a single Text segment containing the full input.
        let src = "Loose vs. tight list items:\n1. Tight item\n";
        let segs = super::split_text_and_fences(src);
        assert_eq!(
            segs.len(),
            1,
            "expected single text segment, got {}",
            segs.len()
        );
        match &segs[0] {
            super::Segment::Text(s) => assert_eq!(s, src),
            _ => panic!("expected Text segment for non-fence input"),
        }
    }

    #[test]
    fn append_markdown_keeps_ordered_list_line_unsplit_in_context() {
        use codex_core::config_types::UriBasedFileOpener;
        use std::path::Path;
        let src = "Loose vs. tight list items:\n1. Tight item\n";
        let cwd = Path::new("/");
        let mut out = Vec::new();
        append_markdown_with_opener_and_cwd(src, &mut out, UriBasedFileOpener::None, cwd);

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
