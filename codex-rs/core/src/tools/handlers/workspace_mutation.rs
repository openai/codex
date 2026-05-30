use crate::function_tool::FunctionCallError;
use crate::session::session::Session;
use crate::session::session::SessionSettingsUpdate;
use crate::session::thread_settings_applied_event;
use crate::session::turn_context::TurnContext;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::parse_arguments;
use crate::tools::handlers::workspace_mutation_spec::create_add_workspace_root_tool;
use crate::tools::handlers::workspace_mutation_spec::create_set_working_directory_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutionPolicy;
use crate::tools::registry::ToolExecutor;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::FileSystemSandboxContext;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::protocol::TurnEnvironmentSelections;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsArgs;
use codex_protocol::request_permissions::WorkspaceMutationApprovalRequest;
use codex_protocol::request_permissions::WorkspaceMutationOperation;
use codex_sandboxing::policy_transforms::intersect_permission_profiles;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use serde::Deserialize;
use serde::Serialize;
use std::io;
use std::path::Path;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

const MAX_MODEL_VISIBLE_WORKSPACE_ROOTS: usize = 16;

pub(crate) struct WorkspaceMutationHandler {
    operation: WorkspaceMutationOperation,
}

impl WorkspaceMutationHandler {
    pub(crate) fn set_working_directory() -> Self {
        Self {
            operation: WorkspaceMutationOperation::SetWorkingDirectory,
        }
    }

    pub(crate) fn add_workspace_root() -> Self {
        Self {
            operation: WorkspaceMutationOperation::AddWorkspaceRoot,
        }
    }
}

#[derive(Deserialize)]
struct WorkspaceMutationArgs {
    path: String,
}

pub(crate) struct WorkspaceMutationPlan {
    pub(crate) changed: bool,
    pub(crate) cwd: AbsolutePathBuf,
    pub(crate) workspace_roots: Vec<AbsolutePathBuf>,
}

struct WorkspacePermissionRequest {
    call_id: String,
    environment_id: String,
    target: AbsolutePathBuf,
    resulting_workspace_roots: Vec<AbsolutePathBuf>,
    requested_permissions: RequestPermissionProfile,
    current_cwd: AbsolutePathBuf,
    cancellation_token: CancellationToken,
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
pub(crate) struct WorkspaceMutationError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) cwd: AbsolutePathBuf,
    pub(crate) workspace_roots: Vec<AbsolutePathBuf>,
    #[serde(skip_serializing_if = "is_zero")]
    omitted_workspace_roots: usize,
}

pub(crate) async fn plan_workspace_mutation(
    session: &Session,
    turn: &TurnContext,
    operation: WorkspaceMutationOperation,
    path: String,
) -> Result<WorkspaceMutationPlan, WorkspaceMutationError> {
    let current = session.runtime_workspace_snapshot().await;
    let Some(environment) = turn
        .environments
        .primary()
        .filter(|_| turn.environments.turn_environments.len() == 1)
    else {
        return Err(WorkspaceMutationError {
            code: "unsupported_environment_count",
            message: "workspace mutation requires exactly one execution environment".to_string(),
            cwd: current.cwd,
            workspace_roots: current.workspace_roots,
            omitted_workspace_roots: 0,
        });
    };
    let requested = current.cwd.join(path);
    let fs = environment.environment.get_filesystem();
    let canonical = fs
        .canonicalize(&requested, /*sandbox*/ None)
        .await
        .map_err(|err| WorkspaceMutationError {
            code: io_error_code(&err),
            message: err.to_string(),
            cwd: current.cwd.clone(),
            workspace_roots: current.workspace_roots.clone(),
            omitted_workspace_roots: 0,
        })?;
    let metadata = fs
        .get_metadata(&canonical, /*sandbox*/ None)
        .await
        .map_err(|err| WorkspaceMutationError {
            code: io_error_code(&err),
            message: err.to_string(),
            cwd: current.cwd.clone(),
            workspace_roots: current.workspace_roots.clone(),
            omitted_workspace_roots: 0,
        })?;
    if !metadata.is_directory {
        return Err(WorkspaceMutationError {
            code: "not_a_directory",
            message: format!(
                "workspace mutation target is not a directory: {}",
                canonical.as_path().display()
            ),
            cwd: current.cwd,
            workspace_roots: current.workspace_roots,
            omitted_workspace_roots: 0,
        });
    }

    Ok(workspace_mutation_plan(
        operation,
        &current.cwd,
        &current.workspace_roots,
        canonical,
    ))
}

