//! Read-only snapshots of retained command sessions, including uncollected exits.
//! Handles stay private until their initial exec_command collection finishes.

use super::UnifiedExecProcessManager;
use crate::context::world_state::CommandSession;
use crate::context::world_state::CommandSessionStatus;
use crate::context::world_state::CommandSessionsState;
use std::sync::atomic::Ordering;

pub(crate) async fn command_sessions_state(
    manager: &UnifiedExecProcessManager,
) -> CommandSessionsState {
    let store = manager.process_store.lock().await;
    CommandSessionsState::new(
        store
            .processes
            .values()
            .filter(|entry| !entry.initial_exec_command_active.load(Ordering::Acquire))
            .map(|entry| CommandSession {
                session_id: entry.process_id,
                status: if entry.process.has_exited() {
                    CommandSessionStatus::Exited
                } else {
                    CommandSessionStatus::Running
                },
                in_use: entry.process.interaction_lock().try_lock_owned().is_err(),
            }),
    )
}
