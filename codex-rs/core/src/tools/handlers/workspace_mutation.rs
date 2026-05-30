use crate::function_tool::FunctionCallError;
use crate::session::session::SessionSettingsUpdate;
use crate::session::thread_settings_applied_event;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::parse_arguments;
use crate::tools::handlers::workspace_mutation_spec::create_add_workspace_root_tool;
use crate::tools::handlers::workspace_mutation_spec::create_set_working_directory_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsArgs;
use codex_protocol::request_permissions::WorkspaceMutationApprovalRequest;
use codex_protocol::request_permissions::WorkspaceMutationOperation;
use codex_sandboxing::policy_transforms::intersect_permission_profiles;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use std::io;
use std::path::Path;

const MAX_MODEL_VISIBLE_WORKSPACE_ROOTS: usize = 16;

#[derive(Clone, Copy)]
enum WorkspaceMutation {
    SetWorkingDirectory,
    AddWorkspaceRoot,
}

pub(crate) struct WorkspaceMutationHandler {
    mutation: WorkspaceMutation,
}

impl WorkspaceMutationHandler {
    pub(crate) fn set_working_directory() -> Self {
        Self {
            mutation: WorkspaceMutation::SetWorkingDirectory,
        }
    }

    pub(crate) fn add_workspace_root() -> Self {
        Self {
            mutation: WorkspaceMutation::AddWorkspaceRoot,
        }
    }
}

#[derive(Deserialize)]
struct WorkspaceMutationArgs {
    path: String,
}

#[derive(Serialize)]
struct WorkspaceMutationResult {
    changed: bool,
    cwd: AbsolutePathBuf,
    workspace_roots: Vec<AbsolutePathBuf>,
    #[serde(skip_serializing_if = "is_zero")]
    omitted_workspace_roots: usize,
}

