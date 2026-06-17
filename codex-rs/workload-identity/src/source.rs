use std::path::PathBuf;

use thiserror::Error;
use tokio::io::AsyncReadExt;

pub const JWT_SUBJECT_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:jwt";
pub const MAX_SUBJECT_TOKEN_BYTES: usize = 1024 * 1024;

/// Acquires one external assertion from an explicit workload runtime source.
/// Implementations must not fall back to another source or expose credentials in errors.
pub trait SubjectTokenProvider: Send + Sync {
    fn subject_token(
        &self,
    ) -> impl std::future::Future<Output = Result<SubjectToken, SubjectTokenError>> + Send;
}

#[derive(PartialEq, Eq)]
pub struct SubjectToken {
    value: String,
    token_type: &'static str,
}

impl SubjectToken {
    pub fn jwt(value: impl Into<String>, source: &'static str) -> Result<Self, SubjectTokenError> {
        Self::new(value, JWT_SUBJECT_TOKEN_TYPE, source)
    }

    pub fn new(
        value: impl Into<String>,
        token_type: &'static str,
        source: &'static str,
    ) -> Result<Self, SubjectTokenError> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() {
            return Err(SubjectTokenError::Empty { provider: source });
        }
        if value.len() > MAX_SUBJECT_TOKEN_BYTES {
            return Err(SubjectTokenError::TooLarge { provider: source });
        }
        Ok(Self {
            value: value.to_string(),
            token_type,
        })
    }

    pub(crate) fn value(&self) -> &str {
        &self.value
    }

    pub(crate) const fn token_type(&self) -> &'static str {
        self.token_type
    }
}

impl std::fmt::Debug for SubjectToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SubjectToken")
            .field("value", &"[REDACTED]")
            .field("token_type", &self.token_type)
            .finish()
    }
}

#[derive(Debug, Error)]
pub enum SubjectTokenError {
    #[error("{provider} credential source requires {prerequisite}")]
    MissingPrerequisite {
        provider: &'static str,
        prerequisite: String,
    },
    #[error("{provider} credential source contains an empty subject token")]
    Empty { provider: &'static str },
    #[error("{provider} credential source returned an oversized subject token")]
    TooLarge { provider: &'static str },
    #[error("{provider} credential source could not read {path}: {kind}")]
    ReadFile {
        provider: &'static str,
        path: PathBuf,
        kind: std::io::ErrorKind,
    },
    #[error("{provider} credential source path is not a regular file: {path}")]
    NotAFile {
        provider: &'static str,
        path: PathBuf,
    },
    #[error("{provider} credential source is unavailable")]
    Unavailable { provider: &'static str },
    #[error("{provider} credential source returned an invalid response")]
    InvalidResponse { provider: &'static str },
    #[error("{provider} credential source configuration is invalid")]
    InvalidConfiguration { provider: &'static str },
}

enum CapturedEnvironmentValue {
    Value(String),
    Missing,
    NotUnicode,
}

pub struct EnvironmentSubjectTokenSource {
    variable: String,
    captured: CapturedEnvironmentValue,
}

impl EnvironmentSubjectTokenSource {
    pub fn capture(variable: impl Into<String>) -> Self {
        let variable = variable.into();
        let captured = match std::env::var(&variable) {
            Ok(value) => CapturedEnvironmentValue::Value(value),
            Err(std::env::VarError::NotPresent) => CapturedEnvironmentValue::Missing,
            Err(std::env::VarError::NotUnicode(_)) => CapturedEnvironmentValue::NotUnicode,
        };
        Self { variable, captured }
    }
}

impl SubjectTokenProvider for EnvironmentSubjectTokenSource {
    async fn subject_token(&self) -> Result<SubjectToken, SubjectTokenError> {
        match &self.captured {
            CapturedEnvironmentValue::Value(value) => {
                SubjectToken::jwt(value.clone(), "environment")
            }
            CapturedEnvironmentValue::Missing | CapturedEnvironmentValue::NotUnicode => {
                Err(SubjectTokenError::MissingPrerequisite {
                    provider: "environment",
                    prerequisite: self.variable.clone(),
                })
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct FileSubjectTokenSource {
    source: &'static str,
    path: PathBuf,
}

impl FileSubjectTokenSource {
    pub fn new(path: PathBuf) -> Self {
        Self::for_source("file", path)
    }

    pub fn for_source(source: &'static str, path: PathBuf) -> Self {
        Self { source, path }
    }
}

impl SubjectTokenProvider for FileSubjectTokenSource {
    async fn subject_token(&self) -> Result<SubjectToken, SubjectTokenError> {
        let metadata =
            tokio::fs::metadata(&self.path)
                .await
                .map_err(|error| SubjectTokenError::ReadFile {
                    provider: self.source,
                    path: self.path.clone(),
                    kind: error.kind(),
                })?;
        validate_file_metadata(self.source, &self.path, &metadata)?;

        let mut options = tokio::fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        options.custom_flags(libc::O_NONBLOCK);
        let file = options
            .open(&self.path)
            .await
            .map_err(|error| SubjectTokenError::ReadFile {
                provider: self.source,
                path: self.path.clone(),
                kind: error.kind(),
            })?;
        let metadata = file
            .metadata()
            .await
            .map_err(|error| SubjectTokenError::ReadFile {
                provider: self.source,
                path: self.path.clone(),
                kind: error.kind(),
            })?;
        validate_file_metadata(self.source, &self.path, &metadata)?;
        let mut bytes = Vec::new();
        file.take(MAX_SUBJECT_TOKEN_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .await
            .map_err(|error| SubjectTokenError::ReadFile {
                provider: self.source,
                path: self.path.clone(),
                kind: error.kind(),
            })?;
        if bytes.len() > MAX_SUBJECT_TOKEN_BYTES {
            return Err(SubjectTokenError::TooLarge {
                provider: self.source,
            });
        }
        let value = String::from_utf8(bytes).map_err(|_| SubjectTokenError::ReadFile {
            provider: self.source,
            path: self.path.clone(),
            kind: std::io::ErrorKind::InvalidData,
        })?;
        SubjectToken::jwt(value, self.source)
    }
}

fn validate_file_metadata(
    source: &'static str,
    path: &std::path::Path,
    metadata: &std::fs::Metadata,
) -> Result<(), SubjectTokenError> {
    if !metadata.is_file() {
        return Err(SubjectTokenError::NotAFile {
            provider: source,
            path: path.to_path_buf(),
        });
    }
    if metadata.len() > MAX_SUBJECT_TOKEN_BYTES as u64 {
        return Err(SubjectTokenError::TooLarge { provider: source });
    }
    Ok(())
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
