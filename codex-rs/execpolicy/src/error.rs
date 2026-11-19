//! Diagnostics for loading and validating a Starlark policy file.

use crate::ErrorLocation;
use crate::RuleError;
use codex_execpolicy_rules::TextPosition;
use codex_execpolicy_rules::TextRange;
use starlark::codemap::FileSpan;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

/// A failure to evaluate a policy file or validate its rules.
#[derive(Debug, Error)]
pub enum Error {
    #[error("starlark error: {0}")]
    Starlark(starlark::Error),
    #[error(transparent)]
    Rule(#[from] RuleError),
}

impl Error {
    /// Return the one-based source range when the failure has a span.
    pub fn location(&self) -> Option<ErrorLocation> {
        match self {
            Error::Starlark(error) => error.span().map(error_location_from_file_span),
            Error::Rule(error) => error.location(),
        }
    }
}

/// Convert a parser span to the one-based coordinates used for policy diagnostics.
pub(crate) fn error_location_from_file_span(span: &FileSpan) -> ErrorLocation {
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
}