#[derive(Serialize)]
struct WorkspaceMutationError {
    code: &'static str,
    message: String,
    cwd: AbsolutePathBuf,
    workspace_roots: Vec<AbsolutePathBuf>,
    #[serde(skip_serializing_if = "is_zero")]
    omitted_workspace_roots: usize,
}

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for WorkspaceMutationHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(match self.mutation {
            WorkspaceMutation::SetWorkingDirectory => "set_working_directory",
            WorkspaceMutation::AddWorkspaceRoot => "add_workspace_root",
        })
    }

    fn spec(&self) -> ToolSpec {
        match self.mutation {
            WorkspaceMutation::SetWorkingDirectory => create_set_working_directory_tool(),
            WorkspaceMutation::AddWorkspaceRoot => create_add_workspace_root_tool(),
        }
    }

    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            cancellation_token,
            call_id,
            payload,
            ..
        } = invocation;
        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "workspace mutation handler received unsupported payload".to_string(),
                ));
            }
        };
        let args: WorkspaceMutationArgs = parse_arguments(&arguments)?;
        let current = turn.runtime_workspace.snapshot().await;
        let requested = current.cwd.join(args.path);
        let environment = match turn.environments.turn_environments.as_slice() {
            [environment] => environment,
            [] => {
                return Err(FunctionCallError::RespondToModel(
                    "workspace mutation is unavailable without an execution environment"
                        .to_string(),
                ));
            }
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "workspace mutation is unavailable with multiple execution environments"
                        .to_string(),
                ));
            }
        };
        let fs = environment.environment.get_filesystem();
        let canonical = match fs.canonicalize(&requested, /*sandbox*/ None).await {
            Ok(path) => path,
            Err(err) => {
                return workspace_error(
                    io_error_code(&err),
                    err.to_string(),
                    current.cwd,
                    current.workspace_roots,
                );
            }
        };
        let metadata = match fs.get_metadata(&canonical, /*sandbox*/ None).await {
            Ok(metadata) => metadata,
            Err(err) => {
                return workspace_error(
                    io_error_code(&err),
                    err.to_string(),
                    current.cwd,
                    current.workspace_roots,
                );
            }
        };
        if !metadata.is_directory {
            return workspace_error(
                "not_a_directory",
                format!(
                    "workspace mutation target is not a directory: {}",
                    canonical.as_path().display()
                ),
                current.cwd,
                current.workspace_roots,
            );
        }

        let mut workspace_roots = current.workspace_roots.clone();
        if !workspace_roots
            .iter()
            .any(|root| canonical.as_path().starts_with(root.as_path()))
        {
            workspace_roots.push(canonical.clone());
        }
        let cwd = match self.mutation {
            WorkspaceMutation::SetWorkingDirectory => canonical.clone(),
            WorkspaceMutation::AddWorkspaceRoot => current.cwd.clone(),
        };
        let changed = cwd != current.cwd || workspace_roots != current.workspace_roots;
        if !changed {
            return workspace_success(/*changed*/ false, cwd, workspace_roots);
        }

        let preview = session
            .preview_settings(&SessionSettingsUpdate {
                cwd: matches!(self.mutation, WorkspaceMutation::SetWorkingDirectory)
                    .then(|| cwd.to_path_buf()),
                workspace_roots: Some(workspace_roots.clone()),
                ..Default::default()
            })
            .await
            .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?;
        let current_policy = current.permission_profile.file_system_sandbox_policy();
        let preview_policy = preview.permission_profile.file_system_sandbox_policy();
        if matches!(self.mutation, WorkspaceMutation::SetWorkingDirectory)
            && !preview_policy.can_read_path_with_cwd(canonical.as_path(), cwd.as_path())
        {
            return workspace_error(
                "permission_denied",
                format!(
                    "working directory is not readable under the active permission profile: {}",
                    canonical.as_path().display()
                ),
                current.cwd,
                current.workspace_roots,
            );
        }
        let requested_permissions = newly_accessible_roots(
            &current_policy,
            current.cwd.as_path(),
            &preview_policy,
            cwd.as_path(),
        );
        if let Some(file_system) = requested_permissions {
            let requested_permissions = RequestPermissionProfile {
                file_system: Some(file_system),
                network: None,
            };
            let response = session
                .request_workspace_permissions_for_cwd(
                    &turn,
                    call_id,
                    RequestPermissionsArgs {
                        reason: Some(match self.mutation {
                            WorkspaceMutation::SetWorkingDirectory => format!(
                                "switch this session's working directory to `{}`",
                                canonical.as_path().display()
                            ),
                            WorkspaceMutation::AddWorkspaceRoot => format!(
                                "add `{}` to this session's workspace",
                                canonical.as_path().display()
                            ),
                        }),
                        permissions: requested_permissions.clone(),
                    },
                    current.cwd.clone(),
                    WorkspaceMutationApprovalRequest {
                        operation: match self.mutation {
                            WorkspaceMutation::SetWorkingDirectory => {
                                WorkspaceMutationOperation::SetWorkingDirectory
                            }
                            WorkspaceMutation::AddWorkspaceRoot => {
                                WorkspaceMutationOperation::AddWorkspaceRoot
                            }
                        },
                        target: canonical.clone(),
                        resulting_workspace_roots: workspace_roots.clone(),
                    },
                    cancellation_token,
                )
                .await;
            let Some(response) = response else {
                return workspace_error(
                    "approval_denied",
                    "workspace mutation approval was cancelled".to_string(),
                    current.cwd,
                    current.workspace_roots,
                );
            };
            if !matches!(response.scope, PermissionGrantScope::Session)
                || !permissions_are_approved(
                    requested_permissions,
                    response.permissions,
                    current.cwd.as_path(),
                )
            {
                return workspace_error(
                    "approval_denied",
                    "workspace mutation requires session-scoped approval with the requested filesystem access"
                        .to_string(),
                    current.cwd,
                    current.workspace_roots,
                );
            }
        }

        session
            .update_runtime_workspace(
                turn.as_ref(),
                matches!(self.mutation, WorkspaceMutation::SetWorkingDirectory)
                    .then_some(cwd.clone()),
                workspace_roots.clone(),
            )
            .await
            .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?;
        session
            .send_event(
                turn.as_ref(),
                thread_settings_applied_event(session.as_ref()).await,
            )
            .await;
        workspace_success(/*changed*/ true, cwd, workspace_roots)
    }
}

impl CoreToolRuntime for WorkspaceMutationHandler {
    fn execution_barrier(&self) -> bool {
        true
    }

    fn cancel_suffix_on_failure(&self) -> bool {
        true
    }
}

fn workspace_success(
    changed: bool,
    cwd: AbsolutePathBuf,
    workspace_roots: Vec<AbsolutePathBuf>,
) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
    let (workspace_roots, omitted_workspace_roots) = bounded_workspace_roots(workspace_roots);
    workspace_output(
        WorkspaceMutationResult {
            changed,
            cwd,
            workspace_roots,
            omitted_workspace_roots,
        },
        /*success*/ true,
    )
}

fn workspace_error(
    code: &'static str,
    message: String,
    cwd: AbsolutePathBuf,
    workspace_roots: Vec<AbsolutePathBuf>,
) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
    let (workspace_roots, omitted_workspace_roots) = bounded_workspace_roots(workspace_roots);
    workspace_output(
        WorkspaceMutationError {
            code,
            message,
            cwd,
            workspace_roots,
            omitted_workspace_roots,
        },
        /*success*/ false,
    )
}

