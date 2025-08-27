use thiserror::Error;

#[derive(Debug, Error)]
pub enum LoginError {
    #[error("native browser login aborted")]
    Aborted,

    #[error("native browser login is only supported on macOS at this time")]
    UnsupportedOs,

    #[error("native browser helper compile failed: {0}")]
    HelperCompileFailed(String),

    #[error("native browser helper returned invalid response")]
    InvalidHelperResponse,

    #[error("oauth state mismatch")]
    StateMismatch,

    #[error("token exchange failed: {0}")]
    TokenExchangeFailed(String),

    #[error(transparent)]
    Network(#[from] reqwest::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