impl ToolExecutor<ToolInvocation> for WorkspaceMutationHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(match self.operation {
            WorkspaceMutationOperation::SetWorkingDirectory => "set_working_directory",
            WorkspaceMutationOperation::AddWorkspaceRoot => "add_workspace_root",
        })
    }

    fn spec(&self) -> ToolSpec {
        match self.operation {
            WorkspaceMutationOperation::SetWorkingDirectory => create_set_working_directory_tool(),
            WorkspaceMutationOperation::AddWorkspaceRoot => create_add_workspace_root_tool(),
        }
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(async move {
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
            let current = session.runtime_workspace_snapshot().await;
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
            if !session
                .runtime_workspace_mutation_environment_matches(&environment.environment_id)
                .await
            {
                return Err(FunctionCallError::RespondToModel(
                "workspace mutation is unavailable unless the session has exactly one persisted execution environment"
                    .to_string(),
            ));
            }
            let active_sandbox = turn.file_system_sandbox_context_for_permission_profile(
                &current.permission_profile,
                /*additional_permissions*/ None,
                &current.cwd,
            );
            let mut inspection_permissions = None;
            let canonical = match resolve_workspace_directory(
                fs.as_ref(),
                &requested,
                &active_sandbox,
            )
            .await
            {
                Ok(path) => path,
                Err(_) => {
                    let provisional = workspace_mutation_plan(
                        self.operation.clone(),
                        &current.cwd,
                        &current.workspace_roots,
                        requested.clone(),
                    );
                    let environments = matches!(
                        self.operation,
                        WorkspaceMutationOperation::SetWorkingDirectory
                    )
                    .then(|| {
                        TurnEnvironmentSelections::new(
                            provisional.cwd.clone(),
                            vec![environment.selection()],
                        )
                    });
                    let preview = session
                        .preview_settings(&SessionSettingsUpdate {
                            environments,
                            workspace_roots: Some(provisional.workspace_roots.clone()),
                            ..Default::default()
                        })
                        .await
                        .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?;
                    let Some(file_system) = newly_accessible_roots(
                        &current.permission_profile.file_system_sandbox_policy(),
                        current.cwd.as_path(),
                        &preview.permission_profile.file_system_sandbox_policy(),
                        provisional.cwd.as_path(),
                    ) else {
                        return workspace_error(
                        "permission_denied",
                        "workspace mutation target is unavailable under the active permission profile"
                            .to_string(),
                        current.cwd,
                        current.workspace_roots,
                    );
                    };
                    let requested_permissions = RequestPermissionProfile {
                        file_system: Some(file_system),
                        network: None,
                    };
                    let granted_permissions = match self
                        .request_permissions(
                            &session,
                            &turn,
                            WorkspacePermissionRequest {
                                call_id: call_id.clone(),
                                environment_id: environment.environment_id.clone(),
                                target: requested.clone(),
                                resulting_workspace_roots: provisional.workspace_roots,
                                requested_permissions,
                                current_cwd: current.cwd.clone(),
                                cancellation_token: cancellation_token.clone(),
                            },
                        )
                        .await
                    {
                        Ok(permissions) => permissions,
                        Err(message) => {
                            return workspace_error(
                                "approval_denied",
                                message.to_string(),
                                current.cwd,
                                current.workspace_roots,
                            );
                        }
                    };
                    let additional_permissions = granted_permissions.clone().into();
                    let inspection_sandbox = turn
                        .file_system_sandbox_context_for_permission_profile(
                            &current.permission_profile,
                            Some(additional_permissions),
                            &current.cwd,
                        );
                    inspection_permissions = Some(granted_permissions);
                    match resolve_workspace_directory(fs.as_ref(), &requested, &inspection_sandbox)
                        .await
                    {
                        Ok(path) => path,
                        Err(err) => {
                            return workspace_error(
                                io_error_code(&err),
                                err.to_string(),
                                current.cwd,
                                current.workspace_roots,
                            );
                        }
                    }
                }
            };

            let plan = workspace_mutation_plan(
                self.operation.clone(),
                &current.cwd,
                &current.workspace_roots,
                canonical.clone(),
            );
            if !plan.changed {
                return workspace_success(/*changed*/ false, plan.cwd, plan.workspace_roots);
            }

            let environments = matches!(
                self.operation,
                WorkspaceMutationOperation::SetWorkingDirectory
            )
            .then(|| {
                TurnEnvironmentSelections::new(plan.cwd.clone(), vec![environment.selection()])
            });
            let preview = session
                .preview_settings(&SessionSettingsUpdate {
                    environments,
                    workspace_roots: Some(plan.workspace_roots.clone()),
                    ..Default::default()
                })
                .await
                .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?;
            let current_policy = current.permission_profile.file_system_sandbox_policy();
            let preview_policy = preview.permission_profile.file_system_sandbox_policy();
            if matches!(
                self.operation,
                WorkspaceMutationOperation::SetWorkingDirectory
            ) && !preview_policy.can_read_path_with_cwd(canonical.as_path(), plan.cwd.as_path())
            {
                return workspace_error(
                    "permission_denied",
                    "workspace mutation target is unavailable under the active permission profile"
                        .to_string(),
                    current.cwd,
                    current.workspace_roots,
                );
            }
            let requested_permissions = newly_accessible_roots(
                &current_policy,
                current.cwd.as_path(),
                &preview_policy,
                plan.cwd.as_path(),
            );
            if let Some(file_system) = requested_permissions {
                let requested_permissions = RequestPermissionProfile {
                    file_system: Some(file_system),
                    network: None,
                };
                if inspection_permissions.as_ref().is_some_and(|granted| {
                    permissions_are_approved(
                        requested_permissions.clone(),
                        granted.clone(),
                        current.cwd.as_path(),
                    )
                }) {
                    self.apply_plan(&session, turn.as_ref(), &plan).await?;
                    return workspace_success(
                        /*changed*/ true,
                        plan.cwd,
                        plan.workspace_roots,
                    );
                }
                if let Err(message) = self
                    .request_permissions(
                        &session,
                        &turn,
                        WorkspacePermissionRequest {
                            call_id,
                            environment_id: environment.environment_id.clone(),
                            target: canonical,
                            resulting_workspace_roots: plan.workspace_roots.clone(),
                            requested_permissions,
                            current_cwd: current.cwd.clone(),
                            cancellation_token,
                        },
                    )
                    .await
                {
                    return workspace_error(
                        "approval_denied",
                        message.to_string(),
                        current.cwd,
                        current.workspace_roots,
                    );
                }
            }

            self.apply_plan(&session, turn.as_ref(), &plan).await?;
            workspace_success(/*changed*/ true, plan.cwd, plan.workspace_roots)
        })
    }
}

