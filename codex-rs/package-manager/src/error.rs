use std::path::PathBuf;
use thiserror::Error;

/// Errors returned by the generic package manager.
#[derive(Debug, Error)]
pub enum PackageManagerError {
    /// The current machine OS/architecture pair is not supported by the package.
    #[error("unsupported platform: {os}-{arch}")]
    UnsupportedPlatform { os: String, arch: String },

    /// The configured release base URL could not be joined with a package-specific path.
    #[error("invalid release base url")]
    InvalidBaseUrl(#[source] url::ParseError),

    /// An HTTP request failed while fetching the manifest or archive.
    #[error("{context}")]
    Http {
        context: String,
        #[source]
        source: reqwest::Error,
    },

    /// A filesystem operation failed while reading, staging, or promoting a package.
    #[error("{context}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },

    /// The release manifest did not contain an archive for the current platform.
    #[error("missing platform entry `{0}` in release manifest")]
    MissingPlatform(String),

    /// The release manifest or installed package reported a different version than requested.
    #[error("unexpected package version: expected `{expected}`, got `{actual}`")]
    UnexpectedPackageVersion { expected: String, actual: String },

    /// The downloaded archive length did not match the manifest metadata.
    #[error("unexpected archive size: expected `{expected}`, got `{actual}`")]
    UnexpectedArchiveSize { expected: u64, actual: u64 },

    /// The downloaded archive checksum did not match the manifest metadata.
    #[error("checksum mismatch: expected `{expected}`, got `{actual}`")]
    ChecksumMismatch { expected: String, actual: String },

    /// Archive extraction failed or the archive contents violated extraction rules.
    #[error("archive extraction failed: {0}")]
    ArchiveExtraction(String),

    /// The extracted archive layout did not contain a detectable package root.
    #[error("archive did not contain a package root with manifest.json under {0}")]
    MissingPackageRoot(PathBuf),
}
