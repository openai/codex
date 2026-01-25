use std::io;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use async_trait::async_trait;
use serde::Deserialize;
use tokio::fs;

use crate::function_tool::FunctionCallError;
use crate::protocol::EventMsg;
use crate::protocol::SandboxPolicy;
use crate::protocol::WriteFileToolCallEvent;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct WriteFileHandler;

#[derive(Deserialize)]
struct WriteFileArgs {
    /// Absolute path to the file that will be written.
    file_path: String,
    /// Content to write to the file.
    content: String,
    /// Whether to overwrite existing files; defaults to true.
    #[serde(default = "defaults::overwrite")]
    overwrite: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WriteSummary {
    bytes_written: usize,
    lines_written: usize,
    overwrote: bool,
}

#[async_trait]
impl ToolHandler for WriteFileHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        true
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            call_id,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "write_file handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: WriteFileArgs = parse_arguments(&arguments)?;

        let WriteFileArgs {
            file_path,
            content,
            overwrite,
        } = args;

        let path = PathBuf::from(&file_path);
        if !path.is_absolute() {
            return Err(FunctionCallError::RespondToModel(
                "file_path must be an absolute path".to_string(),
            ));
        }

        ensure_write_allowed(&path, &turn.sandbox_policy, &turn.cwd)?;
        let summary = write_content(&path, &content, overwrite).await?;
        let WriteSummary {
            bytes_written,
            lines_written,
            overwrote,
        } = summary;

        session
            .send_event(
                turn.as_ref(),
                EventMsg::WriteFileToolCall(WriteFileToolCallEvent {
                    call_id,
                    path: path.clone(),
                    bytes_written,
                    lines_written,
                    overwrote,
                }),
            )
            .await;

        let path_display = path.display();
        Ok(ToolOutput::Function {
            content: format!(
                "Wrote {lines_written} lines ({bytes_written} bytes) to {path_display}"
            ),
            content_items: None,
            success: Some(true),
        })
    }
}

fn ensure_write_allowed(
    path: &Path,
    sandbox_policy: &SandboxPolicy,
    cwd: &Path,
) -> Result<(), FunctionCallError> {
    match sandbox_policy {
        SandboxPolicy::ReadOnly => Err(FunctionCallError::RespondToModel(
            "write_file is not permitted in read-only mode".to_string(),
        )),
        SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. } => Ok(()),
        SandboxPolicy::WorkspaceWrite { .. } => {
            let normalized = normalize_path(path);
            let writable_roots = sandbox_policy.get_writable_roots_with_cwd(cwd);
            if writable_roots
                .iter()
                .any(|root| root.is_path_writable(&normalized))
            {
                Ok(())
            } else {
                let path_display = path.display();
                Err(FunctionCallError::RespondToModel(format!(
                    "path `{path_display}` is outside writable roots"
                )))
            }
        }
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

async fn write_content(
    path: &Path,
    content: &str,
    overwrite: bool,
) -> Result<WriteSummary, FunctionCallError> {
    let mut overwrote = false;
    match fs::symlink_metadata(path).await {
        Ok(metadata) => {
            let path_display = path.display();
            if metadata.is_dir() {
                return Err(FunctionCallError::RespondToModel(format!(
                    "file_path `{path_display}` is a directory"
                )));
            }
            if !overwrite {
                return Err(FunctionCallError::RespondToModel(
                    "file already exists and overwrite is false".to_string(),
                ));
            }
            overwrote = true;
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(FunctionCallError::RespondToModel(format!(
                "failed to inspect file: {err}"
            )));
        }
    }

    fs::write(path, content)
        .await
        .map_err(|err| FunctionCallError::RespondToModel(format!("failed to write file: {err}")))?;

    Ok(WriteSummary {
        bytes_written: content.len(),
        lines_written: count_lines(content),
        overwrote,
    })
}

fn count_lines(content: &str) -> usize {
    if content.is_empty() {
        0
    } else {
        content.lines().count()
    }
}

mod defaults {
    pub fn overwrite() -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::protocol::SandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[tokio::test]
    async fn write_content_writes_multiline_file() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let path = dir.path().join("sample.txt");
        let content = "alpha\nbeta\n";

        let summary = write_content(&path, content, true).await?;
        let actual = std::fs::read_to_string(&path)?;
        assert_eq!(actual, content);
        assert_eq!(
            summary,
            WriteSummary {
                bytes_written: content.len(),
                lines_written: 2,
                overwrote: false,
            }
        );
        Ok(())
    }

    #[tokio::test]
    async fn write_content_rejects_existing_file_without_overwrite() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let path = dir.path().join("sample.txt");
        std::fs::write(&path, "one")?;

        let err = write_content(&path, "two", false)
            .await
            .expect_err("should reject existing file");
        assert_eq!(
            err,
            FunctionCallError::RespondToModel(
                "file already exists and overwrite is false".to_string()
            )
        );
        Ok(())
    }

    #[test]
    fn ensure_write_allowed_respects_workspace_roots() {
        let dir = tempdir().expect("tempdir");
        let cwd = dir.path().to_path_buf();
        let inside = cwd.join("inside.txt");
        let outside = cwd.parent().expect("parent").join("outside.txt");
        let outside_display = outside.display();
        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: Vec::<AbsolutePathBuf>::new(),
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        assert_eq!(ensure_write_allowed(&inside, &policy, &cwd), Ok(()));
        assert_eq!(
            ensure_write_allowed(&outside, &policy, &cwd),
            Err(FunctionCallError::RespondToModel(format!(
                "path `{outside_display}` is outside writable roots"
            )))
        );
    }
}