impl WorkspaceMutationHandler {
    fn approval_reason(&self, target: &AbsolutePathBuf) -> String {
        match self.operation {
            WorkspaceMutationOperation::SetWorkingDirectory => format!(
                "switch this session's working directory to `{}`",
                target.as_path().display()
            ),
            WorkspaceMutationOperation::AddWorkspaceRoot => {
                format!(
                    "add `{}` to this session's workspace",
                    target.as_path().display()
                )
            }
        }
    }

    fn approval_request(
        &self,
        target: AbsolutePathBuf,
        resulting_workspace_roots: Vec<AbsolutePathBuf>,
    ) -> WorkspaceMutationApprovalRequest {
        WorkspaceMutationApprovalRequest {
            operation: self.operation.clone(),
            target,
            resulting_workspace_roots,
        }
    }

    async fn request_permissions(
        &self,
        session: &Arc<Session>,
        turn: &Arc<TurnContext>,
        request: WorkspacePermissionRequest,
    ) -> Result<RequestPermissionProfile, &'static str> {
        let response = session
            .request_workspace_permissions_for_cwd(
                turn,
                request.call_id,
                RequestPermissionsArgs {
                    environment_id: Some(request.environment_id),
                    reason: Some(self.approval_reason(&request.target)),
                    permissions: request.requested_permissions.clone(),
                },
                request.current_cwd.clone(),
                self.approval_request(request.target, request.resulting_workspace_roots),
                request.cancellation_token,
            )
            .await
            .ok_or("workspace mutation approval was cancelled")?;
        if !matches!(response.scope, PermissionGrantScope::Session)
            || !permissions_are_approved(
                request.requested_permissions,
                response.permissions.clone(),
                request.current_cwd.as_path(),
            )
        {
            return Err(
                "workspace mutation requires session-scoped approval with the requested filesystem access",
            );
        }
        Ok(response.permissions)
    }

    async fn apply_plan(
        &self,
        session: &Arc<Session>,
        turn: &TurnContext,
        plan: &WorkspaceMutationPlan,
    ) -> Result<(), FunctionCallError> {
        session
            .update_runtime_workspace(
                turn,
                matches!(
                    self.operation,
                    WorkspaceMutationOperation::SetWorkingDirectory
                )
                .then_some(plan.cwd.clone()),
                plan.workspace_roots.clone(),
            )
            .await
            .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?;
        session
            .send_event(turn, thread_settings_applied_event(session.as_ref()).await)
            .await;
        Ok(())
    }
}

