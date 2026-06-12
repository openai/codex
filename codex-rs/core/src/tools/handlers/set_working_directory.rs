use crate::function_tool::FunctionCallError;
use crate::session::session::RuntimeWorkspaceSnapshot;
use crate::session::session::Session;
use crate::session::session::SessionSettingsUpdate;
use crate::session::turn_context::TurnContext;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::parse_arguments;
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
use codex_sandboxing::policy_transforms::intersect_permission_profiles;
use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub(crate) struct SetWorkingDirectoryHandler;

#[derive(Deserialize)]
struct SetWorkingDirectoryArgs {
    path: String,
}

impl ToolExecutor<ToolInvocation> for SetWorkingDirectoryHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("set_working_directory")
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec::Function(ResponsesApiTool {
            name: "set_working_directory".to_string(),
            description: "Change this session's working directory. Relative paths resolve from the current working directory. Later tool calls in the same response wait for this change and are cancelled if it fails.".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(
                BTreeMap::from([(
                    "path".to_string(),
                    JsonSchema::string(
                        /*description*/ Some("Existing directory path.".to_string()),
                    ),
                )]),
                /*required*/ Some(vec!["path".to_string()]),
                /*additional_properties*/ Some(false.into()),
            ),
            output_schema: None,
        })
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(self.handle_call(invocation))
    }
}

impl SetWorkingDirectoryHandler {
    async fn handle_call(
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
                    "set_working_directory received unsupported payload".to_string(),
                ));
            }
        };
        let args: SetWorkingDirectoryArgs = parse_arguments(&arguments)?;
        let current = session.runtime_workspace_snapshot().await;
        let requested = current.cwd.join(args.path);
        let [environment] = turn.environments.turn_environments.as_slice() else {
            return Err(FunctionCallError::RespondToModel(
                "set_working_directory requires exactly one execution environment".to_string(),
            ));
        };
        let fs = environment.environment.get_filesystem();
        let preview = preview_cwd(&session, environment.selection(), requested.clone()).await?;
        let inspection_permissions = match required_permissions(&current, &preview) {
            Some(requested_permissions) => Some(
                request_session_permissions(
                    &session,
                    &turn,
                    call_id,
                    requested.clone(),
                    requested_permissions,
                    &current,
                    cancellation_token,
                )
                .await?,
            ),
            None => None,
        };
        let sandbox_cwd = PathUri::from_abs_path(&current.cwd);
        let sandbox = turn.file_system_sandbox_context_for_permission_profile(
            &current.permission_profile,
            inspection_permissions.clone().map(Into::into),
            &sandbox_cwd,
        );
        let canonical = resolve_directory(fs.as_ref(), &requested, &sandbox)
            .await
            .map_err(|err| {
                FunctionCallError::RespondToModel(format!("cannot change working directory: {err}"))
            })?;

        if canonical == current.cwd {
            return set_working_directory_success(canonical);
        }

        let preview = preview_cwd(&session, environment.selection(), canonical.clone()).await?;
        if !preview
            .permission_profile
            .file_system_sandbox_policy()
            .can_read_path_with_cwd(canonical.as_path(), canonical.as_path())
        {
            return Err(FunctionCallError::RespondToModel(
                "the requested directory is unavailable under the active permission profile"
                    .to_string(),
            ));
        }
        if required_permissions(&current, &preview).is_some_and(|requested_permissions| {
            !inspection_permissions.as_ref().is_some_and(|granted| {
                permissions_are_approved(
                    requested_permissions,
                    granted.clone(),
                    current.cwd.as_path(),
                )
            })
        }) {
            return Err(FunctionCallError::RespondToModel(
                "the canonical directory requires filesystem access outside the approved path"
                    .to_string(),
            ));
        }

        session
            .update_runtime_cwd(turn.as_ref(), canonical.clone())
            .await
            .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?;
        set_working_directory_success(canonical)
    }
}

impl CoreToolRuntime for SetWorkingDirectoryHandler {
    fn execution_policy(&self) -> ToolExecutionPolicy {
        ToolExecutionPolicy::BarrierAndCancelSuffix
    }
}

async fn preview_cwd(
    session: &Session,
    environment: codex_protocol::protocol::TurnEnvironmentSelection,
    cwd: AbsolutePathBuf,
) -> Result<crate::codex_thread::ThreadConfigSnapshot, FunctionCallError> {
    session
        .preview_settings(&SessionSettingsUpdate {
            environments: Some(TurnEnvironmentSelections::new(cwd, vec![environment])),
            ..Default::default()
        })
        .await
        .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))
}

async fn request_session_permissions(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    call_id: String,
    target: AbsolutePathBuf,
    requested_permissions: RequestPermissionProfile,
    current: &RuntimeWorkspaceSnapshot,
    cancellation_token: CancellationToken,
) -> Result<RequestPermissionProfile, FunctionCallError> {
    let response = session
        .request_session_permissions_for_cwd(
            turn,
            call_id,
            RequestPermissionsArgs {
                environment_id: None,
                reason: Some(format!(
                    "switch this session's working directory to `{}`",
                    target.as_path().display()
                )),
                permissions: requested_permissions.clone(),
            },
            current.cwd.clone(),
            cancellation_token,
        )
        .await
        .ok_or_else(|| {
            FunctionCallError::RespondToModel(
                "working directory approval was cancelled".to_string(),
            )
        })?;
    if !matches!(response.scope, PermissionGrantScope::Session)
        || !permissions_are_approved(
            requested_permissions,
            response.permissions.clone(),
            current.cwd.as_path(),
        )
    {
        return Err(FunctionCallError::RespondToModel(
            "changing the working directory requires session-scoped filesystem approval"
                .to_string(),
        ));
    }
    Ok(response.permissions)
}

fn required_permissions(
    current: &RuntimeWorkspaceSnapshot,
    preview: &crate::codex_thread::ThreadConfigSnapshot,
) -> Option<RequestPermissionProfile> {
    newly_accessible_roots(
        &current.permission_profile.file_system_sandbox_policy(),
        current.cwd.as_path(),
        &preview.permission_profile.file_system_sandbox_policy(),
        preview.cwd().as_path(),
    )
    .map(|file_system| RequestPermissionProfile {
        file_system: Some(file_system),
        network: None,
    })
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

async fn resolve_directory(
    fs: &dyn ExecutorFileSystem,
    requested: &AbsolutePathBuf,
    sandbox: &FileSystemSandboxContext,
) -> io::Result<AbsolutePathBuf> {
    let requested = PathUri::from_abs_path(requested);
    let canonical = fs.canonicalize(&requested, Some(sandbox)).await?;
    let metadata = fs.get_metadata(&canonical, Some(sandbox)).await?;
    let canonical = canonical.to_abs_path()?;
    if metadata.is_directory {
        Ok(canonical)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "working directory target is not a directory: {}",
                canonical.as_path().display()
            ),
        ))
    }
}

fn set_working_directory_success(
    cwd: AbsolutePathBuf,
) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
    let content = serde_json::json!({ "cwd": cwd }).to_string();
    Ok(boxed_tool_output(FunctionToolOutput::from_text(
        content,
        /*success*/ Some(true),
    )))
}
