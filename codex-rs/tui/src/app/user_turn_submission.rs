//! User-turn submission, including permission resolution and active-turn steer races.
//!
//! This is the single app-server submission path for both new user turns and steering.

use super::*;
use crate::app_command::CloudSubmissionIntent;
use crate::app_server_session::UnsupportedLegacyPermissionProfile;
use crate::app_server_session::user_message_submission::TurnStartSettings;
use crate::app_server_session::user_message_submission::UserMessageAdmission;
use crate::app_server_session::user_message_submission::UserMessageSubmissionFailure;

pub(super) async fn try_submit_user_turn(
    app: &mut App,
    app_server: &mut AppServerSession,
    thread_id: ThreadId,
    op: &AppCommand,
) -> Result<bool> {
    let AppCommand::UserTurn {
        turn_trigger,
        cloud_submission,
        client_user_message_id,
        ..
    } = op
    else {
        return Ok(false);
    };
    let intent = if *turn_trigger == "user" {
        match (*cloud_submission, app_server.thread_params_mode()) {
            (
                CloudSubmissionIntent::QuestionAnswer { during_turn },
                crate::ThreadParamsMode::Cloud,
            ) => {
                if during_turn {
                    CloudSubmissionIntent::StartOrSteer
                } else {
                    CloudSubmissionIntent::StartIfIdle
                }
            }
            (CloudSubmissionIntent::QuestionAnswer { .. }, _) => {
                CloudSubmissionIntent::UseKnownTurn
            }
            (intent, _) => intent,
        }
    } else {
        CloudSubmissionIntent::UseKnownTurn
    };
    let original_active = app.active_turn_id_for_thread(thread_id).await;
    let mut active = original_active.clone();
    let preflight = |model: &str| {
        let enabled = app.chat_widget.daybreak_enabled
            && !app.chat_widget.side_conversation_active()
            && !app.side_threads.contains_key(&thread_id);
        crate::daybreak::program_for_turn(
            &app.chat_widget.model_catalog().models,
            model,
            app.chat_widget.daybreak_account_eligible(),
            enabled,
        )
    };
    let start_settings = || turn_start_settings(app, thread_id, op);
    let result = app_server
        .submit_user_message(
            thread_id,
            op,
            intent,
            &mut active,
            preflight,
            start_settings,
        )
        .await;
    if active != original_active
        && let Some(channel) = app.thread_event_channels.get(&thread_id)
    {
        let mut store = channel.store.lock().await;
        if store.active_turn_id() == original_active.as_deref() {
            if let Some(active) = active {
                store.set_active_turn_id(active);
            } else {
                store.clear_active_turn_id();
            }
        }
    }

    let ordinary = intent != CloudSubmissionIntent::UseKnownTurn;
    if let Err(UserMessageSubmissionFailure::Unhandled(error)) = result {
        return Err(error);
    }
    if app.active_thread_id != Some(thread_id) || app.chat_widget.thread_id() != Some(thread_id) {
        if let Err(error) = result {
            tracing::warn!(%thread_id, ?error, "User submission failed in a background thread");
        }
        return Ok(true);
    }

    match result {
        Ok((admission, selection)) => {
            if selection.is_some() {
                app.chat_widget.cloud_picker_selection = selection;
            }
            if !matches!(&admission, UserMessageAdmission::Queued(_)) {
                crate::startup_recovery::acknowledged(client_user_message_id);
            }
            if ordinary || !matches!(&admission, UserMessageAdmission::Steered(_)) {
                app.chat_widget
                    .user_message_admitted(admission, op, client_user_message_id);
            }
        }
        Err(UserMessageSubmissionFailure::Preflight(message)) if !ordinary => {
            if !app.chat_widget.handle_turn_start_rejection(message.clone()) {
                app.chat_widget.add_error_message(message);
            }
        }
        Err(UserMessageSubmissionFailure::Preflight(message)) => {
            app.chat_widget
                .restore_rejected_user_message(client_user_message_id, message);
        }
        Err(UserMessageSubmissionFailure::Rejected(error)) => {
            if error
                .downcast_ref::<UnsupportedLegacyPermissionProfile>()
                .is_some()
            {
                app.chat_widget
                    .set_queue_autosend_suppressed(/*suppressed*/ true);
            }
            app.chat_widget
                .restore_rejected_user_message(client_user_message_id, format!("{error:#}"));
        }
        Err(UserMessageSubmissionFailure::Unconfirmed { error, reconnect }) => {
            app.chat_widget
                .hold_unconfirmed_user_message(client_user_message_id);
            if !reconnect || !app.recover_transport_error(&error) {
                let label = if reconnect {
                    "Submission"
                } else {
                    "Cloud submission"
                };
                app.chat_widget
                    .add_error_message(format!("{label} could not be confirmed: {error}"));
            }
        }
        Err(UserMessageSubmissionFailure::Unhandled(_)) => unreachable!("returned above"),
    }
    Ok(true)
}

fn turn_start_settings<'a>(
    app: &'a App,
    thread_id: ThreadId,
    op: &AppCommand,
) -> TurnStartSettings<'a> {
    let AppCommand::UserTurn {
        approval_policy,
        approvals_reviewer,
        ..
    } = op
    else {
        unreachable!("turn/start requires a user turn");
    };
    let config = app.chat_widget.config_ref();
    let selected_profile = app.pending_server_profiles.get(&thread_id).or_else(|| {
        app.agents_overview
            .requested_permission_profiles
            .get(&thread_id)
    });
    let selected_active =
        selected_profile.map(|profile| ActivePermissionProfile::new(profile.profile_id.clone()));
    // Ordinary turns inherit the server reviewer; echoing its effective value loses the request.
    let (turn_approval_policy, turn_approvals_reviewer) = if let Some(profile) = selected_profile {
        (
            profile.approval_policy,
            profile.approvals_reviewer.map(Into::into),
        )
    } else if app.app_server_target.thread_params_mode()
        == crate::app_server_session::ThreadParamsMode::Remote
    {
        (
            Some(config.permissions.approval_policy.value().into()),
            approvals_reviewer.map(Into::into),
        )
    } else {
        (Some(*approval_policy), approvals_reviewer.map(Into::into))
    };
    // Only explicit choices may override the saved server profile.
    let explicit_profile = app
        .runtime_permission_profile_override
        .as_ref()
        .filter(|profile| {
            profile.turn_override == RuntimePermissionProfileTurnOverride::LegacySandbox
        });
    let permissions_override = App::turn_permissions_override_from_config(
        config,
        selected_active.as_ref().or_else(|| {
            explicit_profile.and_then(|profile| profile.active_permission_profile.as_ref())
        }),
        explicit_profile.and_then(RuntimePermissionProfileOverride::turn_permission_profile),
    );
    TurnStartSettings {
        approval_policy: turn_approval_policy,
        approvals_reviewer: turn_approvals_reviewer,
        permissions: permissions_override,
        workspace_roots: config.permissions.user_visible_workspace_roots(),
    }
}
