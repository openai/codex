use super::Config;
use super::merge_managed_permission_profiles;
use super::permissions::BUILT_IN_DANGER_FULL_ACCESS_PROFILE;
use super::permissions::BUILT_IN_READ_ONLY_PROFILE;
use super::permissions::BUILT_IN_WORKSPACE_PROFILE;
use super::permissions::builtin_permission_profile;
use super::permissions::compile_permission_profile_selection;
use crate::windows_sandbox::WindowsSandboxLevelExt;
use codex_config::config_toml::ConfigToml;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use std::io::ErrorKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectablePermissionProfile {
    pub id: String,
    pub description: Option<String>,
    pub is_default: bool,
}

impl Config {
    /// Returns named permission profiles that can be selected under the current requirements.
    pub async fn selectable_permission_profiles(
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
        let active_profile_id = self
            .permissions
            .active_permission_profile()
            .map(|profile| profile.id);
        let requirements = self.config_layer_stack.requirements();
        let default_permission_profile = if active_profile_id.is_none() {
            let mut permission_profile = requirements.permission_profile.clone();
            let configured_profile = config_toml
                .derive_permission_profile(
                    /*sandbox_mode_override*/ None,
                    WindowsSandboxLevel::from_config(self),
                    Some(&self.active_project),
                    Some(&requirements.permission_profile),
                )
                .await;
            let _ = permission_profile.set(configured_profile);
            Some(permission_profile.get().clone())
        } else {
            None
        };
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
                active_profile_id.as_deref(),
                default_permission_profile.as_ref(),
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
                    active_profile_id.as_deref(),
                    default_permission_profile.as_ref(),
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
        active_profile_id: Option<&str>,
        default_permission_profile: Option<&PermissionProfile>,
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
        let mut constrained_permission_profile = self
            .config_layer_stack
            .requirements()
            .permission_profile
            .clone();
        constrained_permission_profile
            .set(permission_profile)
            .ok()?;

        Some(SelectablePermissionProfile {
            id: id.to_string(),
            description,
            is_default: active_profile_id == Some(id)
                || active_profile_id.is_none()
                    && default_permission_profile == Some(constrained_permission_profile.get()),
        })
    }
}
