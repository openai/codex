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
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsArgs;
use codex_protocol::request_permissions::WorkspaceMutationApprovalRequest;
use codex_protocol::request_permissions::WorkspaceMutationOperation;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use std::io;

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
}

#[derive(Serialize)]
struct WorkspaceMutationError {
    code: &'static str,
    message: String,
    cwd: AbsolutePathBuf,
    workspace_roots: Vec<AbsolutePathBuf>,
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
        let Some(environment) = turn.environments.primary() else {
            return Err(FunctionCallError::RespondToModel(
                "workspace mutation is unavailable without an execution environment".to_string(),
            ));
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
        let requested_permissions = if preview_policy
            .can_write_path_with_cwd(canonical.as_path(), cwd.as_path())
            && !current_policy.can_write_path_with_cwd(canonical.as_path(), current.cwd.as_path())
        {
            Some(FileSystemPermissions::from_read_write_roots(
                /*read*/ None,
                Some(vec![canonical.clone()]),
            ))
        } else if preview_policy.can_read_path_with_cwd(canonical.as_path(), cwd.as_path())
            && !current_policy.can_read_path_with_cwd(canonical.as_path(), current.cwd.as_path())
        {
            Some(FileSystemPermissions::from_read_write_roots(
                Some(vec![canonical.clone()]),
                /*write*/ None,
            ))
        } else {
            None
        };
        if let Some(file_system) = requested_permissions {
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
                        permissions: RequestPermissionProfile {
                            file_system: Some(file_system),
                            network: None,
                        },
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
            if response.permissions.is_empty()
                || !matches!(response.scope, PermissionGrantScope::Session)
            {
                return workspace_error(
                    "approval_denied",
                    "workspace mutation requires session-scoped approval".to_string(),
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
    workspace_output(
        WorkspaceMutationResult {
            changed,
            cwd,
            workspace_roots,
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
    workspace_output(
        WorkspaceMutationError {
            code,
            message,
            cwd,
            workspace_roots,
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
        Some(success),
    )))
}

fn io_error_code(err: &io::Error) -> &'static str {
    match err.kind() {
        io::ErrorKind::NotFound => "path_not_found",
        io::ErrorKind::PermissionDenied => "permission_denied",
        _ => "resolution_failed",
    }
}
