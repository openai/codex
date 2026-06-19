use std::time::Duration;

use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::ApprovalsReviewer;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::PermissionPreset;
use codex_app_server_protocol::PermissionPresetDefaultSource;
use codex_app_server_protocol::PermissionPresetKind;
use codex_app_server_protocol::PermissionPresetListParams;
use codex_app_server_protocol::PermissionPresetListResponse;
use codex_app_server_protocol::PermissionPresetUnavailabilityReason;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxPolicy;
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
async fn permission_preset_list_returns_complete_builtin_catalog() -> Result<()> {
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
                builtin_preset(
                    "read-only",
                    PermissionPresetKind::ReadOnly,
                    BUILT_IN_PERMISSION_PROFILE_READ_ONLY,
                    read_only(),
                    AskForApproval::OnRequest,
                    ApprovalsReviewer::User,
                ),
                builtin_preset(
                    "auto",
                    PermissionPresetKind::Auto,
                    BUILT_IN_PERMISSION_PROFILE_WORKSPACE,
                    workspace_write(),
                    AskForApproval::OnRequest,
                    ApprovalsReviewer::User,
                ),
                builtin_preset(
                    "granular",
                    PermissionPresetKind::Granular,
                    BUILT_IN_PERMISSION_PROFILE_WORKSPACE,
                    workspace_write(),
                    AskForApproval::Granular {
                        sandbox_approval: false,
                        rules: false,
                        skill_approval: false,
                        request_permissions: true,
                        mcp_elicitations: true,
                    },
                    ApprovalsReviewer::User,
                ),
                builtin_preset(
                    "guardian-approvals",
                    PermissionPresetKind::GuardianApprovals,
                    BUILT_IN_PERMISSION_PROFILE_WORKSPACE,
                    workspace_write(),
                    AskForApproval::OnRequest,
                    ApprovalsReviewer::AutoReview,
                ),
                builtin_preset(
                    "full-access",
                    PermissionPresetKind::FullAccess,
                    BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS,
                    SandboxPolicy::DangerFullAccess,
                    AskForApproval::Never,
                    ApprovalsReviewer::User,
                ),
            ],
            next_cursor: None,
            default_preset_id: "guardian-approvals".to_string(),
            default_source: PermissionPresetDefaultSource::Config,
        }
    );
    Ok(())
}

#[tokio::test]
async fn permission_preset_list_keeps_allowed_config_default_provenance() -> Result<()> {
    let actual = list_permission_presets_from_config(
        r#"
default_permissions = ":workspace"
"#,
        Some(
            r#"
default_permissions = ":read-only"

[allowed_permission_profiles]
":read-only" = true
":workspace" = true
"#,
        ),
    )
    .await?;

    assert_eq!(actual.default_preset_id, "auto");
    assert_eq!(actual.default_source, PermissionPresetDefaultSource::Config);
    Ok(())
}

#[tokio::test]
async fn permission_preset_list_retains_disallowed_profiles_without_coercion() -> Result<()> {
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

    assert_eq!(actual.default_preset_id, "permission-profile:audit");
    assert_eq!(
        actual.default_source,
        PermissionPresetDefaultSource::Requirements
    );
    assert_eq!(
        actual
            .data
            .iter()
            .map(|preset| preset.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "read-only",
            "auto",
            "granular",
            "guardian-approvals",
            "full-access",
            "permission-profile:audit",
            "permission-profile:dev",
        ]
    );
    assert_eq!(
        preset(&actual, "auto").unavailability_reason,
        Some(PermissionPresetUnavailabilityReason::PermissionProfile {
            permission_profile_id: BUILT_IN_PERMISSION_PROFILE_WORKSPACE.to_string(),
        })
    );
    assert_eq!(
        preset(&actual, "full-access").approval_policy,
        AskForApproval::Never
    );
    assert_eq!(
        preset(&actual, "permission-profile:dev").unavailability_reason,
        Some(PermissionPresetUnavailabilityReason::PermissionProfile {
            permission_profile_id: "dev".to_string(),
        })
    );
    assert!(preset(&actual, "permission-profile:audit").allowed);
    Ok(())
}

