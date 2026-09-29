//! Tracks direct launch permission choices separately from configuration defaults.
//! Omitted choices let app-server restore the destination task's saved settings.

use crate::legacy_core::config::Config;
use crate::legacy_core::config::ConfigOverrides;
use codex_config::ConfigLayerSource;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ResumePermissions {
    pub approval_policy: bool,
    pub approvals_reviewer: bool,
    pub profile: bool,
    pub workspace_roots: bool,
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

    #[cfg(test)]
    pub(crate) const CURRENT_CONFIG: Self = Self {
        approval_policy: true,
        approvals_reviewer: true,
        profile: true,
        workspace_roots: true,
    };
}

#[cfg(test)]
#[path = "resume_permissions_tests.rs"]
mod tests;
