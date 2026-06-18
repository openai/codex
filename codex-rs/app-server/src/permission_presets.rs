use crate::config_manager::ConfigManager;
use codex_app_server_protocol::PermissionPreset;
use codex_core::config::ConfigOverrides;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_utils_approval_presets::builtin_approval_presets;
use std::collections::HashMap;
use std::path::PathBuf;

struct PermissionPresetCandidate {
    id: String,
    permission_profile_id: String,
    description: Option<String>,
    approval_policy: AskForApproval,
    approvals_reviewer: ApprovalsReviewer,
    profile_is_default: bool,
}

pub(crate) async fn permission_presets(
    config_manager: &ConfigManager,
    cwd: Option<PathBuf>,
) -> std::io::Result<Vec<PermissionPreset>> {
    let config = config_manager
        .load_for_cwd(None, ConfigOverrides::default(), cwd)
        .await?;
    let selectable_profiles = config
        .selectable_permission_profiles()?
        .into_iter()
        .map(|profile| (profile.id.clone(), profile))
        .collect::<HashMap<_, _>>();
    let active_profile_id = config
        .permissions
        .active_permission_profile()
        .map(|profile| profile.id);
    let mut candidates = Vec::new();

    for preset in builtin_approval_presets() {
        if !selectable_profiles.contains_key(&preset.active_permission_profile.id) {
            continue;
        }
        let profile_is_default = match active_profile_id.as_deref() {
            Some(id) => id == preset.active_permission_profile.id,
            None => preset.matches_permission_profile(
                config.permissions.permission_profile(),
                config.cwd.as_path(),
            ),
        };
        candidates.push(PermissionPresetCandidate {
            id: preset.id.to_string(),
            permission_profile_id: preset.active_permission_profile.id.clone(),
            description: None,
            approval_policy: preset.approval,
            approvals_reviewer: ApprovalsReviewer::User,
            profile_is_default,
        });
        if preset.id == "auto" {
            candidates.push(PermissionPresetCandidate {
                id: "granular".to_string(),
                permission_profile_id: preset.active_permission_profile.id.clone(),
                description: None,
                approval_policy: AskForApproval::Granular(GranularApprovalConfig {
                    sandbox_approval: false,
                    rules: false,
                    skill_approval: false,
                    request_permissions: true,
                    mcp_elicitations: true,
                }),
                approvals_reviewer: ApprovalsReviewer::User,
                profile_is_default,
            });
            if config.features.enabled(Feature::GuardianApproval) {
                candidates.push(PermissionPresetCandidate {
                    id: "guardian-approvals".to_string(),
                    permission_profile_id: preset.active_permission_profile.id,
                    description: None,
                    approval_policy: AskForApproval::OnRequest,
                    approvals_reviewer: ApprovalsReviewer::AutoReview,
                    profile_is_default,
                });
            }
        }
    }

    let mut custom_profiles = selectable_profiles
        .into_values()
        .filter(|profile| !profile.id.starts_with(':'))
        .collect::<Vec<_>>();
    custom_profiles.sort_by(|left, right| left.id.cmp(&right.id));
    candidates.extend(custom_profiles.into_iter().map(|profile| {
        let profile_is_default = active_profile_id.as_deref() == Some(profile.id.as_str());
        PermissionPresetCandidate {
            id: format!("permission-profile:{}", profile.id),
            permission_profile_id: profile.id,
            description: profile.description,
            approval_policy: config.permissions.approval_policy.value(),
            approvals_reviewer: config.approvals_reviewer,
            profile_is_default,
        }
    }));

    let requirements = config.config_layer_stack.requirements();
    let mut presets = Vec::new();
    for candidate in candidates {
        let approval_policy = if requirements
            .approval_policy
            .can_set(&candidate.approval_policy)
            .is_ok()
        {
            candidate.approval_policy
        } else {
            requirements.approval_policy.value()
        };
        let approvals_reviewer = if requirements
            .approvals_reviewer
            .can_set(&candidate.approvals_reviewer)
            .is_ok()
        {
            candidate.approvals_reviewer
        } else {
            requirements.approvals_reviewer.value()
        };
        let preset = PermissionPreset {
            id: candidate.id,
            permission_profile_id: candidate.permission_profile_id,
            description: candidate.description,
            approval_policy: approval_policy.into(),
            approvals_reviewer: approvals_reviewer.into(),
            is_default: candidate.profile_is_default
                && config.permissions.approval_policy.value() == approval_policy
                && config.approvals_reviewer == approvals_reviewer,
        };
        if presets.iter().any(|existing: &PermissionPreset| {
            existing.permission_profile_id == preset.permission_profile_id
                && existing.approval_policy == preset.approval_policy
                && existing.approvals_reviewer == preset.approvals_reviewer
        }) {
            continue;
        }
        presets.push(preset);
    }

    Ok(presets)
}
