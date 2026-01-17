//! Bash syntax highlighting helpers for rendering terminal output.
//!
//! This module uses tree-sitter's bash highlight queries to produce styled
//! `Line` values for display in the TUI. Highlight configuration is cached in
//! `OnceLock` statics to avoid reloading query data, and the output is streamed
//! to preserve style boundaries across multi-line input. The highlight names
//! follow the tree-sitter bash query definitions at
//! <https://github.com/tree-sitter/tree-sitter-bash/blob/master/queries/highlights.scm>.

use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use std::sync::OnceLock;
use tree_sitter_highlight::Highlight;
use tree_sitter_highlight::HighlightConfiguration;
use tree_sitter_highlight::HighlightEvent;
use tree_sitter_highlight::Highlighter;

/// Highlight categories referenced by the tree-sitter bash query.
///
/// The variant list must mirror the query's highlight names exactly, as the
/// mapping relies on positional indices.
#[derive(Copy, Clone)]
enum BashHighlight {
    Comment,
    Constant,
    Embedded,
    Function,
    Keyword,
    Number,
    Operator,
    Property,
    String,
}

impl BashHighlight {
    /// All highlight variants in query order.
    const ALL: [Self; 9] = [
        Self::Comment,
        Self::Constant,
        Self::Embedded,
        Self::Function,
        Self::Keyword,
        Self::Number,
        Self::Operator,
        Self::Property,
        Self::String,
    ];

    /// Returns the highlight name used by the tree-sitter query.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Comment => "comment",
            Self::Constant => "constant",
            Self::Embedded => "embedded",
            Self::Function => "function",
            Self::Keyword => "keyword",
            Self::Number => "number",
            Self::Operator => "operator",
            Self::Property => "property",
            Self::String => "string",
        }
    }

    /// Returns the style to apply for this highlight category.
    fn style(self) -> Style {
        match self {
            Self::Comment | Self::Operator | Self::String => Style::default().dim(),
            _ => Style::default(),
        }
    }
}

/// Cached tree-sitter highlight configuration for bash.
static HIGHLIGHT_CONFIG: OnceLock<HighlightConfiguration> = OnceLock::new();

/// Returns the list of highlight names expected by the configuration.
fn highlight_names() -> &'static [&'static str] {
    static NAMES: OnceLock<[&'static str; BashHighlight::ALL.len()]> = OnceLock::new();
    NAMES
        .get_or_init(|| BashHighlight::ALL.map(BashHighlight::as_str))
        .as_slice()
}

/// Returns a lazily initialized highlight configuration for bash.
fn highlight_config() -> &'static HighlightConfiguration {
    HIGHLIGHT_CONFIG.get_or_init(|| {
        let language = tree_sitter_bash::LANGUAGE.into();
        #[expect(clippy::expect_used)]
        let mut config = HighlightConfiguration::new(
            language,
            "bash",
            tree_sitter_bash::HIGHLIGHT_QUERY,
            "",
            "",
        )
        .expect("load bash highlight query");
        config.configure(highlight_names());
        config
    })
}

/// Maps a tree-sitter highlight index to the local enum variant.
fn highlight_for(highlight: Highlight) -> BashHighlight {
    BashHighlight::ALL[highlight.0]
}

/// Pushes a possibly multi-line segment into the output lines.
///
/// The segment is split on newlines, creating new `Line` entries as needed, and
/// each part is appended as a span with the supplied style.
fn push_segment(lines: &mut Vec<Line<'static>>, segment: &str, style: Option<Style>) {
    for (i, part) in segment.split('\n').enumerate() {
        if i > 0 {
            lines.push(Line::from(""));
        }
        if part.is_empty() {
            continue;
        }
        let span = match style {
            Some(style) => Span::styled(part.to_string(), style),
            None => part.to_string().into(),
        };
        if let Some(last) = lines.last_mut() {
            last.spans.push(span);
        }
    }
}

