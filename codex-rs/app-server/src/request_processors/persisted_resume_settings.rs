use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::ActivePermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadSettingsSnapshot;
use codex_rollout::InitialHistory;
use codex_rollout::RolloutItem;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct PersistedResumeSettings {
    pub(super) approval_policy: AskForApproval,
    pub(super) approvals_reviewer: Option<ApprovalsReviewer>,
    pub(super) active_permission_profile: Option<ActivePermissionProfile>,
}

pub(super) fn latest_persisted_resume_settings(
    history: &[RolloutItem],
) -> Option<PersistedResumeSettings> {
    history
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, item)| match item {
            RolloutItem::TurnContext(turn_context) => Some(PersistedResumeSettings {
                approval_policy: turn_context.approval_policy,
                approvals_reviewer: turn_context.approvals_reviewer.or_else(|| {
                    history[..index].iter().rev().find_map(|item| match item {
                        RolloutItem::TurnContext(turn_context) => turn_context.approvals_reviewer,
                        RolloutItem::EventMsg(EventMsg::ThreadSettingsApplied(event)) => {
                            Some(event.thread_settings.approvals_reviewer)
                        }
                        _ => None,
                    })
                }),
                active_permission_profile: turn_context.active_permission_profile.clone(),
            }),
            RolloutItem::EventMsg(EventMsg::ThreadSettingsApplied(event)) => {
                Some(PersistedResumeSettings {
                    approval_policy: event.thread_settings.approval_policy,
                    approvals_reviewer: Some(event.thread_settings.approvals_reviewer),
                    active_permission_profile: event
                        .thread_settings
                        .active_permission_profile
                        .clone(),
                })
            }
            _ => None,
        })
}

pub(super) fn latest_persisted_model_settings(
    history: &InitialHistory,
) -> Option<&ThreadSettingsSnapshot> {
    let InitialHistory::Resumed(history) = history else {
        return None;
    };
    let mut crossed_foreign_snapshot = false;
    for item in history.history.iter().rev() {
        let RolloutItem::EventMsg(EventMsg::ThreadSettingsApplied(event)) = item else {
            continue;
        };
        match event.thread_id {
            Some(thread_id) if thread_id == history.conversation_id => {
                return Some(&event.thread_settings);
            }
            Some(_) => crossed_foreign_snapshot = true,
            None if !crossed_foreign_snapshot => return Some(&event.thread_settings),
            None => {}
        }
    }
    None
}

#[cfg(test)]
#[path = "persisted_resume_settings_tests.rs"]
mod tests;
