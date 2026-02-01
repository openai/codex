//! Error types for the plugin system.

use std::path::PathBuf;

use cocode_error::ErrorExt;
use cocode_error::Location;
use cocode_error::StatusCode;
use cocode_error::stack_trace_debug;
use snafu::Snafu;

/// Plugin errors.
#[stack_trace_debug]
#[derive(Snafu)]
#[snafu(visibility(pub(crate)), module)]
pub enum PluginError {
    /// Plugin manifest not found.
    #[snafu(display("Plugin manifest not found: {}", path.display()))]
    ManifestNotFound {
        path: PathBuf,
        #[snafu(implicit)]
        location: Location,
    },

    /// Invalid plugin manifest.
    #[snafu(display("Invalid plugin manifest at {}: {message}", path.display()))]
    InvalidManifest {
        path: PathBuf,
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Plugin already registered.
    #[snafu(display("Plugin already registered: {name}"))]
    AlreadyRegistered {
        name: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Plugin not found.
    #[snafu(display("Plugin not found: {name}"))]
    NotFound {
        name: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// IO error during plugin loading.
    #[snafu(display("IO error at {}: {message}", path.display()))]
    Io {
        path: PathBuf,
        message: String,
        #[snafu(implicit)]
        location: Location,
    },

    /// Path traversal attempted.
    #[snafu(display("Path traversal not allowed: {}", path.display()))]
    PathTraversal {
        path: PathBuf,
        #[snafu(implicit)]
        location: Location,
    },

    /// Invalid version format.
    #[snafu(display("Invalid version format: {version}"))]
    InvalidVersion {
        version: String,
        #[snafu(implicit)]
        location: Location,
    },
}

impl ErrorExt for PluginError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::ManifestNotFound { .. } => StatusCode::FileNotFound,
            Self::InvalidManifest { .. } => StatusCode::InvalidConfig,
            Self::AlreadyRegistered { .. } => StatusCode::InvalidArguments,
            Self::NotFound { .. } => StatusCode::FileNotFound,
            Self::Io { .. } => StatusCode::IoError,
            Self::PathTraversal { .. } => StatusCode::PermissionDenied,
            Self::InvalidVersion { .. } => StatusCode::InvalidConfig,
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Result type for plugin operations.
pub type Result<T> = std::result::Result<T, PluginError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = plugin_error::ManifestNotFoundSnafu {
            path: PathBuf::from("/path/to/plugin"),
        }
        .build();
        assert!(err.to_string().contains("/path/to/plugin"));

        let err = plugin_error::InvalidManifestSnafu {
            path: PathBuf::from("/plugin"),
            message: "missing name",
        }
        .build();
        assert!(err.to_string().contains("missing name"));
    }

    #[test]
    fn test_error_status_codes() {
        let err = plugin_error::ManifestNotFoundSnafu {
            path: PathBuf::from("/test"),
        }
        .build();
        assert_eq!(err.status_code(), StatusCode::FileNotFound);

        let err = plugin_error::PathTraversalSnafu {
            path: PathBuf::from("../../../etc/passwd"),
        }
        .build();
        assert_eq!(err.status_code(), StatusCode::PermissionDenied);
    }
}
