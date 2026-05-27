//! Scheduling glue for extension-owned turns that should run when a session is idle.

use std::sync::Arc;

use crate::session::TurnInput;
use crate::session::session::Session;
use crate::state::ActiveTurn;

use super::RegularTask;

pub(super) fn schedule_turn(session: &Arc<Session>) {
    if session
        .services
        .extensions
        .idle_turn_contributors()
        .is_empty()
    {
        return;
    }

    let session = Arc::clone(session);
    let _handle = tokio::spawn(async move {
        maybe_start_turn(session).await;
    });
}

async fn maybe_start_turn(session: Arc<Session>) {
    // Give queued user-visible work the first chance to wake the session.
    session.maybe_start_turn_for_pending_work().await;
    if has_active_or_pending_work(&session).await {
        return;
    }

    // Ask extensions for one idle turn only while the session still looks quiet.
    let Some(input) = next_idle_turn_input(&session).await else {
        return;
    };

    // Extension callbacks can race with user input or mailbox delivery. Re-check before
    // claiming the idle slot, and let the normal pending-work path handle anything that appeared.
    if has_active_or_pending_work(&session).await {
        session.maybe_start_turn_for_pending_work().await;
        return;
    }

    // Reserve the active turn after the final quiet check so another task cannot start while
    // the turn context is being built.
    {
        let mut active_turn = session.active_turn.lock().await;
        if active_turn.is_some() {
            return;
        }
        *active_turn = Some(ActiveTurn::default());
    }

    // Treat extension-provided items as the new turn's initial input rather than stashing them
    // in turn-state pending input; this keeps rollback logic out of the scheduler.
    let turn_context = session
        .new_default_turn_with_sub_id(uuid::Uuid::new_v4().to_string())
        .await;
    session
        .maybe_emit_unknown_model_warning_for_turn(turn_context.as_ref())
        .await;
    session
        .start_task(turn_context, input, RegularTask::new())
        .await;
}

async fn has_active_or_pending_work(session: &Session) -> bool {
    session.active_turn.lock().await.is_some()
        || session
            .input_queue
            .has_queued_response_items_for_next_turn()
            .await
        || session.input_queue.has_trigger_turn_mailbox_items().await
}

async fn next_idle_turn_input(session: &Session) -> Option<Vec<TurnInput>> {
    let collaboration_mode = session.collaboration_mode().await;
    for contributor in session.services.extensions.idle_turn_contributors() {
        let Some(items) = contributor
            .next_idle_turn(codex_extension_api::IdleTurnInput {
                collaboration_mode: &collaboration_mode,
                session_store: &session.services.session_extension_data,
                thread_store: &session.services.thread_extension_data,
            })
            .await
        else {
            continue;
        };
        if !items.is_empty() {
            return Some(
                items
                    .into_iter()
                    .map(TurnInput::ResponseInputItem)
                    .collect(),
            );
        }
    }

    None
}
