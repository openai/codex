use starlark::Error as StarlarkError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextPosition {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRange {
    pub start: TextPosition,
    pub end: TextPosition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorLocation {
    pub path: String,
    pub range: TextRange,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid decision: {0}")]
    InvalidDecision(String),
    #[error("invalid pattern element: {0}")]
    InvalidPattern(String),
    #[error("invalid example: {0}")]
    InvalidExample(String),
    #[error("invalid rule: {0}")]
    InvalidRule(String),
    #[error(
        "expected every example to match at least one rule. rules: {rules:?}; unmatched examples: \
         {examples:?}"
    )]
    ExampleDidNotMatch {
        rules: Vec<String>,
        examples: Vec<String>,
    },
    #[error("expected example to not match rule `{rule}`: {example}")]
    ExampleDidMatch { rule: String, example: String },
    #[error("starlark error: {0}")]
    Starlark(StarlarkError),
}

impl Error {
    pub fn location(&self) -> Option<ErrorLocation> {
        match self {
            Error::Starlark(err) => err.span().map(|span| {
                let resolved = span.resolve_span();
                ErrorLocation {
                    path: span.filename().to_string(),
                    range: TextRange {
                        start: TextPosition {
                            line: resolved.begin.line + 1,
                            column: resolved.begin.column + 1,
                        },
                        end: TextPosition {
                            line: resolved.end.line + 1,
                            column: resolved.end.column + 1,
                        },
                    },
                }
            }),
            _ => None,
        }
    }
}
