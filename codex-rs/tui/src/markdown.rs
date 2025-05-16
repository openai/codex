use codex_core::config::Config;
use codex_core::config::UriBasedFileOpener;
use ratatui::text::Line;
use ratatui::text::Span;
use std::borrow::Cow;
use std::path::Path;

use crate::citation_regex::CITATION_REGEX;

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
    // Perform citation rewrite *before* feeding the string to the markdown
    // renderer. When `file_opener` is absent we bypass the transformation to
    // avoid unnecessary allocations.
    let processed_markdown = rewrite_file_citations(markdown_source, file_opener, cwd);

    let markdown = tui_markdown::from_str(&processed_markdown);

    // `tui_markdown` returns a `ratatui::text::Text` where every `Line` borrows
    // from the input `message` string. Since the `HistoryCell` stores its lines
    // with a `'static` lifetime we must create an **owned** copy of each line
    // so that it is no longer tied to `message`. We do this by cloning the
    // content of every `Span` into an owned `String`.

    for borrowed_line in markdown.lines {
        let mut owned_spans = Vec::with_capacity(borrowed_line.spans.len());
        for span in &borrowed_line.spans {
            // Create a new owned String for the span's content to break the lifetime link.
            let owned_span = Span::styled(span.content.to_string(), span.style);
            owned_spans.push(owned_span);
        }

        let owned_line: Line<'static> = Line::from(owned_spans).style(borrowed_line.style);
        // Preserve alignment if it was set on the source line.
        let owned_line = match borrowed_line.alignment {
            Some(alignment) => owned_line.alignment(alignment),
            None => owned_line,
        };

        lines.push(owned_line);
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

    CITATION_REGEX.replace_all(src, |caps: &regex::Captures<'_>| {
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
}
