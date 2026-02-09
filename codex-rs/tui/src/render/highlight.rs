use ratatui::style::Color as RtColor;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::FontStyle;
use syntect::highlighting::Style as SyntectStyle;
use syntect::highlighting::Theme;
use syntect::parsing::SyntaxReference;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use two_face::theme::EmbeddedThemeName;

// -- Global singletons -------------------------------------------------------

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME: OnceLock<Theme> = OnceLock::new();

fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(two_face::syntax::extra_newlines)
}

fn theme() -> &'static Theme {
    THEME.get_or_init(|| {
        let ts = two_face::theme::extra();
        // Pick light or dark theme based on terminal background color.
        let name = match crate::terminal_palette::default_bg() {
            Some(bg) if crate::color::is_light(bg) => EmbeddedThemeName::CatppuccinLatte,
            _ => EmbeddedThemeName::CatppuccinMocha,
        };
        ts.get(name).clone()
    })
}

// -- Style conversion (syntect -> ratatui) ------------------------------------

/// Convert a syntect `Style` to a ratatui `Style`.
///
/// Syntax highlighting themes inherently produce RGB colors, so we allow
/// `Color::Rgb` here despite the project-wide preference for ANSI colors.
#[allow(clippy::disallowed_methods)]
fn convert_style(syn_style: SyntectStyle) -> Style {
    let mut rt_style = Style::default();

    // Map foreground color when visible.
    let fg = syn_style.foreground;
    if fg.a > 0 {
        rt_style = rt_style.fg(RtColor::Rgb(fg.r, fg.g, fg.b));
    }
    // Intentionally skip background to avoid overwriting terminal bg.

    if syn_style.font_style.contains(FontStyle::BOLD) {
        rt_style.add_modifier |= Modifier::BOLD;
    }
    // Intentionally skip italic — many terminals render it poorly or not at all.
    if syn_style.font_style.contains(FontStyle::UNDERLINE) {
        rt_style.add_modifier |= Modifier::UNDERLINED;
    }

    rt_style
}

// -- Syntax lookup ------------------------------------------------------------

/// Try to find a syntect `SyntaxReference` for the given language identifier.
///
/// two-face's extended syntax set (~250 languages) resolves most names and
/// extensions directly.  We only patch the few aliases it cannot handle.
fn find_syntax(lang: &str) -> Option<&'static SyntaxReference> {
    let ss = syntax_set();

    // Aliases that two-face does not resolve on its own.
    let patched = match lang {
        "golang" => "go",
        "python3" => "python",
        "shell" => "bash",
        _ => lang,
    };

    // Try by token (matches file_extensions case-insensitively).
    if let Some(s) = ss.find_syntax_by_token(patched) {
        return Some(s);
    }
    // Try by exact syntax name (e.g. "Rust", "Python").
    if let Some(s) = ss.find_syntax_by_name(patched) {
        return Some(s);
    }
    // Try case-insensitive name match (e.g. "rust" -> "Rust").
    let lower = patched.to_ascii_lowercase();
    if let Some(s) = ss
        .syntaxes()
        .iter()
        .find(|s| s.name.to_ascii_lowercase() == lower)
    {
        return Some(s);
    }
    // Try raw input as file extension.
    if let Some(s) = ss.find_syntax_by_extension(lang) {
        return Some(s);
    }
    None
}

// -- Guardrail constants ------------------------------------------------------

/// Skip highlighting for inputs larger than 512 KB to avoid excessive memory
/// and CPU usage.  Callers fall back to plain unstyled text.
const MAX_HIGHLIGHT_BYTES: usize = 512 * 1024;

/// Skip highlighting for inputs with more than 10,000 lines.
const MAX_HIGHLIGHT_LINES: usize = 10_000;

// -- Core highlighting --------------------------------------------------------

