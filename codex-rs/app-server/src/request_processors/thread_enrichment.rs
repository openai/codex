use super::turn_processor::can_accept_direct_input;
use crate::thread_status::ThreadWatchManager;
use crate::thread_status::resolve_thread_status;
use codex_app_server_protocol::SessionSource;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadStatus;
use codex_core::ThreadManager;
use codex_protocol::ThreadId;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::SubAgentSource;

pub(super) async fn enrich_loaded_threads<T>(
    thread_manager: &ThreadManager,
    thread_watch_manager: &ThreadWatchManager,
    threads: &mut [T],
    mut as_thread: impl FnMut(&mut T) -> &mut Thread,
) {
    let statuses = thread_watch_manager
        .loaded_statuses_for_threads(
            threads
                .iter_mut()
                .map(&mut as_thread)
                .map(|thread| thread.id.clone()),
        )
        .await;

    futures::future::join_all(threads.iter_mut().map(as_thread).map(|thread| {
        let statuses = &statuses;
        async move {
            let watched_status = statuses.get(&thread.id);
            if let Some(status) = watched_status {
                thread.status = status.clone();
            }

            if !matches!(
                &thread.source,
                SessionSource::SubAgent(SubAgentSource::ThreadSpawn { .. })
            ) || matches!(watched_status, Some(ThreadStatus::NotLoaded))
            {
                return;
            }

            let Ok(thread_id) = ThreadId::from_string(&thread.id) else {
                return;
            };
            let Ok(loaded_thread) = thread_manager.get_thread(thread_id).await else {
                return;
            };
            match loaded_thread.agent_status().await {
                AgentStatus::Running => {
                    if watched_status.is_none() {
                        thread.status = resolve_thread_status(
                            ThreadStatus::Idle,
                            /*has_in_progress_turn*/ true,
                        );
                    }
                }
                AgentStatus::PendingInit | AgentStatus::Interrupted | AgentStatus::Completed(_) => {
                    if watched_status.is_none() {
                        thread.status = ThreadStatus::Idle;
                    }
                }
                AgentStatus::Errored(_) => {
                    thread.status = ThreadStatus::SystemError;
                }
                AgentStatus::Shutdown | AgentStatus::NotFound => {
                    thread.status = ThreadStatus::NotLoaded;
                    return;
                }
            }
            let config_snapshot = loaded_thread.config_snapshot().await;
            thread.can_accept_direct_input = Some(can_accept_direct_input(
                loaded_thread.multi_agent_version(),
                &config_snapshot.session_source,
            ));
        }
    }))
    .await;
}
