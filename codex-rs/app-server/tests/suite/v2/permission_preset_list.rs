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
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_READ_ONLY;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_WORKSPACE;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::test]
async fn permission_preset_list_returns_resolved_builtin_presets() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"
default_permissions = ":workspace"
approvals_reviewer = "auto_review"

[features]
guardian_approval = true
"#,
    )?;

    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_permission_preset_list_request(PermissionPresetListParams {
            cursor: None,
            limit: None,
            cwd: None,
        })
        .await?;
    let actual = read_response::<PermissionPresetListResponse>(&mut mcp, request_id).await?;

    assert_eq!(
        actual,
        PermissionPresetListResponse {
            data: vec![
                preset(
                    "read-only",
                    BUILT_IN_PERMISSION_PROFILE_READ_ONLY,
                    AskForApproval::OnRequest,
                    ApprovalsReviewer::User,
                    false,
                ),
                preset(
                    "auto",
                    BUILT_IN_PERMISSION_PROFILE_WORKSPACE,
                    AskForApproval::OnRequest,
                    ApprovalsReviewer::User,
                    false,
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
                    false,
                ),
                preset(
                    "guardian-approvals",
                    BUILT_IN_PERMISSION_PROFILE_WORKSPACE,
                    AskForApproval::OnRequest,
                    ApprovalsReviewer::AutoReview,
                    true,
                ),
                preset(
                    "full-access",
                    BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS,
                    AskForApproval::Never,
                    ApprovalsReviewer::User,
                    false,
                ),
            ],
            next_cursor: None,
        }
    );
    Ok(())
}

#[tokio::test]
async fn permission_preset_list_applies_managed_profile_and_policy_fallbacks() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"
default_permissions = "dev"
approval_policy = "never"
approvals_reviewer = "auto_review"

[permissions.dev]
description = "Day-to-day coding work."

[permissions.dev.filesystem]
":workspace_roots" = "write"
"#,
    )?;
    std::fs::write(
        codex_home.path().join("requirements.toml"),
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
    )?;

    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_permission_preset_list_request(PermissionPresetListParams {
            cursor: None,
            limit: None,
            cwd: None,
        })
        .await?;
    let actual = read_response::<PermissionPresetListResponse>(&mut mcp, request_id).await?;

    assert_eq!(
        actual,
        PermissionPresetListResponse {
            data: vec![
                preset(
                    "read-only",
                    BUILT_IN_PERMISSION_PROFILE_READ_ONLY,
                    AskForApproval::OnRequest,
                    ApprovalsReviewer::User,
                    false,
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
    for (config, requirements, expected) in [
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
                true,
            ),
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
                true,
            ),
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
                true,
            ),
        ),
    ] {
        let codex_home = TempDir::new()?;
        std::fs::write(codex_home.path().join("config.toml"), config)?;
        if let Some(requirements) = requirements {
            std::fs::write(codex_home.path().join("requirements.toml"), requirements)?;
        }

        let mut mcp = TestAppServer::new(codex_home.path()).await?;
        timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

        let request_id = mcp
            .send_permission_preset_list_request(PermissionPresetListParams {
                cursor: None,
                limit: None,
                cwd: None,
            })
            .await?;
        let actual = read_response::<PermissionPresetListResponse>(&mut mcp, request_id).await?;

        assert_eq!(
            actual
                .data
                .into_iter()
                .filter(|preset| preset.is_default)
                .collect::<Vec<_>>(),
            vec![expected]
        );
    }
    Ok(())
}

#[tokio::test]
async fn permission_preset_list_removes_presets_with_identical_resolved_settings() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"
default_permissions = ":workspace"
approval_policy = "never"
approvals_reviewer = "auto_review"
"#,
    )?;
    std::fs::write(
        codex_home.path().join("requirements.toml"),
        r#"
allowed_approval_policies = ["on-request"]
allowed_approvals_reviewers = ["user"]
default_permissions = ":workspace"

[allowed_permission_profiles]
":workspace" = true
"#,
    )?;

    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_permission_preset_list_request(PermissionPresetListParams {
            cursor: None,
            limit: None,
            cwd: None,
        })
        .await?;
    let actual = read_response::<PermissionPresetListResponse>(&mut mcp, request_id).await?;

    assert_eq!(
        actual,
        PermissionPresetListResponse {
            data: vec![preset(
                "auto",
                BUILT_IN_PERMISSION_PROFILE_WORKSPACE,
                AskForApproval::OnRequest,
                ApprovalsReviewer::User,
                true,
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

async fn read_response<T: serde::de::DeserializeOwned>(
    mcp: &mut TestAppServer,
    request_id: i64,
) -> Result<T> {
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}
