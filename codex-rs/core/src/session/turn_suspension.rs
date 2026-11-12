use super::handlers;
use super::session::Session;
use crate::state::TaskKind;
use crate::thread_manager::AgentTreeShutdownFailure;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::turn_input::SuspendTurnOutcome;
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;

pub(super) async fn suspend_turn_and_shutdown(
    session: &Arc<Session>,
    submission_id: String,
) -> CodexResult<SuspendTurnOutcome> {
    {
        let active = session.active_turn.lock().await;
        let Some(task) = active.as_ref().and_then(|turn| turn.task.as_ref()) else {
            return Ok(SuspendTurnOutcome::NotActive);
        };
        if task.kind != TaskKind::Regular {
            return Ok(SuspendTurnOutcome::UnsupportedTask);
        }
    }

    let live_thread = session
        .live_thread_for_persistence("suspend an unfinished root turn")
        .map_err(|error| CodexErr::Fatal(error.to_string()))?;
    // Flush before canceling execution so a persistence failure leaves the original turn running.
    live_thread.flush().await.map_err(|error| {
        CodexErr::Fatal(format!("flush before root turn suspension failed: {error}"))
    })?;

    // The flush can yield while the active turn completes or changes. Recheck its
    // kind under the same lock used to remove it.
    let mut turn = {
        let mut active = session.active_turn.lock().await;
        let Some(active_turn) = active.as_ref() else {
            return Ok(SuspendTurnOutcome::NotActive);
        };
        let Some(task) = active_turn.task.as_ref() else {
            return Ok(SuspendTurnOutcome::NotActive);
        };
        if task.kind != TaskKind::Regular {
            return Ok(SuspendTurnOutcome::UnsupportedTask);
        }
        active.take().ok_or_else(|| {
            CodexErr::Fatal("accepted root turn suspension had no running turn".to_string())
        })?
    };

    let task = turn.task.take().ok_or_else(|| {
        CodexErr::Fatal("accepted root turn suspension had no running task".to_string())
    })?;
    let turn_id = task.turn_context.sub_id.clone();
    // A suspended task emits no terminal outcome.
    drop(task.outcome_handoff);
    // Normal shutdown records a terminal turn event, preventing another worker from
    // recovering this turn under its original ID. Cancel the task without that event.
    task.cancellation_token.cancel();
    task.turn_context
        .turn_metadata_state
        .cancel_git_enrichment_task();
    let mut task_handle = task.handle.detach();
    match tokio::time::timeout(
        Duration::from_millis(crate::tasks::GRACEFULL_INTERRUPTION_TIMEOUT_MS),
        &mut task_handle,
    )
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            warn!(thread_id = %session.thread_id, %error, "suspended turn task exited abnormally");
        }
        Err(_) => {
            warn!(
                thread_id = %session.thread_id,
                "suspended turn task did not stop gracefully; aborting it"
            );
            task_handle.abort();
            let _ = task_handle.await;
        }
    }
    // Save any initial input the task had not claimed so a new worker can recover the turn.
    let recorded = session
        .record_pending_initial_input(
            &task.turn_context,
            &turn.turn_state,
            task.pending_initial_input,
            /*abort_reason*/ None,
        )
        .await;
    // Other pending input and interactive waiters remain best effort across handoff.
    session.input_queue.clear_pending(&turn).await;

    // The root task has stopped without a terminal event. Fence new starts and stop every
    // other local session, even if the suspension caller disconnects during cleanup.
    // The host owns the separate tree-cleanup wait and any persistence-writer fencing.
    session.services.local_agent_runtime.request_shutdown();

    // Stop all producers before flushing their final history and closing its writer.
    // If either persistence step fails, do not report success: the current worker
    // retains ownership until worker-failure recovery can take responsibility.
    handlers::shutdown_session_runtime(session).await;
    live_thread.flush().await.map_err(|error| {
        CodexErr::Fatal(format!("flush after root turn suspension failed: {error}"))
    })?;
    live_thread.shutdown().await.map_err(|error| {
        CodexErr::Fatal(format!("close suspended root turn writer failed: {error}"))
    })?;
    recorded?;
    // Announce completion only after extension cleanup and writer closure so a
    // replacement worker cannot write the same thread concurrently.
    session
        .deliver_event_raw(Event {
            id: submission_id,
            msg: EventMsg::ShutdownComplete,
        })
        .await;
    Ok(SuspendTurnOutcome::Suspended { turn_id })
}

pub(super) async fn stop_tree_member(session: &Arc<Session>) -> bool {
    let runtime = &session.services.local_agent_runtime;
    let submission_id = super::new_submission_id();
    if let Some(root) = runtime.suspension_root()
        && session.live_thread().is_some()
        && (root == session.thread_id
            || session.multi_agent_version() == Some(MultiAgentVersion::V2)
                && session
                    .state
                    .lock()
                    .await
                    .session_configuration
                    .session_source
                    .get_agent_path()
                    .is_some())
    {
        match suspend_turn_and_shutdown(session, submission_id.clone()).await {
            Ok(SuspendTurnOutcome::Suspended { turn_id }) => {
                runtime.record_suspended_turn(session.thread_id, turn_id);
                return true;
            }
            Ok(SuspendTurnOutcome::NotActive | SuspendTurnOutcome::UnsupportedTask) => {}
            Err(error) => {
                runtime.record_shutdown_failure(AgentTreeShutdownFailure::operation_failed(
                    "tree_suspension",
                    "suspend_regular_turn",
                    Some(session.thread_id),
                    "suspension_failed",
                ));
                warn!(thread_id = %session.thread_id, %error, "failed to suspend agent turn");
            }
        }
    }
    handlers::shutdown(session, submission_id).await
}
