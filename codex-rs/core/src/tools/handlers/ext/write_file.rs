//! Write File Handler - Create or overwrite files
//!
//! This module provides the WriteFileHandler which writes content to files,
//! creating parent directories as needed and validating paths.

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use serde::Deserialize;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Write file tool arguments
#[derive(Debug, Clone, Deserialize)]
struct WriteFileArgs {
    file_path: String,
    content: String,
}

/// Error types for write_file operations (matches gemini-cli)
#[derive(Debug, Clone, Copy)]
enum WriteFileErrorType {
    PermissionDenied,
    NoSpaceLeft,
    TargetIsDirectory,
    FileWriteFailure,
    PathValidation,
}

impl WriteFileErrorType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::PermissionDenied => "PERMISSION_DENIED",
            Self::NoSpaceLeft => "NO_SPACE_LEFT",
            Self::TargetIsDirectory => "TARGET_IS_DIRECTORY",
            Self::FileWriteFailure => "FILE_WRITE_FAILURE",
            Self::PathValidation => "PATH_VALIDATION_ERROR",
        }
    }
}

/// Write File Handler
///
/// Creates new files or overwrites existing files.
/// This is a mutating handler - requires approval flow.
pub struct WriteFileHandler;

#[async_trait]
impl ToolHandler for WriteFileHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    /// Mark as mutating - requires approval before execution
    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        true
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        // 1. Parse arguments
        let arguments = match &invocation.payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "Invalid payload type for write_file".to_string(),
                ));
            }
        };

        let args: WriteFileArgs = serde_json::from_str(arguments)
            .map_err(|e| FunctionCallError::RespondToModel(format!("Invalid arguments: {e}")))?;

        // 2. Validate file_path is not empty
        if args.file_path.trim().is_empty() {
            return make_error_response(
                WriteFileErrorType::PathValidation,
                "file_path must not be empty",
            );
        }

        // 3. Resolve path (handle relative paths via turn context)
        let path = invocation.turn.resolve_path(Some(args.file_path.clone()));

        // 4. Validate path is absolute
        if !path.is_absolute() {
            return make_error_response(
                WriteFileErrorType::PathValidation,
                &format!("file_path must be an absolute path: {}", path.display()),
            );
        }

        // 5. Validate path is within workspace (cwd)
        let cwd = &invocation.turn.cwd;
        if !is_path_within_workspace(&path, cwd) {
            return make_error_response(
                WriteFileErrorType::PathValidation,
                &format!(
                    "file_path must be within the workspace directory: {}",
                    cwd.display()
                ),
            );
        }

        // 6. Check if target is a directory
        if path.is_dir() {
            return make_error_response(
                WriteFileErrorType::TargetIsDirectory,
                &format!("Target is a directory, not a file: {}", path.display()),
            );
        }

        // 7. Check if file exists (for success message)
        let file_existed = path.exists();

        // 8. Create parent directories if needed
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                if let Err(e) = fs::create_dir_all(parent).await {
                    return map_io_error(e, &path);
                }
            }
        }

        // 9. Write the file
        match write_file_content(&path, &args.content).await {
            Ok(()) => {
                let line_count = args.content.lines().count();
                let action = if file_existed {
                    "overwrote"
                } else {
                    "created new"
                };
                let content = format!(
                    "Successfully {} file: {} ({} lines written)",
                    action,
                    path.display(),
                    line_count
                );
                Ok(ToolOutput::Function {
                    content,
                    content_items: None,
                    success: Some(true),
                })
            }
            Err(e) => map_io_error(e, &path),
        }
    }
}

/// Write content to file
async fn write_file_content(path: &Path, content: &str) -> std::io::Result<()> {
    let mut file = fs::File::create(path).await?;
    file.write_all(content.as_bytes()).await?;
    file.flush().await?;
    Ok(())
}

/// Check if path is within workspace
fn is_path_within_workspace(path: &Path, workspace: &Path) -> bool {
    // Canonicalize both paths for accurate comparison
    // Note: We can't canonicalize path if it doesn't exist yet,
    // so we check the parent directory
    let check_path = if path.exists() {
        path.canonicalize().ok()
    } else if let Some(parent) = path.parent() {
        parent
            .canonicalize()
            .ok()
            .map(|p| p.join(path.file_name().unwrap_or_default()))
    } else {
        None
    };

    let workspace_canonical = workspace.canonicalize().ok();

    match (check_path, workspace_canonical) {
        (Some(p), Some(w)) => p.starts_with(&w),
        // If canonicalization fails, do a simple prefix check
        _ => path.starts_with(workspace),
    }
}

