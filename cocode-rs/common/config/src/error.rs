//! Error types for configuration management.

use cocode_error::ErrorExt;
use cocode_error::Location;
use cocode_error::StatusCode;
use cocode_error::stack_trace_debug;
use snafu::Snafu;
use std::any::Any;

/// The kind of resource that was not found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotFoundKind {
    Provider,
    Model,
    Profile,
}

impl std::fmt::Display for NotFoundKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Provider => write!(f, "Provider"),
            Self::Model => write!(f, "Model"),
            Self::Profile => write!(f, "Profile"),
        }
    }
}

/// Configuration error type.
///
/// Use snafu context selectors from `config_error` module within the crate:
/// ```ignore
/// use crate::error::config_error::*;
/// use snafu::ResultExt;
///
/// // Wrapping std::io::Error
/// fs::read(path).context(IoSnafu { message: "read file" })?;
///
/// // Wrapping serde_json::Error
/// serde_json::from_str(s).context(JsonParseSnafu { file: "config.json" })?;
///
/// // For errors without a source, use ensure! or .fail()
/// snafu::ensure!(condition, NotFoundSnafu { kind: NotFoundKind::Provider, name: "test" });
/// return NotFoundSnafu { kind: NotFoundKind::Model, name }.fail();
/// ```
#[stack_trace_debug]
#[derive(Snafu)]
#[snafu(visibility(pub(crate)), module)]
pub enum ConfigError {
    /// I/O or system error (wraps std::io::Error).
    #[snafu(display("IO error: {message}"))]
    Io {
        message: String,
        #[snafu(source)]
        source: std::io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    /// Configuration parsing error (wraps serde_json::Error).
    #[snafu(display("Config error in {file}: {source}"))]
    JsonParse {
        file: String,
        #[snafu(source)]
        source: serde_json::Error,
        #[snafu(implicit)]
        location: Location,
    },

    /// JSONC parsing error (comments, trailing commas, etc.).
    #[snafu(display("JSONC parse error in {file}: {message}"))]
    JsoncParse {
        file: String,
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Configuration validation error (no underlying source).
    #[snafu(display("Config error in {file}: {message}"))]
    ConfigValidation {
        file: String,
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Internal error (lock poisoning, unexpected state, etc).
    #[snafu(display("Internal error: {message}"))]
    Internal {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Resource not found.
    #[snafu(display("{kind} not found: {name}"))]
    NotFound {
        kind: NotFoundKind,
        name: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Authentication failed.
    #[snafu(display("Authentication failed: {message}"))]
    Auth {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },
}

impl ErrorExt for ConfigError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Io { .. } => StatusCode::IoError,
            Self::JsonParse { .. } | Self::JsoncParse { .. } | Self::ConfigValidation { .. } => {
                StatusCode::InvalidConfig
            }
            Self::Internal { .. } => StatusCode::Internal,
            Self::NotFound { kind, .. } => match kind {
                NotFoundKind::Provider => StatusCode::ProviderNotFound,
                NotFoundKind::Model => StatusCode::ModelNotFound,
                NotFoundKind::Profile => StatusCode::InvalidConfig,
            },
            Self::Auth { .. } => StatusCode::AuthenticationFailed,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Result type alias for configuration operations.
pub type Result<T> = std::result::Result<T, ConfigError>;
