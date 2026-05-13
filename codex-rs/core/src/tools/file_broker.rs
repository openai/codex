#![allow(dead_code)]

use codex_tools::FileRef;
use codex_tools::FileScheme;
use std::fmt;
use std::fs;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

/// Minimal broker for moving bytes across Code Mode file refs.
///
/// This POC intentionally implements only the workspace environment provider.
/// Connector, Library, and remote-environment adapters can plug in behind this
/// boundary without changing model-facing tool contracts.
#[derive(Debug)]
pub(crate) struct CodeModeFileBroker {
    current_root: PathBuf,
}

impl CodeModeFileBroker {
    pub(crate) fn new(current_root: impl Into<PathBuf>) -> Self {
        Self {
            current_root: current_root.into(),
        }
    }

    pub(crate) fn read_to_bytes(&self, source: &FileRef) -> Result<Vec<u8>, FileBrokerError> {
        let source_path = self.resolve_env_path(source)?;
        fs::read(&source_path).map_err(|source| FileBrokerError::Io {
            action: "read",
            source,
        })
    }

    pub(crate) fn write_bytes(
        &self,
        target: &FileRef,
        bytes: &[u8],
    ) -> Result<FileBrokerWriteResult, FileBrokerError> {
        let target_path = self.resolve_env_path(target)?;
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|source| FileBrokerError::Io {
                action: "create target directory",
                source,
            })?;
        }
        fs::write(&target_path, bytes).map_err(|source| FileBrokerError::Io {
            action: "write",
            source,
        })?;
        Ok(FileBrokerWriteResult {
            file_ref: target.raw().to_string(),
            byte_count: bytes.len() as u64,
        })
    }

    pub(crate) fn copy(
        &self,
        source: &FileRef,
        target: &FileRef,
    ) -> Result<FileBrokerCopyResult, FileBrokerError> {
        let bytes = self.read_to_bytes(source)?;
        let write_result = self.write_bytes(target, &bytes)?;
        Ok(FileBrokerCopyResult {
            source_ref: source.raw().to_string(),
            target_ref: write_result.file_ref,
            byte_count: write_result.byte_count,
        })
    }

    fn resolve_env_path(&self, file_ref: &FileRef) -> Result<PathBuf, FileBrokerError> {
        if file_ref.scheme() != FileScheme::Env {
            return Err(FileBrokerError::UnsupportedProvider {
                file_ref: file_ref.raw().to_string(),
            });
        }

        let Some(path) = file_ref.body().strip_prefix("current/") else {
            return Err(FileBrokerError::UnsupportedEnvironment {
                file_ref: file_ref.raw().to_string(),
            });
        };
        if path.is_empty() {
            return Err(FileBrokerError::InvalidEnvPath {
                file_ref: file_ref.raw().to_string(),
            });
        }

        let relative_path =
            clean_relative_path(path).ok_or_else(|| FileBrokerError::InvalidEnvPath {
                file_ref: file_ref.raw().to_string(),
            })?;
        Ok(self.current_root.join(relative_path))
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FileBrokerWriteResult {
    pub(crate) file_ref: String,
    pub(crate) byte_count: u64,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FileBrokerCopyResult {
    pub(crate) source_ref: String,
    pub(crate) target_ref: String,
    pub(crate) byte_count: u64,
}

#[derive(Debug)]
pub(crate) enum FileBrokerError {
    UnsupportedProvider {
        file_ref: String,
    },
    UnsupportedEnvironment {
        file_ref: String,
    },
    InvalidEnvPath {
        file_ref: String,
    },
    Io {
        action: &'static str,
        source: std::io::Error,
    },
}

impl fmt::Display for FileBrokerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProvider { file_ref } => {
                write!(f, "file provider for `{file_ref}` is not available")
            }
            Self::UnsupportedEnvironment { file_ref } => {
                write!(f, "`{file_ref}` must use env://current/... in this runtime")
            }
            Self::InvalidEnvPath { file_ref } => {
                write!(f, "`{file_ref}` must resolve to a relative workspace path")
            }
            Self::Io { action, source } => write!(f, "failed to {action} file: {source}"),
        }
    }
}

impl std::error::Error for FileBrokerError {}

fn clean_relative_path(path: &str) -> Option<PathBuf> {
    let mut clean = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!clean.as_os_str().is_empty()).then_some(clean)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn file_ref(raw: &str) -> FileRef {
        FileRef::parse(raw).expect("file ref should parse")
    }

    #[test]
    fn writes_and_reads_env_current_refs() {
        let temp_dir = TempDir::new().expect("temp dir");
        let broker = CodeModeFileBroker::new(temp_dir.path());

        let target = file_ref("env://current/out/report.txt");
        let write_result = broker
            .write_bytes(&target, b"hello")
            .expect("write should succeed");

        assert_eq!(
            write_result,
            FileBrokerWriteResult {
                file_ref: "env://current/out/report.txt".to_string(),
                byte_count: 5,
            }
        );
        assert_eq!(
            broker.read_to_bytes(&target).expect("read should succeed"),
            b"hello"
        );
    }

    #[test]
    fn copies_between_env_current_refs() {
        let temp_dir = TempDir::new().expect("temp dir");
        let broker = CodeModeFileBroker::new(temp_dir.path());
        let source = file_ref("env://current/source.bin");
        let target = file_ref("env://current/nested/target.bin");
        broker
            .write_bytes(&source, b"payload")
            .expect("write should succeed");

        let copy_result = broker.copy(&source, &target).expect("copy should succeed");

        assert_eq!(
            copy_result,
            FileBrokerCopyResult {
                source_ref: "env://current/source.bin".to_string(),
                target_ref: "env://current/nested/target.bin".to_string(),
                byte_count: 7,
            }
        );
        assert_eq!(
            broker
                .read_to_bytes(&target)
                .expect("copied target should exist"),
            b"payload"
        );
    }

    #[test]
    fn rejects_env_path_traversal() {
        let temp_dir = TempDir::new().expect("temp dir");
        let broker = CodeModeFileBroker::new(temp_dir.path());
        let source = file_ref("env://current/../secret.txt");

        assert!(matches!(
            broker.read_to_bytes(&source),
            Err(FileBrokerError::InvalidEnvPath { .. })
        ));
    }

    #[test]
    fn rejects_provider_refs_without_adapter() {
        let temp_dir = TempDir::new().expect("temp dir");
        let broker = CodeModeFileBroker::new(temp_dir.path());
        let source = file_ref("oai_library://file_123");

        assert!(matches!(
            broker.read_to_bytes(&source),
            Err(FileBrokerError::UnsupportedProvider { .. })
        ));
    }
}
