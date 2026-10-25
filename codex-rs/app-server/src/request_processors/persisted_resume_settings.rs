use codex_protocol::ThreadId;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::ActivePermissionProfile;
use codex_protocol::models::PermissionProfile;
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
    pub(super) unnamed_permission_profile: Option<PermissionProfile>,
}

impl PersistedResumeSettings {
    pub(super) fn from_history(history: &[RolloutItem], thread_id: ThreadId) -> Option<Self> {
        // A running turn can record stale permissions. Prefer a settings update whose
        // owner is known, including a legacy update following that owner's checkpoint.
        let mut latest_legacy = None;
        for item in history.iter().rev() {
            let RolloutItem::EventMsg(EventMsg::ThreadSettingsApplied(event)) = item else {
                continue;
            };
            match event.thread_id {
                Some(owner) if owner == thread_id => {
                    return Some(Self::from(latest_legacy.unwrap_or(&event.thread_settings)));
                }
                Some(_) => latest_legacy = None,
                None => {
                    latest_legacy.get_or_insert(&event.thread_settings);
                }
            }
        }
        latest_persisted_resume_settings(history)
    }
}

impl From<&ThreadSettingsSnapshot> for PersistedResumeSettings {
    fn from(settings: &ThreadSettingsSnapshot) -> Self {
        Self {
            approval_policy: settings.approval_policy,
            approvals_reviewer: Some(settings.approvals_reviewer),
            active_permission_profile: settings.active_permission_profile.clone(),
            unnamed_permission_profile: settings
                .active_permission_profile
                .is_none()
                .then(|| settings.permission_profile.clone()),
        }
    }
}

pub(super) fn latest_owned_thread_settings(
    history: &[RolloutItem],
    thread_id: ThreadId,
) -> Option<&ThreadSettingsSnapshot> {
    history.iter().rev().find_map(|item| match item {
        RolloutItem::EventMsg(EventMsg::ThreadSettingsApplied(event))
            if event.thread_id == Some(thread_id) =>
        {
            Some(&event.thread_settings)
        }
        _ => None,
    })
}

fn latest_persisted_resume_settings(history: &[RolloutItem]) -> Option<PersistedResumeSettings> {
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
                unnamed_permission_profile: None,
            }),
            RolloutItem::EventMsg(EventMsg::ThreadSettingsApplied(event)) => {
                // History without an identifiable owner can supply a profile ID, but
                // its raw filesystem grants may belong to another thread or executor.
                let settings = &event.thread_settings;
                Some(PersistedResumeSettings {
                    approval_policy: settings.approval_policy,
                    approvals_reviewer: Some(settings.approvals_reviewer),
                    active_permission_profile: settings.active_permission_profile.clone(),
                    unnamed_permission_profile: None,
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
