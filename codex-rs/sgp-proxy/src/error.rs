use thiserror::Error;

/// Top-level proxy error.
#[derive(Debug, Error)]
pub enum ProxyError {
    #[error("agentex error: {0}")]
    Agentex(#[from] AgentexError),

    #[error("request parse error: {0}")]
    RequestParse(String),

    #[error("session error: {0}")]
    Session(String),

    #[error("internal error: {0}")]
    Internal(String),
}

/// Errors originating from the Agentex JSON-RPC interaction.
#[derive(Debug, Error)]
pub enum AgentexError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON-RPC error {code}: {message}")]
    Rpc { code: i64, message: String },

    #[error("stream error: {0}")]
    Stream(String),

    #[error("parse error: {0}")]
    Parse(String),
}
