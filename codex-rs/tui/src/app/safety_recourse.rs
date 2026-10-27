//! Check recourse availability and submit selected follow-ups on verified forks.

use super::session_lifecycle::ThreadAttachPresentation;
use super::*;
use crate::app_server_session::ForkGoalContinuation;
use color_eyre::eyre::OptionExt;
use color_eyre::eyre::ensure;

pub(super) fn supports_server(app_server: &AppServerSession) -> bool {
    // Remote servers before 0.151.0 silently ignore the required turn trigger.
    let minimum = semver::Version::new(/*major*/ 0, /*minor*/ 151, /*patch*/ 0);
    let Some(version) = app_server.server_version() else {
        return app_server.uses_embedded_app_server();
    };
    semver::Version::parse(version).is_ok_and(|version| version >= minimum)
}

pub(super) async fn continue_with_rewrite(
    app: &mut App,
    tui: &mut tui::Tui,
    app_server: &mut AppServerSession,
    thread_id: ThreadId,
    turn_id: String,
    prompt: String,
) {
    if !supports_server(app_server)
        || app.active_thread_id != Some(thread_id)
        || app.primary_thread_id != Some(thread_id)
        || app.chat_widget.last_turn_id() != Some(turn_id.as_str())
        || app.chat_widget.is_user_turn_pending_or_running()
        || app.chat_widget.has_queued_follow_up_messages()
    {
        return;
    }
    let mut attached_fork = None;
    let result = async {
        ensure!(
            !app.pending_server_profiles.contains_key(&thread_id),
            "permissions are still updating"
        );
        let source = app_server
            .thread_read(thread_id, /*include_turns*/ false)
            .await?;
        let source_turns = app_server
            .thread_turns_page(thread_id, /*cursor*/ None, /*limit*/ 2)
            .await?;
        let is_blocked_turn = |turn: &Turn| turn.id == turn_id && turn.status == TurnStatus::Failed;
        ensure!(
            !source.ephemeral
                && source_turns.data.first().is_some_and(is_blocked_turn)
                && (source_turns.data.len() > 1 || source_turns.next_cursor.is_none()),
            "the blocked turn is no longer the latest turn"
        );
        let mut config = app.chat_widget.config_ref().clone();
        config.model = Some(app.chat_widget.current_model().to_string());
        config.model_reasoning_effort = app.chat_widget.current_reasoning_effort();
        config.daybreak_enabled = app.chat_widget.daybreak_enabled;
        let selected_profile = app.selected_server_profile(thread_id);
        let started = app_server
            .fork_thread_at(
                &app.local_settings,
                config.clone(),
                thread_id,
                /*last_turn_id*/ None,
                /*before_turn_id*/ Some(turn_id.clone()),
                // Deferring also copies the source's current goal, beyond the turn cutoff.
                ForkGoalContinuation::StartIfIdle,
                selected_profile.as_ref(),
            )
            .await?;
        let fork_id = started.session.thread_id;
        let fork_turns = app_server
            .thread_turns_page(fork_id, /*cursor*/ None, /*limit*/ 1)
            .await?;
        let latest = app_server
            .thread_turns_page(thread_id, /*cursor*/ None, /*limit*/ 1)
            .await?;
        ensure!(
            !started.blocks_direct_input
                && started.session.forked_from_id == Some(thread_id)
                && latest.data.first().is_some_and(is_blocked_turn)
                && (!fork_turns.data.is_empty() || fork_turns.next_cursor.is_none())
                && fork_turns.data.first().map(|turn| &turn.id)
                    == source_turns.data.get(1).map(|turn| &turn.id),
            "the fork boundary could not be verified"
        );
        app.config = config;
        if let Some(input) = app.chat_widget.capture_thread_input_state() {
            app.agents_overview.input_states.insert(thread_id, input);
        }
        app.detach_current_thread_for_fork(app_server, fork_id)
            .await;
        attached_fork = Some(fork_id);
        app.replace_chat_widget_with_app_server_thread(
            tui,
            started,
            ThreadAttachPresentation::SessionLineage,
            /*initial_user_message*/ None,
        )
        .await?;
        if selected_profile.is_some() {
            app.adopt_inherited_server_selection();
        }
        ensure!(!app.thread_unavailable(fork_id), "the fork is unavailable");
        let turn = app
            .chat_widget
            .prepare_safety_recourse_rewrite(&prompt)
            .ok_or_eyre("the follow-up could not be submitted")?;
        app.submit_thread_op(app_server, fork_id, turn).await
    }
    .await;
    match result {
        Ok(()) if app.chat_widget.is_user_turn_pending_or_running() => return,
        Ok(()) => {} // Submission was rejected and already showed its error.
        Err(error) if app.recover_transport_error(&error) && attached_fork.is_some() => return,
        Err(_) => {
            let text = history_cell::sanitize_user_text((&prompt).into());
            let message = format!("Couldn’t start a new chat for this rewrite. {text}");
            if !app.chat_widget.handle_turn_start_rejection(message.clone()) {
                app.chat_widget.add_error_message(message);
            }
        }
    }
    if attached_fork.is_some_and(|fork_id| app.active_thread_id == Some(fork_id)) {
        app.chat_widget
            .restore_user_message_to_composer(prompt.into());
    }
}
