use super::PersistedResumeSettings;
use super::latest_persisted_resume_settings;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::ThreadSettingsAppliedEvent;
use codex_protocol::protocol::ThreadSettingsSnapshot;
use codex_protocol::protocol::TurnContextItem;
use codex_rollout::RolloutItem;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

fn cwd() -> AbsolutePathBuf {
    AbsolutePathBuf::try_from(std::env::current_dir().expect("current directory"))
        .expect("absolute current directory")
}

fn settings_item(approvals_reviewer: ApprovalsReviewer) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::ThreadSettingsApplied(
        ThreadSettingsAppliedEvent {
            thread_settings: ThreadSettingsSnapshot {
                model: "gpt-5".to_string(),
                model_provider_id: "openai".to_string(),
                service_tier: None,
                approval_policy: AskForApproval::OnRequest,
                approvals_reviewer,
                permission_profile: PermissionProfile::read_only(),
                active_permission_profile: None,
                cwd: cwd(),
                reasoning_effort: None,
                reasoning_summary: None,
                personality: None,
                collaboration_mode: CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: "gpt-5".to_string(),
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                },
            },
        },
    ))
}

fn turn_context_item(turn_id: &str, approvals_reviewer: Option<ApprovalsReviewer>) -> RolloutItem {
    RolloutItem::TurnContext(TurnContextItem {
        turn_id: Some(turn_id.to_string()),
        cwd: cwd(),
        workspace_roots: None,
        current_date: None,
        timezone: None,
        approval_policy: AskForApproval::OnRequest,
        approvals_reviewer,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        permission_profile: None,
        active_permission_profile: None,
        network: None,
        file_system_sandbox_policy: None,
        model: "gpt-5".to_string(),
        comp_hash: None,
        personality: None,
        collaboration_mode: None,
        multi_agent_version: None,
        multi_agent_mode: None,
        realtime_active: None,
        effort: None,
        summary: codex_protocol::config_types::ReasoningSummary::Auto,
    })
}

#[test]
fn latest_settings_snapshot_wins() {
    let history = vec![
        settings_item(ApprovalsReviewer::User),
        settings_item(ApprovalsReviewer::AutoReview),
    ];

    assert_eq!(
        latest_persisted_resume_settings(&history),
        Some(PersistedResumeSettings {
            approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
        })
    );
}

#[test]
fn latest_turn_context_wins_over_earlier_settings_update() {
    let history = vec![
        settings_item(ApprovalsReviewer::AutoReview),
        turn_context_item("turn-2", Some(ApprovalsReviewer::User)),
    ];

    assert_eq!(
        latest_persisted_resume_settings(&history),
        Some(PersistedResumeSettings {
            approvals_reviewer: Some(ApprovalsReviewer::User),
        })
    );
}

#[test]
fn older_reviewer_is_used_when_latest_turn_context_omits_it() {
    let history = vec![
        turn_context_item("turn-1", Some(ApprovalsReviewer::AutoReview)),
        turn_context_item("turn-2", /*approvals_reviewer*/ None),
    ];

    assert_eq!(
        latest_persisted_resume_settings(&history),
        Some(PersistedResumeSettings {
            approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
        })
    );
}