impl CoreToolRuntime for WorkspaceMutationHandler {
    fn execution_policy(&self) -> ToolExecutionPolicy {
        ToolExecutionPolicy::BarrierAndCancelSuffix
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

fn workspace_roots_with_target(
    workspace_roots: &[AbsolutePathBuf],
    target: &AbsolutePathBuf,
) -> Vec<AbsolutePathBuf> {
    let mut workspace_roots = workspace_roots.to_vec();
    if !workspace_roots
        .iter()
        .any(|root| target.as_path().starts_with(root.as_path()))
    {
        workspace_roots.push(target.clone());
    }
    workspace_roots
}

fn workspace_mutation_plan(
    operation: WorkspaceMutationOperation,
    current_cwd: &AbsolutePathBuf,
    current_workspace_roots: &[AbsolutePathBuf],
    target: AbsolutePathBuf,
) -> WorkspaceMutationPlan {
    let workspace_roots = workspace_roots_with_target(current_workspace_roots, &target);
    let cwd = match operation {
        WorkspaceMutationOperation::SetWorkingDirectory => target,
        WorkspaceMutationOperation::AddWorkspaceRoot => current_cwd.clone(),
    };
    WorkspaceMutationPlan {
        changed: cwd != *current_cwd || workspace_roots != current_workspace_roots,
        cwd,
        workspace_roots,
    }
}

async fn resolve_workspace_directory(
    fs: &dyn ExecutorFileSystem,
    requested: &AbsolutePathBuf,
    sandbox: &FileSystemSandboxContext,
) -> io::Result<AbsolutePathBuf> {
    let requested = PathUri::from_abs_path(requested)?;
    let canonical = fs.canonicalize(&requested, Some(sandbox)).await?;
    let metadata = fs.get_metadata(&canonical, Some(sandbox)).await?;
    let canonical = canonical.to_abs_path()?;
    if metadata.is_directory {
        Ok(canonical)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "workspace mutation target is not a directory: {}",
                canonical.as_path().display()
            ),
        ))
    }
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
        io::ErrorKind::InvalidInput => "not_a_directory",
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

    #[test]
    fn workspace_roots_with_target_adds_only_external_targets() {
        let root = AbsolutePathBuf::from_absolute_path("/workspace").expect("absolute root");
        let external = AbsolutePathBuf::from_absolute_path("/external").expect("absolute target");

        assert_eq!(
            workspace_roots_with_target(std::slice::from_ref(&root), &root.join("src")),
            vec![root.clone()]
        );
        assert_eq!(
            workspace_roots_with_target(std::slice::from_ref(&root), &external),
            vec![root, external]
        );
    }

    #[test]
    fn invalid_workspace_target_maps_to_not_a_directory() {
        assert_eq!(
            io_error_code(&io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a directory"
            )),
            "not_a_directory"
        );
    }
}