/// Parse `code` using syntect for `lang` and return per-line styled spans.
/// Each inner Vec represents one source line.  Returns None when the language
/// is not recognized or the input exceeds safety limits.
fn highlight_to_line_spans(code: &str, lang: &str) -> Option<Vec<Vec<Span<'static>>>> {
    // Empty input has nothing to highlight; fall back to the plain text path
    // which correctly produces a single empty Line.
    if code.is_empty() {
        return None;
    }

    // Bail out early for oversized inputs to avoid excessive resource usage.
    if code.len() > MAX_HIGHLIGHT_BYTES
        || code.as_bytes().iter().filter(|&&b| b == b'\n').count() > MAX_HIGHLIGHT_LINES
    {
        return None;
    }

    let syntax = find_syntax(lang)?;
    let mut h = HighlightLines::new(syntax, theme());
    let mut lines: Vec<Vec<Span<'static>>> = Vec::new();

    for line in LinesWithEndings::from(code) {
        let ranges = h.highlight_line(line, syntax_set()).ok()?;
        let mut spans: Vec<Span<'static>> = Vec::new();
        for (style, text) in ranges {
            // Strip trailing newline since we handle line breaks ourselves.
            let text = text.trim_end_matches('\n');
            if text.is_empty() {
                continue;
            }
            spans.push(Span::styled(text.to_string(), convert_style(style)));
        }
        if spans.is_empty() {
            spans.push(Span::raw(String::new()));
        }
        lines.push(spans);
    }

    Some(lines)
}

// -- Public API ---------------------------------------------------------------

/// Highlight code in any supported language, returning styled ratatui Lines.
/// Falls back to plain unstyled text when the language is not recognized.
pub(crate) fn highlight_code_to_lines(code: &str, lang: &str) -> Vec<Line<'static>> {
    if let Some(line_spans) = highlight_to_line_spans(code, lang) {
        line_spans.into_iter().map(Line::from).collect()
    } else {
        // Fallback: plain text, one Line per source line.
        code.split('\n')
            .map(|l| Line::from(l.to_string()))
            .collect()
    }
}

/// Backward-compatible wrapper for bash highlighting used by exec cells.
pub(crate) fn highlight_bash_to_lines(script: &str) -> Vec<Line<'static>> {
    highlight_code_to_lines(script, "bash")
}

