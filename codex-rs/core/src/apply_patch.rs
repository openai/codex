use crate::codex::TurnContext;
use crate::function_tool::FunctionCallError;
use crate::safety::SafetyCheck;
use crate::safety::assess_patch_safety;
use crate::tools::sandboxing::ExecApprovalRequirement;
use codex_apply_patch::ApplyPatchAction;
use codex_apply_patch::ApplyPatchError;
use codex_apply_patch::ApplyPatchFileChange;
use codex_apply_patch::ApplyPatchFileSystem;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::FileSystemOperationOptions;
use codex_exec_server::RemoveOptions;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::FileSystemSandboxPolicy;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

pub(crate) enum InternalApplyPatchInvocation {
    /// The `apply_patch` call was handled programmatically, without any sort
    /// of sandbox, because the user explicitly approved it. This is the
    /// result to use with the `shell` function call that contained `apply_patch`.
    Output(Result<String, FunctionCallError>),

    /// The `apply_patch` call was approved, either automatically because it
    /// appears that it should be allowed based on the user's sandbox policy
    /// or because the user explicitly approved it. The tool runtime realizes
    /// the verified patch through the environment filesystem.
    DelegateToRuntime(ApprovedApplyPatch),
}

#[derive(Debug)]
pub(crate) struct ApprovedApplyPatch {
    pub(crate) action: ApplyPatchAction,
    pub(crate) auto_approved: bool,
    pub(crate) exec_approval_requirement: ExecApprovalRequirement,
}

pub(crate) struct EnvironmentApplyPatchFileSystem {
    file_system: Arc<dyn ExecutorFileSystem>,
    operation_options: FileSystemOperationOptions,
}

impl EnvironmentApplyPatchFileSystem {
    pub(crate) fn for_verification(file_system: Arc<dyn ExecutorFileSystem>, cwd: PathBuf) -> Self {
        Self {
            file_system,
            operation_options: FileSystemOperationOptions {
                cwd: AbsolutePathBuf::from_absolute_path(cwd).ok(),
                ..FileSystemOperationOptions::default()
            },
        }
    }

    pub(crate) fn for_apply(
        file_system: Arc<dyn ExecutorFileSystem>,
        cwd: PathBuf,
        sandbox_policy: SandboxPolicy,
    ) -> Self {
        Self {
            file_system,
            operation_options: FileSystemOperationOptions {
                sandbox_policy: Some(sandbox_policy),
                cwd: AbsolutePathBuf::from_absolute_path(cwd).ok(),
            },
        }
    }
}

impl ApplyPatchFileSystem for EnvironmentApplyPatchFileSystem {
    fn read_text<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<String, ApplyPatchError>> + Send + 'a>>
    {
        Box::pin(async move {
            let path = absolute_path(path)?;
            let bytes = self
                .file_system
                .read_file_with_options(&path, &self.operation_options)
                .await
                .map_err(|source| {
                    ApplyPatchError::io_error(format!("Failed to read {}", path.display()), source)
                })?;
            String::from_utf8(bytes).map_err(|source| {
                ApplyPatchError::io_error(
                    format!("Failed to decode UTF-8 for {}", path.display()),
                    io::Error::new(io::ErrorKind::InvalidData, source.to_string()),
                )
            })
        })
    }

    fn write_text<'a>(
        &'a self,
        path: &'a std::path::Path,
        contents: String,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<(), ApplyPatchError>> + Send + 'a>> {
        Box::pin(async move {
            let path = absolute_path(path)?;
            let contents = contents.into_bytes();
            self.file_system
                .write_file_with_options(&path, contents, &self.operation_options)
                .await
                .map_err(|source| {
                    ApplyPatchError::io_error(
                        format!("Failed to write file {}", path.display()),
                        source,
                    )
                })
        })
    }

    fn create_dir_all<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<(), ApplyPatchError>> + Send + 'a>> {
        Box::pin(async move {
            let path = absolute_path(path)?;
            self.file_system
                .create_directory_with_options(
                    &path,
                    CreateDirectoryOptions { recursive: true },
                    &self.operation_options,
                )
                .await
                .map_err(|source| {
                    ApplyPatchError::io_error(
                        format!("Failed to create parent directories for {}", path.display()),
                        source,
                    )
                })
        })
    }

    fn remove_file<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<(), ApplyPatchError>> + Send + 'a>> {
        Box::pin(async move {
            let path = absolute_path(path)?;
            let remove_options = RemoveOptions {
                recursive: false,
                force: false,
            };
            self.file_system
                .remove_with_options(&path, remove_options, &self.operation_options)
                .await
                .map_err(|source| {
                    ApplyPatchError::io_error(
                        format!("Failed to delete file {}", path.display()),
                        source,
                    )
                })
        })
    }
}

