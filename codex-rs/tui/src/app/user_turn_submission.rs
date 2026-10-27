//! User-turn submission, including permission resolution and active-turn steer races.
//!
//! This is the single app-server submission path for both new user turns and steering.

use super::*;

pub(super) async fn try_submit_user_turn(
    app: &mut App,
    app_server: &mut AppServerSession,
    thread_id: ThreadId,
    op: &AppCommand,
) -> Result<bool> {
    let AppCommand::UserTurn {
        turn_trigger,
        client_user_message_id,
        items,
        cwd,
        approval_policy,
        approvals_reviewer,
        active_permission_profile: _,
        model,
        effort,
        summary,
        service_tier,
        final_output_json_schema,
        collaboration_mode,
    } = op
    else {
        return Ok(false);
    };
    if let Some(turn_id) = app.active_turn_id_for_thread(thread_id).await {
        if *turn_trigger != "user" {
            color_eyre::eyre::bail!("the fork already has an active turn");
        }
        let mut steer_turn_id = turn_id;
        let mut retried_after_turn_mismatch = false;
        loop {
            match app_server
                .turn_steer(
                    thread_id,
                    steer_turn_id.clone(),
                    client_user_message_id.clone(),
                    items.to_vec(),
                )
                .await
            {
                Ok(_) => {
                    if app.active_thread_id == Some(thread_id)
                        && app.chat_widget.thread_id() == Some(thread_id)
                    {
                        crate::startup_recovery::acknowledged(client_user_message_id);
                    }
                    return Ok(true);
                }
                Err(error) => {
                    if let Some(turn_error) = active_turn_not_steerable_turn_error(&error) {
                        if !app.chat_widget.enqueue_rejected_steer() {
                            app.chat_widget.add_error_message(turn_error.message);
                        }
                        return Ok(true);
                    }
                    match active_turn_steer_race(&error) {
                        Some(ActiveTurnSteerRace::Missing) => {
                            if let Some(channel) = app.thread_event_channels.get(&thread_id) {
                                let mut store = channel.store.lock().await;
                                store.clear_active_turn_id();
                            }
                            break;
                        }
                        Some(ActiveTurnSteerRace::ExpectedTurnMismatch { actual_turn_id })
                            if !retried_after_turn_mismatch && actual_turn_id != steer_turn_id =>
                        {
                            // Review flows can swap the active turn before the TUI
                            // processes the corresponding notification. Retry once with
                            // the server-reported turn id so non-steerable review turns
                            // still fall through to the existing queueing behavior.
                            if let Some(channel) = app.thread_event_channels.get(&thread_id) {
                                let mut store = channel.store.lock().await;
                                store.set_active_turn_id(actual_turn_id.clone());
                            }
                            steer_turn_id = actual_turn_id;
                            retried_after_turn_mismatch = true;
                        }
                        Some(ActiveTurnSteerRace::ExpectedTurnMismatch { actual_turn_id }) => {
                            if let Some(channel) = app.thread_event_channels.get(&thread_id) {
                                let mut store = channel.store.lock().await;
                                store.set_active_turn_id(actual_turn_id);
                            }
                            return Err(error.into());
                        }
                        None => return Err(error.into()),
                    }
                }
            }
        }
    }
    let eligible_account = app.chat_widget.daybreak_account_eligible();
    let enabled = app.chat_widget.daybreak_enabled
        && !app.chat_widget.side_conversation_active()
        && !app.side_threads.contains_key(&thread_id);
    let cyber_access_program = match crate::daybreak::program_for_turn(
        &app.chat_widget.model_catalog().models,
        model,
        eligible_account,
        enabled,
    ) {
        Ok(program) => program,
        Err(message) => {
            if !app.chat_widget.handle_turn_start_rejection(message.clone()) {
                app.chat_widget.add_error_message(message);
            }
            return Ok(true);
        }
    };
    let config = app.chat_widget.config_ref();
    let selected_profile = app.pending_server_profiles.get(&thread_id).or_else(|| {
        app.agents_overview
            .requested_permission_profiles
            .get(&thread_id)
    });
    let selected_active =
        selected_profile.map(|profile| ActivePermissionProfile::new(profile.profile_id.clone()));
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
            Some(config.approvals_reviewer.into()),
        )
    } else {
        (
            Some(*approval_policy),
            Some(
                approvals_reviewer
                    .unwrap_or(config.approvals_reviewer)
                    .into(),
            ),
        )
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
    let response = app_server
        .turn_start(
            thread_id,
            turn_trigger,
            client_user_message_id.clone(),
            items.to_vec(),
            cwd.clone(),
            turn_approval_policy,
            turn_approvals_reviewer,
            permissions_override,
            config.permissions.user_visible_workspace_roots(),
            model.to_string(),
            effort.clone(),
            *summary,
            service_tier.clone(),
            collaboration_mode.clone(),
            final_output_json_schema.clone(),
            cyber_access_program.map(Into::into),
        )
        .await?;
    if app.active_thread_id == Some(thread_id) && app.chat_widget.thread_id() == Some(thread_id) {
        crate::startup_recovery::acknowledged(client_user_message_id);
        app.chat_widget
            .record_safety_buffering_turn(response.turn.id, op);
    }
    Ok(true)
}