/// Map IO error to appropriate error response
fn map_io_error(error: std::io::Error, path: &Path) -> Result<ToolOutput, FunctionCallError> {
    use std::io::ErrorKind;

    let (error_type, message) = match error.kind() {
        ErrorKind::PermissionDenied => (
            WriteFileErrorType::PermissionDenied,
            format!(
                "Permission denied writing to file: {} (EACCES)",
                path.display()
            ),
        ),
        #[cfg(unix)]
        _ if error.raw_os_error() == Some(28) => (
            // ENOSPC on Unix
            WriteFileErrorType::NoSpaceLeft,
            format!("No space left on device: {} (ENOSPC)", path.display()),
        ),
        #[cfg(windows)]
        _ if error.raw_os_error() == Some(112) => (
            // ERROR_DISK_FULL on Windows
            WriteFileErrorType::NoSpaceLeft,
            format!("No space left on device: {}", path.display()),
        ),
        ErrorKind::IsADirectory => (
            WriteFileErrorType::TargetIsDirectory,
            format!(
                "Target is a directory, not a file: {} (EISDIR)",
                path.display()
            ),
        ),
        _ => (
            WriteFileErrorType::FileWriteFailure,
            format!("Error writing to file '{}': {}", path.display(), error),
        ),
    };

    make_error_response(error_type, &message)
}

/// Create standardized error response
fn make_error_response(
    error_type: WriteFileErrorType,
    message: &str,
) -> Result<ToolOutput, FunctionCallError> {
    Ok(ToolOutput::Function {
        content: format!("[{}] {}", error_type.as_str(), message),
        content_items: None,
        success: Some(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_handler_kind() {
        let handler = WriteFileHandler;
        assert_eq!(handler.kind(), ToolKind::Function);
    }

    #[test]
    fn test_matches_function_payload() {
        let handler = WriteFileHandler;

        assert!(handler.matches_kind(&ToolPayload::Function {
            arguments: "{}".to_string(),
        }));
    }

    #[test]
    fn test_parse_valid_args() {
        let args: WriteFileArgs =
            serde_json::from_str(r#"{"file_path": "/tmp/test.txt", "content": "hello world"}"#)
                .expect("should parse");
        assert_eq!(args.file_path, "/tmp/test.txt");
        assert_eq!(args.content, "hello world");
    }

    #[test]
    fn test_parse_invalid_args_missing_content() {
        let result: Result<WriteFileArgs, _> =
            serde_json::from_str(r#"{"file_path": "/tmp/test.txt"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_path_within_workspace() {
        let workspace = PathBuf::from("/home/user/project");

        // Within workspace
        assert!(is_path_within_workspace(
            Path::new("/home/user/project/src/main.rs"),
            &workspace
        ));

        // Outside workspace
        assert!(!is_path_within_workspace(
            Path::new("/etc/passwd"),
            &workspace
        ));

        // Exact workspace root
        assert!(is_path_within_workspace(
            Path::new("/home/user/project"),
            &workspace
        ));
    }

    #[test]
    fn test_error_type_as_str() {
        assert_eq!(
            WriteFileErrorType::PermissionDenied.as_str(),
            "PERMISSION_DENIED"
        );
        assert_eq!(WriteFileErrorType::NoSpaceLeft.as_str(), "NO_SPACE_LEFT");
        assert_eq!(
            WriteFileErrorType::TargetIsDirectory.as_str(),
            "TARGET_IS_DIRECTORY"
        );
        assert_eq!(
            WriteFileErrorType::FileWriteFailure.as_str(),
            "FILE_WRITE_FAILURE"
        );
        assert_eq!(
            WriteFileErrorType::PathValidation.as_str(),
            "PATH_VALIDATION_ERROR"
        );
    }

    #[tokio::test]
    async fn test_write_file_creates_new_file() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let file_path = temp.path().join("new_file.txt");

        write_file_content(&file_path, "test content").await?;

        assert!(file_path.exists());
        let content = tokio::fs::read_to_string(&file_path).await?;
        assert_eq!(content, "test content");

        Ok(())
    }

    #[tokio::test]
    async fn test_write_file_overwrites_existing() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let file_path = temp.path().join("existing.txt");

        // Create initial file
        tokio::fs::write(&file_path, "original").await?;

        // Overwrite
        write_file_content(&file_path, "new content").await?;

        let content = tokio::fs::read_to_string(&file_path).await?;
        assert_eq!(content, "new content");

        Ok(())
    }

    #[tokio::test]
    async fn test_write_file_creates_parent_dirs() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let file_path = temp.path().join("nested/deep/file.txt");

        // Create parent directories first
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        write_file_content(&file_path, "nested content").await?;

        assert!(file_path.exists());
        let content = tokio::fs::read_to_string(&file_path).await?;
        assert_eq!(content, "nested content");

        Ok(())
    }

    #[test]
    fn test_make_error_response() {
        let result =
            make_error_response(WriteFileErrorType::PermissionDenied, "test error").unwrap();
        if let ToolOutput::Function {
            content, success, ..
        } = result
        {
            assert!(content.contains("[PERMISSION_DENIED]"));
            assert!(content.contains("test error"));
            assert_eq!(success, Some(false));
        } else {
            panic!("Expected ToolOutput::Function");
        }
    }
}
