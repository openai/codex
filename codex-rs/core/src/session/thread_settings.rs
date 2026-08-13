//! Handles persistent thread-settings updates shared by standalone settings
//! submissions and turn-input submission.

use super::session::Session;
use super::session::SessionSettingsUpdate;
use crate::config::ConstraintResult;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadSettingsAppliedEvent;
use codex_protocol::protocol::ThreadSettingsOverrides;
use std::sync::Arc;

/// Applies standalone thread settings and reports invalid overrides through the
/// normal event stream.
pub(super) async fn update(
    session: &Arc<Session>,
    submission_id: String,
    overrides: ThreadSettingsOverrides,
) {
    let updates = prepare_update(session, overrides).await;
    if let Err(error) = apply_update(session, submission_id.clone(), updates).await {
        session
            .send_event_raw(Event {
                id: submission_id,
                msg: EventMsg::Error(ErrorEvent {
                    message: format!("invalid thread settings override: {error}"),
                    codex_error_info: Some(CodexErrorInfo::BadRequest),
                }),
            })
            .await;
    }
}

/// Converts protocol overrides into the internal settings update shape.
pub(super) async fn prepare_update(
    session: &Session,
    overrides: ThreadSettingsOverrides,
) -> SessionSettingsUpdate {
    let ThreadSettingsOverrides {
        environments,
        profile_workspace_roots,
        approval_policy,
        approvals_reviewer,
        sandbox_policy,
        permission_profile,
        active_permission_profile,
        windows_sandbox_level,
        model,
        effort,
        summary,
        service_tier,
        collaboration_mode,
        personality,
    } = overrides;
    let collaboration_mode = match collaboration_mode {
        Some(collaboration_mode) => collaboration_mode,
        None => {
            let state = session.state.lock().await;
            // Model and reasoning effort live in CollaborationMode settings today, so
            // partial thread-settings updates refresh those fields on the active mode.
            state
                .session_configuration
                .collaboration_mode
                .with_updates(model, effort, /*developer_instructions*/ None)
        }
    };
    SessionSettingsUpdate {
        environments,
        profile_workspace_roots,
        approval_policy,
        approvals_reviewer,
        sandbox_policy,
        permission_profile,
        active_permission_profile,
        windows_sandbox_level,
        collaboration_mode: Some(collaboration_mode),
        reasoning_summary: summary,
        service_tier,
        personality,
        ..Default::default()
    }
}

/// Applies persistent settings and emits the resulting effective snapshot.
pub(super) async fn apply_update(
    session: &Session,
    submission_id: String,
    updates: SessionSettingsUpdate,
) -> ConstraintResult<()> {
    session.update_settings(updates).await?;
    emit_applied(session, submission_id).await;
    Ok(())
}

/// Emits the effective thread settings after a successful update.
pub(super) async fn emit_applied(session: &Session, submission_id: String) {
    let msg = applied_event(session).await;
    session
        .send_event_raw_without_materializing_rollout(Event {
            id: submission_id,
            msg,
        })
        .await;
}

/// Builds the effective thread-settings event used by live updates and
/// synthesized fork history.
pub(super) async fn applied_event(session: &Session) -> EventMsg {
    let snapshot = session.thread_config_snapshot().await;
    EventMsg::ThreadSettingsApplied(ThreadSettingsAppliedEvent {
        thread_settings: snapshot.into_thread_settings_snapshot(),
    })
}