#[tokio::test]
async fn permission_preset_list_reports_approval_and_reviewer_constraints() -> Result<()> {
    let actual = list_permission_presets_from_config(
        r#"
[features]
guardian_approval = true
"#,
        Some(
            r#"
allowed_approval_policies = ["on-request"]
allowed_approvals_reviewers = ["user"]
default_permissions = ":workspace"

[allowed_permission_profiles]
":read-only" = true
":workspace" = true
":danger-full-access" = true
"#,
        ),
    )
    .await?;

    assert_eq!(
        preset(&actual, "granular").unavailability_reason,
        Some(PermissionPresetUnavailabilityReason::ApprovalPolicy)
    );
    assert_eq!(
        preset(&actual, "guardian-approvals").unavailability_reason,
        Some(PermissionPresetUnavailabilityReason::ApprovalsReviewer)
    );
    assert_eq!(
        preset(&actual, "full-access").unavailability_reason,
        Some(PermissionPresetUnavailabilityReason::ApprovalPolicy)
    );
    assert_eq!(
        preset(&actual, "full-access").approval_policy,
        AskForApproval::Never
    );
    Ok(())
}

#[tokio::test]
async fn permission_preset_list_keeps_equivalent_legacy_config() -> Result<()> {
    let actual = list_permission_presets_from_config(
        r#"
sandbox_mode = "read-only"
approval_policy = "on-request"
"#,
        /*requirements*/ None,
    )
    .await?;

    assert_eq!(actual.default_preset_id, "legacy-config");
    assert_eq!(actual.default_source, PermissionPresetDefaultSource::Config);
    assert_eq!(
        preset(&actual, "legacy-config").kind,
        PermissionPresetKind::LegacyConfig
    );
    assert_eq!(
        preset(&actual, "legacy-config").sandbox_policy,
        preset(&actual, "read-only").sandbox_policy
    );
    Ok(())
}

#[tokio::test]
async fn permission_preset_list_keeps_named_profile_equivalent_to_builtin() -> Result<()> {
    let actual = list_permission_presets_from_config(
        r#"
default_permissions = "project"

[permissions.project]
description = "Project-scoped profile."
extends = ":workspace"
"#,
        /*requirements*/ None,
    )
    .await?;

    assert_eq!(actual.default_preset_id, "permission-profile:project");
    assert_eq!(
        preset(&actual, "permission-profile:project").sandbox_policy,
        preset(&actual, "auto").sandbox_policy
    );
    Ok(())
}

#[tokio::test]
async fn permission_preset_list_paginates_without_losing_default_metadata() -> Result<()> {
    let codex_home = TempDir::new()?;
    let workspace = TempDir::new()?;
    let project_config_dir = workspace.path().join(".codex");
    std::fs::create_dir_all(&project_config_dir)?;
    std::fs::write(
        project_config_dir.join("config.toml"),
        r#"
default_permissions = "project"

[permissions.project]
extends = ":workspace"
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
    let second = list_permission_presets(
        &mut mcp,
        PermissionPresetListParams {
            cursor: first.next_cursor.clone(),
            limit: Some(4),
            cwd: Some(cwd),
        },
    )
    .await?;

    assert_eq!(first.next_cursor, Some("4".to_string()));
    assert_eq!(second.data.len(), 2);
    assert_eq!(first.default_preset_id, second.default_preset_id);
    assert_eq!(first.default_source, second.default_source);
    Ok(())
}

fn builtin_preset(
    id: &str,
    kind: PermissionPresetKind,
    permission_profile_id: &str,
    sandbox_policy: SandboxPolicy,
    approval_policy: AskForApproval,
    approvals_reviewer: ApprovalsReviewer,
) -> PermissionPreset {
    PermissionPreset {
        id: id.to_string(),
        kind,
        permission_profile_id: Some(permission_profile_id.to_string()),
        description: None,
        sandbox_policy,
        approval_policy,
        approvals_reviewer,
        allowed: true,
        unavailability_reason: None,
    }
}

fn read_only() -> SandboxPolicy {
    SandboxPolicy::ReadOnly {
        network_access: false,
    }
}

fn workspace_write() -> SandboxPolicy {
    SandboxPolicy::WorkspaceWrite {
        writable_roots: Vec::new(),
        network_access: false,
        exclude_tmpdir_env_var: false,
        exclude_slash_tmp: false,
    }
}

fn preset<'a>(response: &'a PermissionPresetListResponse, id: &str) -> &'a PermissionPreset {
    response
        .data
        .iter()
        .find(|preset| preset.id == id)
        .unwrap_or_else(|| panic!("missing permission preset `{id}`"))
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
