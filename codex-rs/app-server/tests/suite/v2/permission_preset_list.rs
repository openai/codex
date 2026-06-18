use std::time::Duration;

use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::ApprovalsReviewer;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::PermissionPreset;
use codex_app_server_protocol::PermissionPresetListParams;
use codex_app_server_protocol::PermissionPresetListResponse;
use codex_app_server_protocol::RequestId;
use codex_core::config::set_project_trust_level;
use codex_protocol::config_types::TrustLevel;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_READ_ONLY;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_WORKSPACE;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::test]
async fn permission_preset_list_returns_resolved_builtin_presets() -> Result<()> {
    let actual = list_permission_presets_from_config(
        r#"
default_permissions = ":workspace"
approvals_reviewer = "auto_review"

[features]
guardian_approval = true
"#,
        /*requirements*/ None,
    )
    .await?;

    assert_eq!(
        actual,
        PermissionPresetListResponse {
            data: vec![
                preset(
                    "read-only",
                    BUILT_IN_PERMISSION_PROFILE_READ_ONLY,
                    AskForApproval::OnRequest,
                    ApprovalsReviewer::User,
                    /*is_default*/ false,
                ),
                preset(
                    "auto",
                    BUILT_IN_PERMISSION_PROFILE_WORKSPACE,
                    AskForApproval::OnRequest,
                    ApprovalsReviewer::User,
                    /*is_default*/ false,
                ),
                preset(
                    "granular",
                    BUILT_IN_PERMISSION_PROFILE_WORKSPACE,
                    AskForApproval::Granular {
                        sandbox_approval: false,
                        rules: false,
                        skill_approval: false,
                        request_permissions: true,
                        mcp_elicitations: true,
                    },
                    ApprovalsReviewer::User,
                    /*is_default*/ false,
                ),
                preset(
                    "guardian-approvals",
                    BUILT_IN_PERMISSION_PROFILE_WORKSPACE,
                    AskForApproval::OnRequest,
                    ApprovalsReviewer::AutoReview,
                    /*is_default*/ true,
                ),
                preset(
                    "full-access",
                    BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS,
                    AskForApproval::Never,
                    ApprovalsReviewer::User,
                    /*is_default*/ false,
                ),
            ],
            next_cursor: None,
        }
    );
    Ok(())
}

#[tokio::test]
async fn permission_preset_list_applies_managed_profile_and_policy_fallbacks() -> Result<()> {
    let actual = list_permission_presets_from_config(
        r#"
default_permissions = "dev"
approval_policy = "never"
approvals_reviewer = "auto_review"

[permissions.dev]
description = "Day-to-day coding work."

[permissions.dev.filesystem]
":workspace_roots" = "write"
"#,
        Some(
            r#"
allowed_approval_policies = ["on-request"]
allowed_approvals_reviewers = ["user"]
default_permissions = "audit"

[allowed_permission_profiles]
":read-only" = true
audit = true

[permissions.audit]
description = "Inspect without writes."

[permissions.audit.filesystem]
":workspace_roots" = "read"
"#,
        ),
    )
    .await?;

    assert_eq!(
        actual,
        PermissionPresetListResponse {
            data: vec![
                preset(
                    "read-only",
                    BUILT_IN_PERMISSION_PROFILE_READ_ONLY,
                    AskForApproval::OnRequest,
                    ApprovalsReviewer::User,
                    /*is_default*/ false,
                ),
                PermissionPreset {
                    id: "permission-profile:audit".to_string(),
                    permission_profile_id: "audit".to_string(),
                    description: Some("Inspect without writes.".to_string()),
                    approval_policy: AskForApproval::OnRequest,
                    approvals_reviewer: ApprovalsReviewer::User,
                    is_default: true,
                },
            ],
            next_cursor: None,
        }
    );
    Ok(())
}

#[tokio::test]
async fn permission_preset_list_matches_legacy_defaults() -> Result<()> {
    for (config, requirements, expected, expected_ids) in [
        (
            r#"
sandbox_mode = "read-only"
approval_policy = "on-request"
"#,
            None,
            preset(
                "read-only",
                BUILT_IN_PERMISSION_PROFILE_READ_ONLY,
                AskForApproval::OnRequest,
                ApprovalsReviewer::User,
                /*is_default*/ true,
            ),
            None,
        ),
        (
            r#"
sandbox_mode = "workspace-write"
approval_policy = "on-request"
"#,
            Some(
                r#"
allowed_sandbox_modes = ["read-only"]
"#,
            ),
            preset(
                "read-only",
                BUILT_IN_PERMISSION_PROFILE_READ_ONLY,
                AskForApproval::OnRequest,
                ApprovalsReviewer::User,
                /*is_default*/ true,
            ),
            Some(vec!["read-only"]),
        ),
        (
            r#"
sandbox_mode = "danger-full-access"
approval_policy = "never"
"#,
            None,
            preset(
                "full-access",
                BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS,
                AskForApproval::Never,
                ApprovalsReviewer::User,
                /*is_default*/ true,
            ),
            None,
        ),
    ] {
        let actual = list_permission_presets_from_config(config, requirements).await?;

        if let Some(expected_ids) = expected_ids {
            assert_eq!(
                actual
                    .data
                    .iter()
                    .map(|preset| preset.id.as_str())
                    .collect::<Vec<_>>(),
                expected_ids
            );
        }

        assert_eq!(
            actual
                .data
                .iter()
                .filter(|preset| preset.is_default)
                .cloned()
                .collect::<Vec<_>>(),
            vec![expected]
        );
    }
    Ok(())
}

