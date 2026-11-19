//! Tracks attempts on native spawned children, including across suspend and resume.
//! Each regular attempt records at most one end, even if suspension is followed by shutdown.

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_extension_api::ExtensionData;
use codex_history::InitialHistory;
use codex_otel::SubagentTurnStatus;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;

use crate::session::turn_context::TurnContext;

enum SpawnedThread {
    New { first_turn_pending: AtomicBool },
    Resumed,
}

struct ActiveSubagentTurn(&'static str);

pub(crate) fn register_thread(
    thread_store: &ExtensionData,
    source: &SessionSource,
    history: &InitialHistory,
) {
    if !matches!(
        source,
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn { .. })
    ) {
        return;
    }
    let thread = match history {
        InitialHistory::Resumed(_) => SpawnedThread::Resumed,
        // A full fork can already contain many turns from its parent.
        InitialHistory::New | InitialHistory::Cleared | InitialHistory::Forked(_) => {
            SpawnedThread::New {
                first_turn_pending: AtomicBool::new(true),
            }
        }
    };
    thread_store.insert(thread);
}

pub(crate) fn record_start(thread_store: &ExtensionData, turn: &TurnContext) {
    let Some(thread) = thread_store.get::<SpawnedThread>() else {
        return;
    };
    let kind = match thread.as_ref() {
        SpawnedThread::New { first_turn_pending }
            if first_turn_pending.swap(false, Ordering::Relaxed) =>
        {
            "initial"
        }
        SpawnedThread::New { .. } => "follow_up",
        // On reload the attempt can be a recovery of an existing turn ID.
        SpawnedThread::Resumed => "resumed_thread",
    };
    turn.extension_data.insert(ActiveSubagentTurn(kind));
    turn.session_telemetry.record_multi_agent_turn_lifecycle(
        &turn.sub_id,
        kind,
        SubagentTurnStatus::Started,
    );
}

pub(crate) fn record_terminal_event(turn: &TurnContext, event: &EventMsg) {
    let status = match event {
        EventMsg::TurnComplete(event) => match &event.error {
            Some(error) => SubagentTurnStatus::Failed(error.codex_error_info.as_ref()),
            None => SubagentTurnStatus::Completed,
        },
        EventMsg::TurnAborted(event) => SubagentTurnStatus::Interrupted {
            reason: &event.reason,
            error: event
                .error
                .as_ref()
                .and_then(|error| error.codex_error_info.as_ref()),
        },
        _ => return,
    };
    record_end(turn, status);
}

pub(crate) fn record_suspension(turn: &TurnContext) {
    record_end(turn, SubagentTurnStatus::Suspended);
}

fn record_end(turn: &TurnContext, status: SubagentTurnStatus<'_>) {
    if let Some(kind) = turn.extension_data.remove::<ActiveSubagentTurn>() {
        turn.session_telemetry
            .record_multi_agent_turn_lifecycle(&turn.sub_id, kind.0, status);
    }
}
