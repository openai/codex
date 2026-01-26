//! Error types for shell parsing.

use thiserror::Error;

/// Errors that can occur during shell parsing.
#[derive(Debug, Error)]
pub enum ParseError {
    /// Failed to load the Bash grammar.
    #[error("failed to load bash grammar: {0}")]
    LanguageError(#[from] tree_sitter::LanguageError),

    /// Shell input could not be parsed (syntax error).
    #[error("failed to parse shell input")]
    SyntaxError,

    /// UTF-8 decoding error.
    #[error("invalid UTF-8 in shell input: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),
}

/// Result type alias for shell parsing operations.
pub type Result<T> = std::result::Result<T, ParseError>;
