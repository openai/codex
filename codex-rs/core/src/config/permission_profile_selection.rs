use codex_config::ConfigRequirementsToml;
use codex_config::permissions_toml::PermissionsToml;

/// A selected permission profile and its uncompiled configured and managed catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPermissionProfileSelection<'a> {
    /// The effective profile selected by configuration and managed requirements.
    pub profile_id: Option<&'a str>,
    /// Configured and managed named profiles, without platform-specific compilation.
    pub profiles: Option<PermissionsToml>,
}

/// Resolves the effective permission profile without interpreting profile paths.
///
/// Managed profiles and allowlists are applied using Core's normal selection rules,
/// while the merged profile catalog remains uncompiled for the executor to interpret.
pub fn resolve_permission_profile_selection<'a>(
    configured_profiles: Option<&PermissionsToml>,
    configured_default_profile_id: Option<&'a str>,
    requirements: &'a ConfigRequirementsToml,
) -> std::io::Result<ResolvedPermissionProfileSelection<'a>> {
    let selection = super::resolve_effective_permission_selection(
        configured_profiles,
        /*default_permissions_override*/ None,
        /*persisted_profile_id*/ None,
        configured_default_profile_id,
        requirements,
        &mut Vec::new(),
    )?;

    Ok(ResolvedPermissionProfileSelection {
        profile_id: selection.selected_profile_id,
        profiles: selection.profiles,
    })
}

#[cfg(test)]
#[path = "permission_profile_selection_tests.rs"]
mod tests;