fn absolute_path(path: &std::path::Path) -> std::result::Result<AbsolutePathBuf, ApplyPatchError> {
    let path = AbsolutePathBuf::from_absolute_path(path).map_err(|error| {
        ApplyPatchError::io_error(
            format!("Expected absolute path for apply_patch: {}", path.display()),
            io::Error::new(io::ErrorKind::InvalidInput, error.to_string()),
        )
    })?;
    Ok(normalize_existing_ancestor_path(path))
}

fn normalize_existing_ancestor_path(path: AbsolutePathBuf) -> AbsolutePathBuf {
    let raw_path = path.to_path_buf();
    for ancestor in raw_path.ancestors() {
        let Ok(canonical_ancestor) = ancestor.canonicalize() else {
            continue;
        };
        let Ok(suffix) = raw_path.strip_prefix(ancestor) else {
            continue;
        };
        if let Ok(normalized_path) =
            AbsolutePathBuf::from_absolute_path(canonical_ancestor.join(suffix))
        {
            return normalized_path;
        }
    }
    path
}

pub(crate) async fn apply_patch(
    turn_context: &TurnContext,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    action: ApplyPatchAction,
) -> InternalApplyPatchInvocation {
    match assess_patch_safety(
        &action,
        turn_context.approval_policy.value(),
        turn_context.sandbox_policy.get(),
        file_system_sandbox_policy,
        &turn_context.cwd,
        turn_context.windows_sandbox_level,
    ) {
        SafetyCheck::AutoApprove {
            user_explicitly_approved,
            ..
        } => InternalApplyPatchInvocation::DelegateToRuntime(ApprovedApplyPatch {
            action,
            auto_approved: !user_explicitly_approved,
            exec_approval_requirement: ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
            },
        }),
        SafetyCheck::AskUser => {
            // Delegate the approval prompt (including cached approvals) to the
            // tool runtime, consistent with how shell/unified_exec approvals
            // are orchestrator-driven.
            InternalApplyPatchInvocation::DelegateToRuntime(ApprovedApplyPatch {
                action,
                auto_approved: false,
                exec_approval_requirement: ExecApprovalRequirement::NeedsApproval {
                    reason: None,
                    proposed_execpolicy_amendment: None,
                },
            })
        }
        SafetyCheck::Reject { reason } => InternalApplyPatchInvocation::Output(Err(
            FunctionCallError::RespondToModel(format!("patch rejected: {reason}")),
        )),
    }
}

pub(crate) fn convert_apply_patch_to_protocol(
    action: &ApplyPatchAction,
) -> HashMap<PathBuf, FileChange> {
    let changes = action.changes();
    let mut result = HashMap::with_capacity(changes.len());
    for (path, change) in changes {
        let protocol_change = match change {
            ApplyPatchFileChange::Add { content } => FileChange::Add {
                content: content.clone(),
            },
            ApplyPatchFileChange::Delete { content } => FileChange::Delete {
                content: content.clone(),
            },
            ApplyPatchFileChange::Update {
                unified_diff,
                move_path,
                new_content: _new_content,
            } => FileChange::Update {
                unified_diff: unified_diff.clone(),
                move_path: move_path.clone(),
            },
        };
        result.insert(path.clone(), protocol_change);
    }
    result
}

#[cfg(test)]
#[path = "apply_patch_tests.rs"]
mod tests;
