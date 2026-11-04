//! Guardian reviews and score reuse use the prepared MCP owner's rules when available.
//! Early scoring uses the issuing context, before the MCP call is prepared.

use codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID;
use codex_file_system::FileSystemSandboxContext;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemSandboxPolicyContext;
use codex_utils_absolute_path::AbsolutePathBuf;

use super::GuardianApprovalRequest;
use super::GuardianReviewContext;
use crate::context::GuardianPermissionContext;
use crate::session::turn_context::TurnEnvironment;
use crate::tools::context::ToolEnvironments;
use crate::tools::context::ToolInvocation;

// Bound both JSON and permission evidence without restricting manual approvals.
const MAX_GUARDIAN_ENVIRONMENT_ID_BYTES: usize = 256;

pub(super) fn check_environment_id(id: Option<&str>) -> anyhow::Result<()> {
    if id.is_some_and(|id| id.len() > MAX_GUARDIAN_ENVIRONMENT_ID_BYTES) {
        anyhow::bail!(
            "approval environment id exceeds Guardian's {MAX_GUARDIAN_ENVIRONMENT_ID_BYTES}-byte limit"
        );
    }
    Ok(())
}

pub(super) fn for_request(
    request: &GuardianApprovalRequest,
    context: Option<&GuardianReviewContext>,
) -> anyhow::Result<Option<GuardianPermissionContext>> {
    match request {
        GuardianApprovalRequest::McpToolCall {
            prompt_permissions: Some((environment_id, sandbox)),
            ..
        } => for_mcp_environment(environment_id, sandbox).map(Some),
        _ => context
            .map(|context| for_environment(context, request.target_environment_id()))
            .transpose(),
    }
}

pub(super) fn for_mcp_environment(
    environment_id: &str,
    sandbox: &FileSystemSandboxContext,
) -> anyhow::Result<GuardianPermissionContext> {
    check_environment_id(Some(environment_id))?;
    if environment_id == DEFAULT_MCP_SERVER_ENVIRONMENT_ID
        && let Ok(cwd) = sandbox.cwd.to_abs_path()
    {
        return Ok(host_permissions(
            Some(environment_id.to_owned()),
            &sandbox.permissions,
            &cwd,
        ));
    }
    executor_permissions(
        environment_id,
        &sandbox.permissions,
        sandbox.policy_context(),
    )
}

pub(crate) fn for_tool(invocation: &ToolInvocation) -> anyhow::Result<GuardianPermissionContext> {
    let context = GuardianReviewContext::from(&invocation.step_context);
    let environment_id = if matches!(invocation.environments, ToolEnvironments::Mcp { .. })
        && invocation.environments.single_id() == Some(DEFAULT_MCP_SERVER_ENVIRONMENT_ID)
        && context.environments().local_selection().is_none()
    {
        // Local execution tools can only resolve to an environment present in the step.
        // Host MCP tools and resources can also serve remote-only turns: their hooks use
        // the server owner, while Guardian needs the turn's primary permissions.
        None
    } else {
        invocation.environments.single_id()
    };
    for_environment(&context, environment_id)
}

pub(super) fn for_environment(
    context: &GuardianReviewContext,
    environment_id: Option<&str>,
) -> anyhow::Result<GuardianPermissionContext> {
    let turn = context.turn();
    let environment = match environment_id {
        Some(id) => Some(
            context
                .environments()
                .turn_environments()
                .find(|environment| environment.selection.environment_id == id)
                .ok_or_else(|| anyhow::anyhow!("approval environment {id} is unavailable"))?,
        ),
        None => context.environments().primary(),
    };
    let native_cwd = environment
        .filter(|environment| !environment.environment().is_remote())
        .and_then(|environment| environment.cwd().to_abs_path().ok());
    let permission_profile = environment
        .map(TurnEnvironment::permission_profile_with_workspace_roots)
        .unwrap_or_else(|| turn.permission_profile_for_environments(context.environments()));
    if let Some(environment) = environment
        && native_cwd.is_none()
    {
        return executor_permissions(
            &environment.selection.environment_id,
            &permission_profile,
            environment
                .sandbox_context(/*additional_permissions*/ None)
                .policy_context(),
        );
    }
    #[allow(deprecated)]
    let cwd = native_cwd.as_ref().unwrap_or(&turn.cwd);
    Ok(host_permissions(
        environment.map(|env| env.selection.environment_id.clone()),
        &permission_profile,
        cwd,
    ))
}

fn executor_permissions(
    environment_id: &str,
    profile: &PermissionProfile,
    paths: FileSystemSandboxPolicyContext<'_>,
) -> anyhow::Result<GuardianPermissionContext> {
    // Remote restrictions must not be interpreted using the filesystem running Guardian.
    // Older executors may not report their temp folders. If a rule explicitly denies those
    // folders, decline automatic approval rather than guess. Default rules do not deny them.
    let file_system_policy = profile.file_system_sandbox_policy();
    let mut denied_globs = file_system_policy
        .get_unreadable_globs_with_context(&paths)
        .map_err(anyhow::Error::msg)?;
    denied_globs.sort();
    denied_globs.dedup();
    Ok(GuardianPermissionContext {
        environment_id: Some(environment_id.to_owned()),
        denied_paths: file_system_policy
            .get_unreadable_roots_with_context(&paths)
            .map_err(anyhow::Error::msg)?
            .into_iter()
            .map(|path| path.inferred_native_path_string())
            .collect(),
        denied_globs,
    })
}

fn host_permissions(
    environment_id: Option<String>,
    profile: &PermissionProfile,
    cwd: &AbsolutePathBuf,
) -> GuardianPermissionContext {
    let file_system_policy = profile.file_system_sandbox_policy();
    GuardianPermissionContext {
        environment_id,
        denied_paths: file_system_policy
            .get_unreadable_roots_with_cwd(cwd)
            .into_iter()
            .map(|root| root.to_string_lossy().into_owned())
            .collect(),
        denied_globs: file_system_policy.get_unreadable_globs_with_cwd(cwd),
    }
}
