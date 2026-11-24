//! Tracks direct launch permission choices separately from configuration defaults.
//! Omitted choices let app-server restore the destination task's saved settings.

use crate::legacy_core::config::Config;
use crate::legacy_core::config::ConfigOverrides;
use codex_app_server_protocol::SandboxPolicy;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadSettingsUpdateParams;
use codex_config::ConfigLayerSource;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ResumePermissions {
    pub approval_policy: bool,
    pub approvals_reviewer: bool,
    pub profile: bool,
    pub workspace_roots: bool,
    /// Require the server to confirm the initial --approve-for-me preset before accepting input.
    pub confirm_preset: bool,
}

impl ResumePermissions {
    pub(crate) fn from_overrides(config: &Config, overrides: &ConfigOverrides) -> Self {
        let has = |path: &str| {
            config.config_layer_stack.layers_high_to_low().any(|layer| {
                matches!(layer.name, ConfigLayerSource::SessionFlags)
                    && path
                        .split('.')
                        .try_fold(&layer.config, |value, key| value.get(key))
                        .is_some()
            })
        };
        Self {
            confirm_preset: false,
            approval_policy: overrides.approval_policy.is_some() || has("approval_policy"),
            approvals_reviewer: overrides.approvals_reviewer.is_some() || has("approvals_reviewer"),
            profile: overrides.sandbox_mode.is_some()
                || overrides.permission_profile.is_some()
                || overrides.default_permissions.is_some()
                || !overrides.additional_writable_roots.is_empty()
                || has("default_permissions")
                || has("sandbox_mode")
                || has("sandbox_workspace_write")
                || has("permissions")
                || has("network"),
            workspace_roots: overrides.cwd.is_some()
                || overrides.workspace_roots.is_some()
                || !overrides.additional_writable_roots.is_empty()
                || has("sandbox_workspace_write.writable_roots"),
        }
    }

    pub(crate) const PRESET: Self = Self {
        confirm_preset: true,
        ..Self::CURRENT_CONFIG
    };

    pub(crate) const CURRENT_CONFIG: Self = Self {
        approval_policy: true,
        approvals_reviewer: true,
        profile: true,
        workspace_roots: true,
        confirm_preset: false,
    };
}

/// The daemon may have rejoined an already-loaded thread and ignored the resume overrides.
pub(crate) fn initial_resume_update(
    config: &Config,
    response: &ThreadResumeResponse,
) -> color_eyre::Result<Option<ThreadSettingsUpdateParams>> {
    // Workspace Write implicitly grants cwd. A named profile also preserves
    // the thread's runtime roots, so it cannot supply missing ones.
    let profile = config.permissions.active_permission_profile();
    let requested: HashSet<_> = config
        .workspace_roots
        .iter()
        .map(codex_utils_absolute_path::AbsolutePathBuf::as_path)
        .collect();
    let active: HashSet<_> = response
        .runtime_workspace_roots
        .iter()
        .map(codex_utils_path_uri::LegacyAppPathString::as_path)
        .collect();
    if response.cwd.as_path() != config.cwd.as_path()
        || !active.is_subset(&requested)
        || (profile.is_some() && active != requested)
    {
        color_eyre::eyre::bail!("Cannot apply --approve-for-me to a different workspace.");
    }
    let approval_policy = config.permissions.approval_policy.value().into();
    let approvals_reviewer = config.approvals_reviewer.into();
    let mut settings = ThreadSettingsUpdateParams {
        thread_id: response.thread.id.clone(),
        approval_policy: Some(approval_policy),
        approvals_reviewer: Some(approvals_reviewer),
        ..Default::default()
    };
    let sandbox: SandboxPolicy = config
        .permissions
        .effective_permission_profile()
        .to_legacy_sandbox_policy(config.cwd.as_path())?
        .into();
    let sandbox_changed = sorted_roots(response.sandbox.clone()) != sorted_roots(sandbox.clone());
    if let Some(profile) = profile {
        if sandbox_changed
            || response
                .active_permission_profile
                .as_ref()
                .is_none_or(|active| active.id != profile.id)
        {
            settings.permissions = Some(profile.id);
        }
    } else if sandbox_changed {
        settings.sandbox_policy = Some(sandbox);
    }
    if response.approval_policy == approval_policy
        && response.approvals_reviewer == approvals_reviewer
        && settings.permissions.is_none()
        && settings.sandbox_policy.is_none()
    {
        return Ok(None);
    }
    Ok(Some(settings))
}

fn sorted_roots(mut policy: SandboxPolicy) -> SandboxPolicy {
    if let SandboxPolicy::WorkspaceWrite { writable_roots, .. } = &mut policy {
        writable_roots.sort_unstable_by(|a, b| a.as_str().cmp(b.as_str()));
    }
    policy
}

#[cfg(test)]
#[path = "resume_permissions_tests.rs"]
mod tests;