#[tokio::test]
async fn permission_preset_list_resolves_project_profiles_and_paginates() -> Result<()> {
    let codex_home = TempDir::new()?;
    let workspace = TempDir::new()?;
    let project_config_dir = workspace.path().join(".codex");
    std::fs::create_dir_all(&project_config_dir)?;
    std::fs::write(
        project_config_dir.join("config.toml"),
        r#"
default_permissions = "project"

[permissions.project]
description = "Project-scoped profile."
extends = ":workspace"

[features]
guardian_approval = false
"#,
    )?;
    set_project_trust_level(codex_home.path(), workspace.path(), TrustLevel::Trusted)?;

    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    let cwd = workspace.path().to_string_lossy().into_owned();

    let first = list_permission_presets(
        &mut mcp,
        PermissionPresetListParams {
            cursor: None,
            limit: Some(4),
            cwd: Some(cwd.clone()),
        },
    )
    .await?;
    assert_eq!(
        first,
        PermissionPresetListResponse {
            data: vec![
                preset(
                    "read-only",
                    BUILT_IN_PERMISSION_PROFILE_READ_ONLY,
                    AskForApproval::OnRequest,
                    ApprovalsReviewer::User,
                    /*is_default*/ false,
                ),
                preset(
                    "auto",
                    BUILT_IN_PERMISSION_PROFILE_WORKSPACE,
                    AskForApproval::OnRequest,
                    ApprovalsReviewer::User,
                    /*is_default*/ false,
                ),
                preset(
                    "granular",
                    BUILT_IN_PERMISSION_PROFILE_WORKSPACE,
                    AskForApproval::Granular {
                        sandbox_approval: false,
                        rules: false,
                        skill_approval: false,
                        request_permissions: true,
                        mcp_elicitations: true,
                    },
                    ApprovalsReviewer::User,
                    /*is_default*/ false,
                ),
                preset(
                    "full-access",
                    BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS,
                    AskForApproval::Never,
                    ApprovalsReviewer::User,
                    /*is_default*/ false,
                ),
            ],
            next_cursor: Some("4".to_string()),
        }
    );

    let second = list_permission_presets(
        &mut mcp,
        PermissionPresetListParams {
            cursor: first.next_cursor,
            limit: Some(4),
            cwd: Some(cwd),
        },
    )
    .await?;
    assert_eq!(
        second,
        PermissionPresetListResponse {
            data: vec![PermissionPreset {
                id: "permission-profile:project".to_string(),
                permission_profile_id: "project".to_string(),
                description: Some("Project-scoped profile.".to_string()),
                approval_policy: AskForApproval::OnRequest,
                approvals_reviewer: ApprovalsReviewer::User,
                is_default: true,
            }],
            next_cursor: None,
        }
    );
    Ok(())
}

#[tokio::test]
async fn permission_preset_list_removes_presets_with_identical_resolved_settings() -> Result<()> {
    let actual = list_permission_presets_from_config(
        r#"
default_permissions = ":workspace"
approval_policy = "never"
approvals_reviewer = "auto_review"
"#,
        Some(
            r#"
allowed_approval_policies = ["on-request"]
allowed_approvals_reviewers = ["user"]
default_permissions = ":workspace"

[allowed_permission_profiles]
":workspace" = true
"#,
        ),
    )
    .await?;

    assert_eq!(
        actual,
        PermissionPresetListResponse {
            data: vec![preset(
                "auto",
                BUILT_IN_PERMISSION_PROFILE_WORKSPACE,
                AskForApproval::OnRequest,
                ApprovalsReviewer::User,
                /*is_default*/ true,
            )],
            next_cursor: None,
        }
    );
    Ok(())
}

fn preset(
    id: &str,
    permission_profile_id: &str,
    approval_policy: AskForApproval,
    approvals_reviewer: ApprovalsReviewer,
    is_default: bool,
) -> PermissionPreset {
    PermissionPreset {
        id: id.to_string(),
        permission_profile_id: permission_profile_id.to_string(),
        description: None,
        approval_policy,
        approvals_reviewer,
        is_default,
    }
}

async fn list_permission_presets(
    mcp: &mut TestAppServer,
    params: PermissionPresetListParams,
) -> Result<PermissionPresetListResponse> {
    let request_id = mcp.send_permission_preset_list_request(params).await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn list_permission_presets_from_config(
    config: &str,
    requirements: Option<&str>,
) -> Result<PermissionPresetListResponse> {
    let codex_home = TempDir::new()?;
    std::fs::write(codex_home.path().join("config.toml"), config)?;
    if let Some(requirements) = requirements {
        std::fs::write(codex_home.path().join("requirements.toml"), requirements)?;
    }

    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    list_permission_presets(
        &mut mcp,
        PermissionPresetListParams {
            cursor: None,
            limit: None,
            cwd: None,
        },
    )
    .await
}
