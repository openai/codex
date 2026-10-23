//! Resolves Guardian permission evidence from the host's captured environment context.
//! Sync review and async scoring use the same rules and environment identity.

use codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID;

use super::GuardianReviewContext;
use crate::context::GuardianPermissionContext;
use crate::session::turn_context::TurnEnvironment;
use crate::tools::context::ToolEnvironments;
use crate::tools::context::ToolInvocation;

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
    let environment_id =
        environment.map(|environment| environment.selection.environment_id.clone());
    let native_cwd = environment
        .filter(|environment| !environment.environment.is_remote())
        .and_then(|environment| environment.cwd().to_abs_path().ok());
    let permission_profile = environment
        .map(TurnEnvironment::permission_profile_with_workspace_roots)
        .unwrap_or_else(|| turn.permission_profile_for_environments(context.environments()));
    let file_system_policy = permission_profile.file_system_sandbox_policy();
    // Remote restrictions must not be interpreted using the filesystem running Guardian.
    // Older executors may not report their temp folders. If a rule explicitly denies those
    // folders, decline automatic approval rather than guess. Default rules do not deny them.
    if let Some(environment) = environment
        && native_cwd.is_none()
    {
        let sandbox = environment.sandbox_context(/*additional_permissions*/ None);
        let paths = sandbox.policy_context();
        let mut denied_globs = file_system_policy
            .get_unreadable_globs_with_context(&paths)
            .map_err(anyhow::Error::msg)?;
        denied_globs.sort();
        denied_globs.dedup();
        return Ok(GuardianPermissionContext {
            environment_id,
            denied_paths: file_system_policy
                .get_unreadable_roots_with_context(&paths)
                .map_err(anyhow::Error::msg)?
                .into_iter()
                .map(|path| path.inferred_native_path_string())
                .collect(),
            denied_globs,
        });
    }
    #[allow(deprecated)]
    let cwd = native_cwd.unwrap_or_else(|| turn.cwd.clone());
    Ok(GuardianPermissionContext {
        environment_id,
        denied_paths: file_system_policy
            .get_unreadable_roots_with_cwd(&cwd)
            .into_iter()
            .map(|root| root.to_string_lossy().into_owned())
            .collect(),
        denied_globs: file_system_policy.get_unreadable_globs_with_cwd(&cwd),
    })
}
