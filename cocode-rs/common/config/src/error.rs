//! Error types for configuration management.

use cocode_error::{ErrorExt, Location, StatusCode, stack_trace_debug};
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
#[stack_trace_debug]
#[derive(Snafu)]
#[snafu(visibility(pub(crate)), module)]
pub enum ConfigError {
    /// I/O or system error.
    #[snafu(display("IO error: {message}"))]
    Io {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Configuration parsing or validation error.
    #[snafu(display("Config error in {file}: {message}"))]
    Config {
        file: String,
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

/// Create a Location from the caller's position.
#[track_caller]
fn caller_location() -> Location {
    let loc = std::panic::Location::caller();
    Location::new(loc.file(), loc.line(), loc.column())
}

/// Clean public constructors (library-agnostic API).
impl ConfigError {
    /// Create an IO error.
    #[track_caller]
    pub fn io(message: impl Into<String>) -> Self {
        Self::Io {
            message: message.into(),
            location: caller_location(),
        }
    }

    /// Create a config parsing error.
    #[track_caller]
    pub fn config(file: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Config {
            file: file.into(),
            message: message.into(),
            location: caller_location(),
        }
    }

    /// Create a provider not found error.
    #[track_caller]
    pub fn provider_not_found(name: impl Into<String>) -> Self {
        Self::NotFound {
            kind: NotFoundKind::Provider,
            name: name.into(),
            location: caller_location(),
        }
    }

    /// Create a model not found error.
    #[track_caller]
    pub fn model_not_found(name: impl Into<String>) -> Self {
        Self::NotFound {
            kind: NotFoundKind::Model,
            name: name.into(),
            location: caller_location(),
        }
    }

    /// Create a profile not found error.
    #[track_caller]
    pub fn profile_not_found(name: impl Into<String>) -> Self {
        Self::NotFound {
            kind: NotFoundKind::Profile,
            name: name.into(),
            location: caller_location(),
        }
    }

    /// Create an authentication error.
    #[track_caller]
    pub fn auth(message: impl Into<String>) -> Self {
        Self::Auth {
            message: message.into(),
            location: caller_location(),
        }
    }
}

impl ErrorExt for ConfigError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Io { .. } => StatusCode::IoError,
            Self::Config { .. } => StatusCode::InvalidConfig,
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
