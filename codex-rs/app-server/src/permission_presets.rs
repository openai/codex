use crate::config_manager::ConfigManager;
use codex_app_server_protocol::PermissionPreset;
use codex_app_server_protocol::PermissionPresetDefaultSource;
use codex_app_server_protocol::PermissionPresetKind;
use codex_app_server_protocol::PermissionPresetUnavailabilityReason;
use codex_app_server_protocol::SandboxPolicy;
use codex_config::config_toml::ConfigToml;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::permission_profile_catalog;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_READ_ONLY;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_WORKSPACE;
use codex_protocol::models::ManagedFileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_sandboxing::compatibility_sandbox_policy_for_permission_profile;
use codex_utils_approval_presets::builtin_approval_presets;
use std::io::ErrorKind;
use std::path::PathBuf;

pub(crate) struct PermissionPresetCatalog {
    entries: Vec<PermissionPresetEntry>,
    pub(crate) default_preset_id: String,
    pub(crate) default_source: PermissionPresetDefaultSource,
}

pub(crate) struct PermissionPresetSelection {
    pub(crate) id: String,
    pub(crate) approval_policy: AskForApproval,
    pub(crate) approvals_reviewer: ApprovalsReviewer,
    pub(crate) permission_profile_id: Option<String>,
    pub(crate) permission_profile: PermissionProfile,
}

impl PermissionPresetSelection {
    pub(crate) fn apply_to(self, overrides: &mut ConfigOverrides) {
        overrides.approval_policy = Some(self.approval_policy);
        overrides.approvals_reviewer = Some(self.approvals_reviewer);
        overrides.active_permission_preset_id = Some(self.id);
        match self.permission_profile_id {
            Some(permission_profile_id) => {
                overrides.default_permissions = Some(permission_profile_id);
            }
            None => {
                overrides.permission_profile = Some(self.permission_profile);
            }
        }
    }
}

struct PermissionPresetEntry {
    preset: PermissionPreset,
    permission_profile: PermissionProfile,
}

impl PermissionPresetCatalog {
    pub(crate) fn presets(&self) -> impl Iterator<Item = &PermissionPreset> {
        self.entries.iter().map(|entry| &entry.preset)
    }

    pub(crate) fn selection(&self, id: &str) -> std::io::Result<PermissionPresetSelection> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.preset.id == id)
            .ok_or_else(|| {
                std::io::Error::new(
                    ErrorKind::InvalidInput,
                    format!("unknown permission preset `{id}`"),
                )
            })?;
        if !entry.preset.allowed {
            return Err(std::io::Error::new(
                ErrorKind::PermissionDenied,
                format!("permission preset `{id}` is not allowed"),
            ));
        }

        Ok(Self::selection_from_entry(entry))
    }

    pub(crate) fn selection_for_session(
        &self,
        active_preset_id: Option<&str>,
        active_profile_id: Option<&str>,
        approval_policy: AskForApproval,
        approvals_reviewer: ApprovalsReviewer,
        permission_profile: &PermissionProfile,
    ) -> Option<PermissionPresetSelection> {
        if let Some(active_preset_id) = active_preset_id {
            return self.selection(active_preset_id).ok();
        }

        let mut matches = self.entries.iter().filter(|entry| {
            entry.preset.allowed
                && entry.preset.approval_policy.to_core() == approval_policy
                && entry.preset.approvals_reviewer.to_core() == approvals_reviewer
                && match active_profile_id {
                    Some(active_profile_id) => {
                        entry.preset.permission_profile_id.as_deref() == Some(active_profile_id)
                    }
                    None => {
                        permission_profiles_match(&entry.permission_profile, permission_profile)
                    }
                }
        });
        let entry = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        Some(Self::selection_from_entry(entry))
    }

    fn selection_from_entry(entry: &PermissionPresetEntry) -> PermissionPresetSelection {
        PermissionPresetSelection {
            id: entry.preset.id.clone(),
            approval_policy: entry.preset.approval_policy.to_core(),
            approvals_reviewer: entry.preset.approvals_reviewer.to_core(),
            permission_profile_id: entry.preset.permission_profile_id.clone(),
            permission_profile: entry.permission_profile.clone(),
        }
    }
}

pub(crate) async fn permission_preset_catalog(
    config_manager: &ConfigManager,
    cwd: Option<PathBuf>,
) -> std::io::Result<PermissionPresetCatalog> {
    let config = config_manager
        .load_for_cwd(
            /* request_overrides */ None,
            ConfigOverrides::default(),
            cwd,
        )
        .await?;
    permission_preset_catalog_for_config(&config)
}

