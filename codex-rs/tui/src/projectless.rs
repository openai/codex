//! Select desktop-like execution defaults for positively discovered local projectless folders.
//! Saved trust, explicit choices, managed policy, and remote execution retain their own defaults.

use crate::app_server_session::AppServerSession;
use crate::legacy_core::config::Config;
use crate::legacy_core::config::ConfigOverrides;
use codex_exec_server::EnvironmentManager;
use codex_protocol::models::ActivePermissionProfile;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_WORKSPACE;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::PermissionProfileSnapshot;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GranularApprovalConfig;

/// Returns whether eligible implicit permissions require Windows sandbox setup.
pub(crate) fn apply_defaults(
    config: &mut Config,
    overrides: &ConfigOverrides,
    app_server: &AppServerSession,
    environments: &EnvironmentManager,
    response: &crate::config_update::EffectiveConfig,
) -> bool {
    let defaults = &response.config;
    let effective = config.config_layer_stack.effective_config();
    if response.layers.as_ref().is_none_or(|layers| layers.iter().any(|layer| {
        layer["name"]["type"] == "project"
    }))
        || app_server.uses_remote_workspace()
        || defaults.sandbox_mode.is_some()
        || defaults.sandbox_workspace_write.is_some()
        || ["default_permissions", "permissions", "network"].iter()
            .any(|key| defaults.additional.get(*key).is_some_and(|value| !value.is_null()))
        // Reuse local discovery only when server markers and trust decisions agree.
        || ["project_root_markers", "projects"].iter().any(|key|
            serde_json::to_value(effective.get(*key)).ok().as_ref()
                != Some(defaults.additional.get(*key).unwrap_or(&serde_json::Value::Null)))
        || !config.config_layer_stack.is_projectless()
        || config.active_project.trust_level.is_some()
        || config.workspace_roots.len() != 1
        || config.workspace_roots.first() != Some(&config.cwd)
        || !has_only_local_environments(environments)
        || overrides.sandbox_mode.is_some()
        || overrides.permission_profile.is_some()
        || overrides.default_permissions.is_some()
        || [
            "sandbox_mode",
            "default_permissions",
            "permissions",
            "sandbox_workspace_write",
            "network",
        ]
        .iter()
        .any(|key| effective.get(*key).is_some())
        || config
            .config_layer_stack
            .requirements_toml()
            .default_permissions
            .is_some()
        || !config.is_permission_profile_allowed(
            BUILT_IN_PERMISSION_PROFILE_WORKSPACE,
            &PermissionProfile::workspace_write(),
        )
    {
        return false;
    }
    let granular = AskForApproval::Granular(GranularApprovalConfig {
        sandbox_approval: false,
        rules: false,
        skill_approval: false,
        request_permissions: true,
        mcp_elicitations: true,
    });
    if overrides.approval_policy.is_none() && effective.get("approval_policy").is_none() {
        let granular = defaults
            .approval_policy
            .as_ref()
            .map_or(granular, |policy| policy.to_core());
        let _ = config.permissions.approval_policy.set(granular);
    }
    #[cfg(target_os = "windows")]
    if config.effective_local_windows_sandbox_type() == codex_sandboxing::SandboxType::None {
        return true;
    }
    if config
        .permissions
        .set_permission_profile(PermissionProfile::workspace_write())
        .is_err()
    {
        return false;
    }
    let snapshot = PermissionProfileSnapshot::active(
        config.permissions.permission_profile().clone(),
        ActivePermissionProfile::new(BUILT_IN_PERMISSION_PROFILE_WORKSPACE.to_string()),
    );
    let _ = config
        .permissions
        .set_permission_profile_from_session_snapshot(snapshot);
    false
}

/// Local filesystem discovery is authoritative only for positively local execution.
pub(crate) fn has_only_local_environments(environments: &EnvironmentManager) -> bool {
    let ids = environments.default_environment_ids();
    !ids.is_empty()
        && ids.iter().all(|id| {
            environments
                .get_environment(id)
                .is_some_and(|environment| !environment.is_remote())
        })
}
