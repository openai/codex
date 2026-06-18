use crate::config_manager::ConfigManager;
use codex_app_server_protocol::PermissionPreset;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_utils_approval_presets::builtin_approval_presets;
use futures::StreamExt;
use std::path::PathBuf;

const PRESET_RESOLUTION_CONCURRENCY: usize = 5;

struct PermissionPresetCandidate {
    id: String,
    permission_profile_id: String,
    description: Option<String>,
    approval_policy: AskForApproval,
    approvals_reviewer: ApprovalsReviewer,
}

pub(crate) async fn permission_presets(
    config_manager: &ConfigManager,
    cwd: Option<PathBuf>,
) -> std::io::Result<Vec<PermissionPreset>> {
    let config = config_manager
        .load_for_cwd(None, ConfigOverrides::default(), cwd.clone())
        .await?;
    let mut candidates = Vec::new();

    for preset in builtin_approval_presets() {
        candidates.push(PermissionPresetCandidate {
            id: preset.id.to_string(),
            permission_profile_id: preset.active_permission_profile.id.clone(),
            description: None,
            approval_policy: preset.approval,
            approvals_reviewer: ApprovalsReviewer::User,
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
            });
            if config.features.enabled(Feature::GuardianApproval) {
                candidates.push(PermissionPresetCandidate {
                    id: "guardian-approvals".to_string(),
                    permission_profile_id: preset.active_permission_profile.id,
                    description: None,
                    approval_policy: AskForApproval::OnRequest,
                    approvals_reviewer: ApprovalsReviewer::AutoReview,
                });
            }
        }
    }

    let mut custom_profiles = config.custom_permission_profiles.clone();
    custom_profiles.sort_by(|left, right| left.id.cmp(&right.id));
    candidates.extend(
        custom_profiles
            .into_iter()
            .map(|profile| PermissionPresetCandidate {
                id: format!("permission-profile:{}", profile.id),
                permission_profile_id: profile.id,
                description: profile.description,
                approval_policy: config.permissions.approval_policy.value(),
                approvals_reviewer: config.approvals_reviewer,
            }),
    );

    let resolved_presets = futures::stream::iter(candidates)
        .map(|candidate| resolve_candidate(config_manager, cwd.clone(), &config, candidate))
        .buffered(PRESET_RESOLUTION_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
    let mut presets = Vec::new();
    for preset in resolved_presets {
        let Some(preset) = preset else {
            continue;
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

async fn resolve_candidate(
    config_manager: &ConfigManager,
    cwd: Option<PathBuf>,
    default_config: &Config,
    candidate: PermissionPresetCandidate,
) -> Option<PermissionPreset> {
    let config = config_manager
        .load_for_cwd(
            None,
            ConfigOverrides {
                approval_policy: Some(candidate.approval_policy),
                approvals_reviewer: Some(candidate.approvals_reviewer),
                default_permissions: Some(candidate.permission_profile_id.clone()),
                ..Default::default()
            },
            cwd,
        )
        .await
        .ok()?;
    let active_profile = config.permissions.active_permission_profile()?;
    if active_profile.id != candidate.permission_profile_id {
        return None;
    }
    let approval_policy = config.permissions.approval_policy.value();
    let approvals_reviewer = config.approvals_reviewer;
    let is_default = default_config
        .permissions
        .active_permission_profile()
        .is_some_and(|profile| profile.id == candidate.permission_profile_id)
        && default_config.permissions.approval_policy.value() == approval_policy
        && default_config.approvals_reviewer == approvals_reviewer;

    Some(PermissionPreset {
        id: candidate.id,
        permission_profile_id: candidate.permission_profile_id,
        description: candidate.description,
        approval_policy: approval_policy.into(),
        approvals_reviewer: approvals_reviewer.into(),
        is_default,
    })
}