/// Highlight code and return per-line styled spans for diff integration.
/// Returns None if the language is unsupported.
pub(crate) fn highlight_code_to_styled_spans(
    code: &str,
    lang: &str,
) -> Option<Vec<Vec<Span<'static>>>> {
    highlight_to_line_spans(code, lang)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Reconstruct plain text from highlighted Lines.
    fn reconstructed(lines: &[Line<'static>]) -> String {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|sp| sp.content.clone())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn highlight_rust_has_keyword_style() {
        let code = "fn main() {}";
        let lines = highlight_code_to_lines(code, "rust");
        assert_eq!(reconstructed(&lines), code);

        // The `fn` keyword should have a non-default style (some color).
        let fn_span = lines[0].spans.iter().find(|sp| sp.content.as_ref() == "fn");
        assert!(fn_span.is_some(), "expected a span containing 'fn'");
        let style = fn_span.map(|s| s.style).unwrap_or_default();
        assert!(
            style.fg.is_some() || style.add_modifier != Modifier::empty(),
            "expected fn keyword to have non-default style, got {style:?}"
        );
    }

    #[test]
    fn highlight_unknown_lang_falls_back() {
        let code = "some random text";
        let lines = highlight_code_to_lines(code, "xyzlang");
        assert_eq!(reconstructed(&lines), code);
        // Should be plain text with no styling.
        for line in &lines {
            for span in &line.spans {
                assert_eq!(
                    span.style,
                    Style::default(),
                    "expected default style for unknown language"
                );
            }
        }
    }

    #[test]
    fn highlight_empty_string() {
        let lines = highlight_code_to_lines("", "rust");
        assert_eq!(lines.len(), 1);
        assert_eq!(reconstructed(&lines), "");
    }

    #[test]
    fn highlight_bash_preserves_content() {
        let script = "echo \"hello world\" && ls -la | grep foo";
        let lines = highlight_bash_to_lines(script);
        assert_eq!(reconstructed(&lines), script);
    }

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn style_conversion_correctness() {
        let syn = SyntectStyle {
            foreground: syntect::highlighting::Color {
                r: 255,
                g: 128,
                b: 0,
                a: 255,
            },
            background: syntect::highlighting::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            font_style: FontStyle::BOLD | FontStyle::ITALIC,
        };
        let rt = convert_style(syn);
        assert_eq!(rt.fg, Some(RtColor::Rgb(255, 128, 0)));
        // Background is intentionally skipped.
        assert_eq!(rt.bg, None);
        assert!(rt.add_modifier.contains(Modifier::BOLD));
        // Italic is intentionally suppressed.
        assert!(!rt.add_modifier.contains(Modifier::ITALIC));
        assert!(!rt.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn highlight_multiline_python() {
        let code = "def hello():\n    print(\"hi\")\n    return 42";
        let lines = highlight_code_to_lines(code, "python");
        assert_eq!(reconstructed(&lines), code);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn highlight_code_to_styled_spans_returns_none_for_unknown() {
        assert!(highlight_code_to_styled_spans("x", "xyzlang").is_none());
    }

    #[test]
    fn highlight_code_to_styled_spans_returns_some_for_known() {
        let result = highlight_code_to_styled_spans("let x = 1;", "rust");
        assert!(result.is_some());
        let spans = result.unwrap_or_default();
        assert!(!spans.is_empty());
    }

    #[test]
    fn highlight_markdown_preserves_content() {
        let code = "```sh\nprintf 'fenced within fenced\\n'\n```";
        let lines = highlight_code_to_lines(code, "markdown");
        let result = reconstructed(&lines);
        assert_eq!(
            result, code,
            "markdown highlighting must preserve content exactly"
        );
    }

    #[test]
    fn highlight_large_input_falls_back() {
        // Input exceeding MAX_HIGHLIGHT_BYTES should return None (plain text
        // fallback) rather than attempting to parse.
        let big = "x".repeat(MAX_HIGHLIGHT_BYTES + 1);
        let result = highlight_code_to_styled_spans(&big, "rust");
        assert!(result.is_none(), "oversized input should fall back to None");
    }

    #[test]
    fn highlight_many_lines_falls_back() {
        // Input exceeding MAX_HIGHLIGHT_LINES should return None.
        let many_lines = "let x = 1;\n".repeat(MAX_HIGHLIGHT_LINES + 1);
        let result = highlight_code_to_styled_spans(&many_lines, "rust");
        assert!(result.is_none(), "too many lines should fall back to None");
    }

    #[test]
    fn find_syntax_resolves_languages_and_aliases() {
        // Languages resolved directly by two-face's extended syntax set.
        let languages = [
            "javascript",
            "typescript",
            "tsx",
            "python",
            "ruby",
            "rust",
            "go",
            "c",
            "cpp",
            "yaml",
            "bash",
            "kotlin",
            "markdown",
            "sql",
            "lua",
            "zig",
            "swift",
            "java",
            "elixir",
            "haskell",
            "scala",
            "dart",
            "r",
            "perl",
            "php",
            "html",
            "css",
            "json",
            "toml",
            "xml",
            "dockerfile",
        ];
        for lang in languages {
            assert!(
                find_syntax(lang).is_some(),
                "find_syntax({lang:?}) returned None"
            );
        }
        // Common file extensions.
        let extensions = [
            "rs", "py", "js", "ts", "rb", "go", "sh", "md", "yml", "kt", "ex", "hs", "pl", "php",
            "css", "html",
        ];
        for ext in extensions {
            assert!(
                find_syntax(ext).is_some(),
                "find_syntax({ext:?}) returned None"
            );
        }
        // Patched aliases that two-face cannot resolve on its own.
        for alias in ["golang", "python3", "shell"] {
            assert!(
                find_syntax(alias).is_some(),
                "find_syntax({alias:?}) returned None — patched alias broken"
            );
        }
    }
}