fn workspace_output(
    output: impl Serialize,
    success: bool,
) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
    let content = serde_json::to_string(&output).map_err(|err| {
        FunctionCallError::Fatal(format!(
            "failed to serialize workspace mutation result: {err}"
        ))
    })?;
    Ok(boxed_tool_output(FunctionToolOutput::from_text(
        content,
        /*success*/ Some(success),
    )))
}

fn newly_accessible_roots(
    current_policy: &FileSystemSandboxPolicy,
    current_cwd: &Path,
    preview_policy: &FileSystemSandboxPolicy,
    preview_cwd: &Path,
) -> Option<FileSystemPermissions> {
    let write = preview_policy
        .get_writable_roots_with_cwd(preview_cwd)
        .into_iter()
        .map(|root| root.root)
        .filter(|root| !current_policy.can_write_path_with_cwd(root.as_path(), current_cwd))
        .collect::<Vec<_>>();
    let read = preview_policy
        .get_readable_roots_with_cwd(preview_cwd)
        .into_iter()
        .filter(|root| !current_policy.can_read_path_with_cwd(root.as_path(), current_cwd))
        .filter(|root| {
            !write
                .iter()
                .any(|writable_root| root.as_path().starts_with(writable_root.as_path()))
        })
        .collect::<Vec<_>>();
    if read.is_empty() && write.is_empty() {
        None
    } else {
        Some(FileSystemPermissions::from_read_write_roots(
            /*read*/ (!read.is_empty()).then_some(read),
            /*write*/ (!write.is_empty()).then_some(write),
        ))
    }
}

fn permissions_are_approved(
    requested: RequestPermissionProfile,
    granted: RequestPermissionProfile,
    cwd: &Path,
) -> bool {
    let requested: AdditionalPermissionProfile = requested.into();
    let granted: AdditionalPermissionProfile = granted.into();
    intersect_permission_profiles(requested.clone(), granted, cwd) == requested
}

fn bounded_workspace_roots(workspace_roots: Vec<AbsolutePathBuf>) -> (Vec<AbsolutePathBuf>, usize) {
    let omitted_workspace_roots = workspace_roots
        .len()
        .saturating_sub(MAX_MODEL_VISIBLE_WORKSPACE_ROOTS);
    (
        workspace_roots
            .into_iter()
            .take(MAX_MODEL_VISIBLE_WORKSPACE_ROOTS)
            .collect(),
        omitted_workspace_roots,
    )
}

// Serde passes `skip_serializing_if` predicates a reference.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(value: &usize) -> bool {
    *value == 0
}

fn io_error_code(err: &io::Error) -> &'static str {
    match err.kind() {
        io::ErrorKind::NotFound => "path_not_found",
        io::ErrorKind::PermissionDenied => "permission_denied",
        _ => "resolution_failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemPath;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::FileSystemSpecialPath;

    #[test]
    fn newly_accessible_roots_include_materialized_workspace_subpaths() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let workspace_root = AbsolutePathBuf::try_from(
            std::fs::canonicalize(temp_dir.path()).expect("canonical tempdir"),
        )
        .expect("absolute tempdir");
        let current_policy = FileSystemSandboxPolicy::restricted(Vec::new());
        let preview_policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(Some(".codex".into())),
            },
            access: FileSystemAccessMode::Write,
        }])
        .materialize_project_roots_with_workspace_roots(std::slice::from_ref(&workspace_root));

        assert_eq!(
            newly_accessible_roots(
                &current_policy,
                workspace_root.as_path(),
                &preview_policy,
                workspace_root.as_path(),
            ),
            Some(FileSystemPermissions::from_read_write_roots(
                /*read*/ None,
                /*write*/ Some(vec![workspace_root.join(".codex")]),
            ))
        );
    }

    #[test]
    fn bounded_workspace_roots_reports_omitted_count() {
        let roots = (0..MAX_MODEL_VISIBLE_WORKSPACE_ROOTS + 2)
            .map(|index| {
                AbsolutePathBuf::from_absolute_path(format!("/root-{index}"))
                    .expect("absolute test root")
            })
            .collect();

        let (visible_roots, omitted_workspace_roots) = bounded_workspace_roots(roots);

        assert_eq!(visible_roots.len(), MAX_MODEL_VISIBLE_WORKSPACE_ROOTS);
        assert_eq!(omitted_workspace_roots, 2);
    }
}
