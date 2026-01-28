use std::sync::OnceLock;
use tree_sitter_highlight::Highlight;
use tree_sitter_highlight::HighlightConfiguration;
use tree_sitter_highlight::HighlightEvent;
use tree_sitter_highlight::Highlighter;

use crate::render::model::RenderCell;
use crate::render::model::RenderLine;
use crate::render::model::RenderStyle;

// Ref: https://github.com/tree-sitter/tree-sitter-bash/blob/master/queries/highlights.scm
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

    fn style(self) -> RenderStyle {
        match self {
            Self::Comment | Self::Operator | Self::String => RenderStyle::builder().dim().build(),
            _ => RenderStyle::default(),
        }
    }
}

static HIGHLIGHT_CONFIG: OnceLock<HighlightConfiguration> = OnceLock::new();

fn highlight_names() -> &'static [&'static str] {
    static NAMES: OnceLock<[&'static str; BashHighlight::ALL.len()]> = OnceLock::new();
    NAMES
        .get_or_init(|| BashHighlight::ALL.map(BashHighlight::as_str))
        .as_slice()
}

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

fn highlight_for(highlight: Highlight) -> BashHighlight {
    BashHighlight::ALL[highlight.0]
}

fn push_segment(lines: &mut Vec<RenderLine>, segment: &str, style: Option<RenderStyle>) {
    for (i, part) in segment.split('\n').enumerate() {
        if i > 0 {
            lines.push(RenderLine::from(""));
        }
        if part.is_empty() {
            continue;
        }
        let cell = match style {
            Some(style) => RenderCell::new(part.to_string(), style),
            None => RenderCell::plain(part),
        };
        if let Some(last) = lines.last_mut() {
            last.spans.push(cell);
        }
    }
}

/// Convert a bash script into per-line styled content using tree-sitter's
/// bash highlight query. The highlighter is streamed so multi-line content is
/// split into `RenderLine`s while preserving style boundaries.
pub(crate) fn highlight_bash_to_lines(script: &str) -> Vec<RenderLine> {
    let mut highlighter = Highlighter::new();
    let iterator =
        match highlighter.highlight(highlight_config(), script.as_bytes(), None, |_| None) {
            Ok(iter) => iter,
            Err(_) => return vec![script.to_string().into()],
        };

    let mut lines: Vec<RenderLine> = vec![RenderLine::from("")];
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
        vec![RenderLine::from("")]
    } else {
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn reconstructed(lines: &[RenderLine]) -> String {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|cell| cell.content.clone())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn dimmed_tokens(lines: &[RenderLine]) -> Vec<String> {
        lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .filter(|cell| cell.style.dim)
            .map(|cell| cell.content.trim().to_string())
            .filter(|token| !token.is_empty())
            .collect()
    }

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

    #[test]
    fn highlight_is_deterministic() {
        let s = "echo hello";
        let lines = highlight_bash_to_lines(s);
        assert_eq!(reconstructed(&lines), s);
    }
}