fn permission_preset_catalog_for_config(
    config: &Config,
) -> std::io::Result<PermissionPresetCatalog> {
    let profile_catalog =
        permission_profile_catalog(&config.config_layer_stack, config.cwd.as_path())?;
    let config_toml: ConfigToml = config
        .config_layer_stack
        .effective_config()
        .try_into()
        .map_err(std::io::Error::other)?;
    let requirements = config.config_layer_stack.requirements();
    let requirements_override_config = config.startup_warnings.iter().any(|warning| {
        [
            "approval_policy",
            "approvals_reviewer",
            "permission_profile",
        ]
        .iter()
        .any(|field| {
            warning.contains(&format!(
                "Configured value for `{field}` is disallowed by requirements"
            ))
        })
    });
    let configured_default_is_active =
        config_toml
            .default_permissions
            .as_deref()
            .is_some_and(|configured_default| {
                config
                    .permissions
                    .active_permission_profile()
                    .is_some_and(|active| active.id == configured_default)
            });
    let default_source = if requirements_override_config {
        PermissionPresetDefaultSource::Requirements
    } else if configured_default_is_active {
        PermissionPresetDefaultSource::Config
    } else if config
        .config_layer_stack
        .requirements_toml()
        .allowed_permission_profiles
        .is_some()
    {
        PermissionPresetDefaultSource::Requirements
    } else if has_explicit_permission_config(&config_toml) {
        PermissionPresetDefaultSource::Config
    } else if requirements.approval_policy.source.is_some()
        || requirements.approvals_reviewer.source.is_some()
        || requirements.permission_profile.source.is_some()
    {
        PermissionPresetDefaultSource::Requirements
    } else {
        PermissionPresetDefaultSource::Implicit
    };
    let mut entries = Vec::new();

    for builtin in builtin_approval_presets() {
        let kind = match builtin.id {
            "read-only" => PermissionPresetKind::ReadOnly,
            "auto" => PermissionPresetKind::Auto,
            "full-access" => PermissionPresetKind::FullAccess,
            id => {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!("unknown built-in permission preset `{id}`"),
                ));
            }
        };
        entries.push(preset_entry(
            builtin.id,
            kind,
            Some(builtin.active_permission_profile.id),
            /* description */ None,
            builtin.permission_profile,
            builtin.approval,
            ApprovalsReviewer::User,
            &profile_catalog,
            config,
        ));
        if builtin.id == "auto" {
            entries.push(preset_entry(
                "granular",
                PermissionPresetKind::Granular,
                Some(BUILT_IN_PERMISSION_PROFILE_WORKSPACE.to_string()),
                /* description */ None,
                PermissionProfile::workspace_write(),
                AskForApproval::Granular(GranularApprovalConfig {
                    sandbox_approval: false,
                    rules: false,
                    skill_approval: false,
                    request_permissions: true,
                    mcp_elicitations: true,
                }),
                ApprovalsReviewer::User,
                &profile_catalog,
                config,
            ));
            if config.features.enabled(Feature::GuardianApproval) {
                entries.push(preset_entry(
                    "guardian-approvals",
                    PermissionPresetKind::GuardianApprovals,
                    Some(BUILT_IN_PERMISSION_PROFILE_WORKSPACE.to_string()),
                    /* description */ None,
                    PermissionProfile::workspace_write(),
                    AskForApproval::OnRequest,
                    ApprovalsReviewer::AutoReview,
                    &profile_catalog,
                    config,
                ));
            }
        }
    }

    for profile in profile_catalog.iter().filter(|profile| {
        !matches!(
            profile.id.as_str(),
            BUILT_IN_PERMISSION_PROFILE_READ_ONLY
                | BUILT_IN_PERMISSION_PROFILE_WORKSPACE
                | BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS
        )
    }) {
        entries.push(preset_entry(
            &format!("permission-profile:{}", profile.id),
            PermissionPresetKind::PermissionProfile,
            Some(profile.id.clone()),
            profile.description.clone(),
            profile.permission_profile.clone().ok_or_else(|| {
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!("permission profile `{}` could not be resolved", profile.id),
                )
            })?,
            config.permissions.approval_policy.value(),
            config.approvals_reviewer,
            &profile_catalog,
            config,
        ));
    }

    let has_legacy_config = has_legacy_permission_config(&config_toml);
    if has_legacy_config {
        entries.push(preset_entry(
            "legacy-config",
            PermissionPresetKind::LegacyConfig,
            /* permission_profile_id */ None,
            /* description */ None,
            config.permissions.permission_profile().clone(),
            config.permissions.approval_policy.value(),
            config.approvals_reviewer,
            &profile_catalog,
            config,
        ));
    }

    let default_preset_id =
        if default_source == PermissionPresetDefaultSource::Config && has_legacy_config {
            "legacy-config".to_string()
        } else {
            active_preset_id(config, &entries).ok_or_else(|| {
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "effective permissions do not match an allowed permission preset",
                )
            })?
        };
    if !entries
        .iter()
        .any(|entry| entry.preset.id == default_preset_id && entry.preset.allowed)
    {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!("default permission preset `{default_preset_id}` is not allowed"),
        ));
    }

    Ok(PermissionPresetCatalog {
        entries,
        default_preset_id,
        default_source,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "constructs the complete preset contract"
)]
fn preset_entry(
    id: &str,
    kind: PermissionPresetKind,
    permission_profile_id: Option<String>,
    description: Option<String>,
    permission_profile: PermissionProfile,
    approval_policy: AskForApproval,
    approvals_reviewer: ApprovalsReviewer,
    profile_catalog: &[codex_core::config::PermissionProfileCatalogEntry],
    config: &Config,
) -> PermissionPresetEntry {
    let profile_allowed = permission_profile_id.as_ref().is_none_or(|profile_id| {
        profile_catalog
            .iter()
            .find(|profile| profile.id == *profile_id)
            .is_some_and(|profile| profile.allowed)
    });
    let unavailability_reason = if !profile_allowed {
        Some(PermissionPresetUnavailabilityReason::PermissionProfile {
            permission_profile_id: permission_profile_id.clone().unwrap_or_default(),
        })
    } else if config
        .config_layer_stack
        .requirements()
        .approval_policy
        .can_set(&approval_policy)
        .is_err()
    {
        Some(PermissionPresetUnavailabilityReason::ApprovalPolicy)
    } else if config
        .config_layer_stack
        .requirements()
        .approvals_reviewer
        .can_set(&approvals_reviewer)
        .is_err()
    {
        Some(PermissionPresetUnavailabilityReason::ApprovalsReviewer)
    } else {
        None
    };
    let sandbox_policy = compatibility_sandbox_policy_for_permission_profile(
        &permission_profile,
        config.cwd.as_path(),
    );

    PermissionPresetEntry {
        preset: PermissionPreset {
            id: id.to_string(),
            kind,
            permission_profile_id,
            description,
            sandbox_policy: SandboxPolicy::from(sandbox_policy),
            approval_policy: approval_policy.into(),
            approvals_reviewer: approvals_reviewer.into(),
            allowed: unavailability_reason.is_none(),
            unavailability_reason,
        },
        permission_profile,
    }
}

