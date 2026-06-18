use super::Config;
use super::merge_managed_permission_profiles;
use super::permissions::BUILT_IN_DANGER_FULL_ACCESS_PROFILE;
use super::permissions::BUILT_IN_READ_ONLY_PROFILE;
use super::permissions::BUILT_IN_WORKSPACE_PROFILE;
use super::permissions::builtin_permission_profile;
use super::permissions::compile_permission_profile_selection;
use codex_config::config_toml::ConfigToml;
use codex_protocol::models::PermissionProfile;
use std::io::ErrorKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectablePermissionProfile {
    pub id: String,
    pub description: Option<String>,
}

impl Config {
    /// Returns built-in and configured permission profiles allowed by current requirements.
    pub fn selectable_permission_profiles(
        &self,
    ) -> std::io::Result<Vec<SelectablePermissionProfile>> {
        let config_toml: ConfigToml = self
            .config_layer_stack
            .effective_config()
            .try_into()
            .map_err(|err| std::io::Error::new(ErrorKind::InvalidData, err))?;
        let requirements_toml = self.config_layer_stack.requirements_toml();
        let permissions =
            merge_managed_permission_profiles(config_toml.permissions.as_ref(), requirements_toml)?;
        let mut profiles = Vec::new();

        for id in [
            BUILT_IN_READ_ONLY_PROFILE,
            BUILT_IN_WORKSPACE_PROFILE,
            BUILT_IN_DANGER_FULL_ACCESS_PROFILE,
        ] {
            if let Some(profile) = self.selectable_permission_profile(
                permissions.as_ref(),
                id,
                /*description*/ None,
            ) {
                profiles.push(profile);
            }
        }
        if let Some(permissions) = permissions.as_ref() {
            for (id, profile) in &permissions.entries {
                if let Some(profile) = self.selectable_permission_profile(
                    Some(permissions),
                    id,
                    profile.description.clone(),
                ) {
                    profiles.push(profile);
                }
            }
        }

        Ok(profiles)
    }

    fn selectable_permission_profile(
        &self,
        permissions: Option<&codex_config::permissions_toml::PermissionsToml>,
        id: &str,
        description: Option<String>,
    ) -> Option<SelectablePermissionProfile> {
        if self
            .config_layer_stack
            .requirements_toml()
            .allowed_permission_profiles
            .as_ref()
            .is_some_and(|allowed| !allowed.get(id).copied().unwrap_or(false))
        {
            return None;
        }
        let permission_profile = match builtin_permission_profile(id, /*workspace_write*/ None) {
            Some(permission_profile) => permission_profile,
            None => {
                let mut startup_warnings = Vec::new();
                let (file_system, network) = compile_permission_profile_selection(
                    permissions,
                    id,
                    /*workspace_write*/ None,
                    self.cwd.as_path(),
                    &mut startup_warnings,
                )
                .ok()?;
                PermissionProfile::from_runtime_permissions(&file_system, network)
            }
        };
        self.permissions
            .can_set_permission_profile(&permission_profile)
            .ok()?;
        self.config_layer_stack
            .requirements()
            .permission_profile
            .clone()
            .set(permission_profile)
            .ok()?;

        Some(SelectablePermissionProfile {
            id: id.to_string(),
            description,
        })
    }
}
