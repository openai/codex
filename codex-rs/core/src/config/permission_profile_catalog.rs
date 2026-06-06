use std::path::Path;

use codex_config::ConfigLayerStack;
use codex_config::RequirementSource;
use codex_config::SandboxModeRequirement;
use codex_config::Sourced;
use codex_config::permissions_toml::PermissionsToml;
use codex_config::sandbox_mode_requirement_for_permission_profile;
use codex_protocol::models::PermissionProfile;

use super::ConstraintError;
use super::ConstraintResult;
use super::merge_managed_permission_profiles;
use super::permissions::BUILT_IN_DANGER_FULL_ACCESS_PROFILE;
use super::permissions::BUILT_IN_READ_ONLY_PROFILE;
use super::permissions::BUILT_IN_WORKSPACE_PROFILE;
use super::permissions::compile_permission_profile_selection;
use super::permissions::is_builtin_permission_profile_name;
use super::permissions::validate_user_permission_profile_names;
use super::validate_required_permission_profile_catalog;

/// A permission profile exposed to clients together with its effective availability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionProfileCatalogEntry {
    pub id: String,
    pub description: Option<String>,
    pub allowed: bool,
}

/// Builds the effective permission profile catalog for a config layer stack.
pub fn permission_profile_catalog(
    config_layer_stack: &ConfigLayerStack,
    policy_cwd: &Path,
) -> std::io::Result<Vec<PermissionProfileCatalogEntry>> {
    let permissions = config_layer_stack
        .effective_config()
        .get("permissions")
        .cloned()
        .map(toml::Value::try_into::<PermissionsToml>)
        .transpose()
        .map_err(std::io::Error::other)?;
    let requirements_toml = config_layer_stack.requirements_toml();
    let permissions = merge_managed_permission_profiles(permissions.as_ref(), requirements_toml)?;
    validate_user_permission_profile_names(permissions.as_ref())?;
    validate_required_permission_profile_catalog(requirements_toml, permissions.as_ref())?;

    let mut catalog = [
        (BUILT_IN_READ_ONLY_PROFILE, PermissionProfile::read_only()),
        (
            BUILT_IN_WORKSPACE_PROFILE,
            PermissionProfile::workspace_write(),
        ),
        (
            BUILT_IN_DANGER_FULL_ACCESS_PROFILE,
            PermissionProfile::Disabled,
        ),
    ]
    .into_iter()
    .map(|(id, permission_profile)| PermissionProfileCatalogEntry {
        id: id.to_string(),
        description: None,
        allowed: permission_profile_is_allowed(config_layer_stack, id, &permission_profile),
    })
    .collect::<Vec<_>>();

    if let Some(permissions) = permissions.as_ref() {
        catalog.extend(permissions.entries.iter().map(|(id, profile)| {
            let mut warnings = Vec::new();
            let allowed = compile_permission_profile_selection(
                Some(permissions),
                id,
                /*workspace_write*/ None,
                policy_cwd,
                &mut warnings,
            )
            .map(|(file_system, network)| {
                PermissionProfile::from_runtime_permissions(&file_system, network)
            })
            .is_ok_and(|permission_profile| {
                permission_profile_is_allowed(config_layer_stack, id, &permission_profile)
            });
            PermissionProfileCatalogEntry {
                id: id.clone(),
                description: profile.description.clone(),
                allowed,
            }
        }));
    }

    Ok(catalog)
}

pub(super) fn permission_profile_is_allowed(
    config_layer_stack: &ConfigLayerStack,
    profile_id: &str,
    permission_profile: &PermissionProfile,
) -> bool {
    let allowed_by_id = is_builtin_permission_profile_name(profile_id)
        || config_layer_stack
            .requirements_toml()
            .allowed_permissions
            .as_ref()
            .is_none_or(|allowed| allowed.iter().any(|allowed_id| allowed_id == profile_id));
    let allowed_by_sandbox_mode = config_layer_stack
        .requirements()
        .permission_profile
        .can_set(permission_profile)
        .is_ok();
    let allowed_by_filesystem = config_layer_stack
        .requirements()
        .filesystem
        .as_ref()
        .is_none_or(|Sourced { value, source }| {
            value.deny_read.is_empty()
                || validate_permission_profile_for_deny_read(permission_profile, source).is_ok()
        });
    allowed_by_id && allowed_by_sandbox_mode && allowed_by_filesystem
}

pub(super) fn validate_permission_profile_for_deny_read(
    permission_profile: &PermissionProfile,
    requirement_source: &RequirementSource,
) -> ConstraintResult<()> {
    let mode = sandbox_mode_requirement_for_permission_profile(permission_profile);
    match mode {
        SandboxModeRequirement::ReadOnly | SandboxModeRequirement::WorkspaceWrite => Ok(()),
        SandboxModeRequirement::DangerFullAccess | SandboxModeRequirement::ExternalSandbox => {
            Err(ConstraintError::InvalidValue {
                field_name: "sandbox_mode",
                candidate: format!("{mode:?}"),
                allowed: "[read-only, workspace-write]".to_string(),
                requirement_source: requirement_source.clone(),
            })
        }
    }
}