fn permission_profiles_match(left: &PermissionProfile, right: &PermissionProfile) -> bool {
    match (left, right) {
        (PermissionProfile::Disabled, PermissionProfile::Disabled) => true,
        (
            PermissionProfile::External {
                network: left_network,
            },
            PermissionProfile::External {
                network: right_network,
            },
        ) => left_network == right_network,
        (
            PermissionProfile::Managed {
                file_system: left_file_system,
                network: left_network,
            },
            PermissionProfile::Managed {
                file_system: right_file_system,
                network: right_network,
            },
        ) => {
            left_network == right_network
                && match (left_file_system, right_file_system) {
                    (
                        ManagedFileSystemPermissions::Unrestricted,
                        ManagedFileSystemPermissions::Unrestricted,
                    ) => true,
                    (
                        ManagedFileSystemPermissions::Restricted {
                            entries: left_entries,
                            glob_scan_max_depth: left_depth,
                        },
                        ManagedFileSystemPermissions::Restricted {
                            entries: right_entries,
                            glob_scan_max_depth: right_depth,
                        },
                    ) => {
                        let mut remaining = right_entries.clone();
                        left_depth == right_depth
                            && left_entries.len() == remaining.len()
                            && left_entries.iter().all(|entry| {
                                let Some(index) =
                                    remaining.iter().position(|candidate| candidate == entry)
                                else {
                                    return false;
                                };
                                remaining.swap_remove(index);
                                true
                            })
                    }
                    _ => false,
                }
        }
        _ => false,
    }
}

fn active_preset_id(config: &Config, entries: &[PermissionPresetEntry]) -> Option<String> {
    let active_profile_id = config
        .permissions
        .active_permission_profile()
        .map(|profile| profile.id);
    entries
        .iter()
        .filter(|entry| entry.preset.allowed)
        .find(|entry| {
            entry.preset.permission_profile_id.as_deref() == active_profile_id.as_deref()
                && entry.preset.approval_policy.to_core()
                    == config.permissions.approval_policy.value()
                && entry.preset.approvals_reviewer.to_core() == config.approvals_reviewer
        })
        .map(|entry| entry.preset.id.clone())
        .or_else(|| {
            entries
                .iter()
                .filter(|entry| entry.preset.allowed)
                .find(|entry| {
                    permission_profiles_match(
                        &entry.permission_profile,
                        config.permissions.permission_profile(),
                    ) && entry.preset.approval_policy.to_core()
                        == config.permissions.approval_policy.value()
                        && entry.preset.approvals_reviewer.to_core() == config.approvals_reviewer
                })
                .map(|entry| entry.preset.id.clone())
        })
}

fn has_explicit_permission_config(config: &ConfigToml) -> bool {
    config.default_permissions.is_some() || has_legacy_permission_config(config)
}

fn has_legacy_permission_config(config: &ConfigToml) -> bool {
    config.default_permissions.is_none()
        && (config.sandbox_mode.is_some()
            || config.sandbox_workspace_write.is_some()
            || config.approval_policy.is_some()
            || config.approvals_reviewer.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_profile_matching_ignores_entry_order() {
        let profile = PermissionProfile::workspace_write();
        let mut reordered = profile.clone();
        let PermissionProfile::Managed {
            file_system: ManagedFileSystemPermissions::Restricted { entries, .. },
            ..
        } = &mut reordered
        else {
            panic!("workspace profile should be restricted");
        };
        entries.reverse();

        assert_ne!(profile, reordered);
        assert!(permission_profiles_match(&profile, &reordered));
    }
}
