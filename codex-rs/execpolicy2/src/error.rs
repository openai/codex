use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid decision: {0}")]
    InvalidDecision(String),
    #[error("invalid pattern element: {0}")]
    InvalidPattern(String),
    #[error("invalid example: {0}")]
    InvalidExample(String),
    #[error("expected example to match rule `{rule_id}`: {example}")]
    ExampleDidNotMatch { rule_id: String, example: String },
    #[error("expected example to not match rule `{rule_id}`: {example}")]
    ExampleDidMatch { rule_id: String, example: String },
    #[error("starlark error: {0}")]
    Starlark(String),
}