/// Converts a bash script into per-line styled content using tree-sitter.
///
/// The highlighter is streamed so multi-line content is split into `Line`s
/// while preserving style boundaries. If highlighting fails for any reason,
/// this returns a single unstyled line containing the original script.
pub(crate) fn highlight_bash_to_lines(script: &str) -> Vec<Line<'static>> {
    let mut highlighter = Highlighter::new();
    let iterator =
        match highlighter.highlight(highlight_config(), script.as_bytes(), None, |_| None) {
            Ok(iter) => iter,
            Err(_) => return vec![script.to_string().into()],
        };

    let mut lines: Vec<Line<'static>> = vec![Line::from("")];
    let mut highlight_stack: Vec<Highlight> = Vec::new();

    for event in iterator {
        match event {
            Ok(HighlightEvent::HighlightStart(highlight)) => highlight_stack.push(highlight),
            Ok(HighlightEvent::HighlightEnd) => {
                highlight_stack.pop();
            }
            Ok(HighlightEvent::Source { start, end }) => {
                if start == end {
                    continue;
                }
                let style = highlight_stack.last().map(|h| highlight_for(*h).style());
                push_segment(&mut lines, &script[start..end], style);
            }
            Err(_) => return vec![script.to_string().into()],
        }
    }

    if lines.is_empty() {
        vec![Line::from("")]
    } else {
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use ratatui::style::Modifier;

    /// Reassembles the original script text from rendered spans.
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

    /// Extracts trimmed token strings that were rendered using dimmed styling.
    fn dimmed_tokens(lines: &[Line<'static>]) -> Vec<String> {
        lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .filter(|sp| sp.style.add_modifier.contains(Modifier::DIM))
            .map(|sp| sp.content.clone().into_owned())
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty())
            .collect()
    }

    /// Ensures common bash operators are dimmed without altering output text.
    #[test]
    fn dims_expected_bash_operators() {
        let s = "echo foo && bar || baz | qux & (echo hi)";
        let lines = highlight_bash_to_lines(s);
        assert_eq!(reconstructed(&lines), s);

        let dimmed = dimmed_tokens(&lines);
        assert!(dimmed.contains(&"&&".to_string()));
        assert!(dimmed.contains(&"|".to_string()));
        assert!(!dimmed.contains(&"echo".to_string()));
    }

    /// Confirms redirection and quoted strings receive the dimmed style.
    #[test]
    fn dims_redirects_and_strings() {
        let s = "echo \"hi\" > out.txt; echo 'ok'";
        let lines = highlight_bash_to_lines(s);
        assert_eq!(reconstructed(&lines), s);

        let dimmed = dimmed_tokens(&lines);
        assert!(dimmed.contains(&">".to_string()));
        assert!(dimmed.contains(&"\"hi\"".to_string()));
        assert!(dimmed.contains(&"'ok'".to_string()));
    }

    /// Verifies commands keep default styling while strings are dimmed.
    #[test]
    fn highlights_command_and_strings() {
        let s = "echo \"hi\"";
        let lines = highlight_bash_to_lines(s);
        let mut echo_style = None;
        let mut string_style = None;
        for span in &lines[0].spans {
            let text = span.content.as_ref();
            if text == "echo" {
                echo_style = Some(span.style);
            }
            if text == "\"hi\"" {
                string_style = Some(span.style);
            }
        }
        let echo_style = echo_style.expect("echo span missing");
        let string_style = string_style.expect("string span missing");
        assert!(echo_style.fg.is_none());
        assert!(!echo_style.add_modifier.contains(Modifier::DIM));
        assert!(string_style.add_modifier.contains(Modifier::DIM));
    }

    /// Treats heredoc bodies as string content so they are dimmed.
    #[test]
    fn highlights_heredoc_body_as_string() {
        let s = "cat <<EOF\nheredoc body\nEOF";
        let lines = highlight_bash_to_lines(s);
        let body_line = &lines[1];
        let mut body_style = None;
        for span in &body_line.spans {
            if span.content.as_ref() == "heredoc body" {
                body_style = Some(span.style);
            }
        }
        let body_style = body_style.expect("missing heredoc span");
        assert!(body_style.add_modifier.contains(Modifier::DIM));
    }
}
