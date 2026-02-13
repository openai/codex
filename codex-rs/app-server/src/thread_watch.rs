#[cfg(test)]
use crate::outgoing_message::OutgoingEnvelope;
#[cfg(test)]
use crate::outgoing_message::OutgoingMessage;
use crate::outgoing_message::OutgoingMessageSender;
#[cfg(test)]
use codex_app_server_protocol::LoadedThreadEntry;
use codex_app_server_protocol::LoadedThreadStatus;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadActiveFlag;
use codex_app_server_protocol::ThreadStatusChangedNotification;
use codex_app_server_protocol::ThreadTerminalOutcome;
use std::collections::HashMap;
#[cfg(test)]
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
#[cfg(test)]
use tokio::sync::mpsc;

#[derive(Clone)]
pub(crate) struct ThreadWatchManager {
    state: Arc<Mutex<ThreadWatchState>>,
    outgoing: Option<Arc<OutgoingMessageSender>>,
}

pub(crate) struct ThreadWatchActiveGuard {
    manager: ThreadWatchManager,
    thread_id: String,
    guard_type: ThreadWatchActiveGuardType,
    handle: tokio::runtime::Handle,
}

impl ThreadWatchActiveGuard {
    fn new(
        manager: ThreadWatchManager,
        thread_id: String,
        guard_type: ThreadWatchActiveGuardType,
    ) -> Self {
        Self {
            manager,
            thread_id,
            guard_type,
            handle: tokio::runtime::Handle::current(),
        }
    }
}

impl Drop for ThreadWatchActiveGuard {
    fn drop(&mut self) {
        let manager = self.manager.clone();
        let thread_id = self.thread_id.clone();
        let guard_type = self.guard_type;
        self.handle.spawn(async move {
            manager
                .note_active_guard_released(thread_id, guard_type)
                .await;
        });
    }
}

#[derive(Clone, Copy)]
enum ThreadWatchActiveGuardType {
    Permission,
    UserInput,
}

impl Default for ThreadWatchManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreadWatchManager {
    pub(crate) fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ThreadWatchState::default())),
            outgoing: None,
        }
    }

    pub(crate) fn new_with_outgoing(outgoing: Arc<OutgoingMessageSender>) -> Self {
        Self {
            state: Arc::new(Mutex::new(ThreadWatchState::default())),
            outgoing: Some(outgoing),
        }
    }

    pub(crate) async fn upsert_thread(&self, thread: Thread) {
        self.mutate_and_publish(move |state| state.upsert_thread(thread.id))
            .await;
    }

    pub(crate) async fn remove_thread(&self, thread_id: &str) {
        let thread_id = thread_id.to_string();
        self.mutate_and_publish(move |state| state.remove_thread(&thread_id))
            .await;
    }

    pub(crate) async fn loaded_status_for_thread(&self, thread_id: &str) -> LoadedThreadStatus {
        self.state.lock().await.loaded_status_for_thread(thread_id)
    }

    #[cfg(test)]
    pub(crate) async fn loaded_entries_for_threads(
        &self,
        threads: Vec<Thread>,
    ) -> Vec<LoadedThreadEntry> {
        let state = self.state.lock().await;
        threads
            .into_iter()
            .map(|thread| LoadedThreadEntry {
                status: state.loaded_status_for_thread(&thread.id),
                thread,
            })
            .collect()
    }

    pub(crate) async fn loaded_statuses_for_threads(
        &self,
        thread_ids: Vec<String>,
    ) -> HashMap<String, LoadedThreadStatus> {
        let state = self.state.lock().await;
        thread_ids
            .into_iter()
            .map(|thread_id| {
                let status = state.loaded_status_for_thread(&thread_id);
                (thread_id, status)
            })
            .collect()
    }

    pub(crate) async fn note_turn_started(&self, thread_id: &str) {
        self.update_runtime_for_thread(thread_id, |runtime| {
            runtime.running = true;
            runtime.last_terminal_outcome = None;
        })
        .await;
    }

    pub(crate) async fn note_turn_completed(&self, thread_id: &str, failed: bool) {
        self.finalize_thread_outcome(
            thread_id,
            if failed {
                ThreadTerminalOutcome::Failed
            } else {
                ThreadTerminalOutcome::Completed
            },
        )
        .await;
    }

    pub(crate) async fn note_turn_interrupted(&self, thread_id: &str) {
        self.finalize_thread_outcome(thread_id, ThreadTerminalOutcome::Interrupted)
            .await;
    }

    pub(crate) async fn note_thread_shutdown(&self, thread_id: &str) {
        self.finalize_thread_outcome(thread_id, ThreadTerminalOutcome::Shutdown)
            .await;
    }

    async fn finalize_thread_outcome(&self, thread_id: &str, outcome: ThreadTerminalOutcome) {
        self.update_runtime_for_thread(thread_id, move |runtime| {
            runtime.running = false;
            runtime.pending_permission_requests = 0;
            runtime.pending_user_input_requests = 0;
            runtime.last_terminal_outcome = Some(outcome);
        })
        .await;
    }

    pub(crate) async fn note_permission_requested(
        &self,
        thread_id: &str,
    ) -> ThreadWatchActiveGuard {
        self.note_pending_request(thread_id, ThreadWatchActiveGuardType::Permission)
            .await
    }

    pub(crate) async fn note_user_input_requested(
        &self,
        thread_id: &str,
    ) -> ThreadWatchActiveGuard {
        self.note_pending_request(thread_id, ThreadWatchActiveGuardType::UserInput)
            .await
    }

    async fn note_pending_request(
        &self,
        thread_id: &str,
        guard_type: ThreadWatchActiveGuardType,
    ) -> ThreadWatchActiveGuard {
        self.update_runtime_for_thread(thread_id, move |runtime| {
            let counter = Self::pending_counter(runtime, guard_type);
            *counter = counter.saturating_add(1);
            runtime.last_terminal_outcome = None;
        })
        .await;
        ThreadWatchActiveGuard::new(self.clone(), thread_id.to_string(), guard_type)
    }

    async fn mutate_and_publish<F>(&self, mutate: F)
    where
        F: FnOnce(&mut ThreadWatchState) -> Option<ThreadStatusChangedNotification>,
    {
        let notification = {
            let mut state = self.state.lock().await;
            mutate(&mut state)
        };

        if let Some(notification) = notification
            && let Some(outgoing) = &self.outgoing
        {
            outgoing
                .send_server_notification(ServerNotification::ThreadStatusChanged(notification))
                .await;
        }
    }

    async fn note_active_guard_released(
        &self,
        thread_id: String,
        guard_type: ThreadWatchActiveGuardType,
    ) {
        self.update_runtime_for_thread(&thread_id, move |runtime| {
            let counter = Self::pending_counter(runtime, guard_type);
            *counter = counter.saturating_sub(1);
        })
        .await;
    }

    async fn update_runtime_for_thread<F>(&self, thread_id: &str, update: F)
    where
        F: FnOnce(&mut RuntimeFacts),
    {
        let thread_id = thread_id.to_string();
        self.mutate_and_publish(move |state| state.update_runtime(&thread_id, update))
            .await;
    }

    fn pending_counter(
        runtime: &mut RuntimeFacts,
        guard_type: ThreadWatchActiveGuardType,
    ) -> &mut u32 {
        match guard_type {
            ThreadWatchActiveGuardType::Permission => &mut runtime.pending_permission_requests,
            ThreadWatchActiveGuardType::UserInput => &mut runtime.pending_user_input_requests,
        }
    }
}

#[derive(Default)]
struct ThreadWatchState {
    runtime_by_thread_id: HashMap<String, RuntimeFacts>,
}

impl ThreadWatchState {
    fn upsert_thread(&mut self, thread_id: String) -> Option<ThreadStatusChangedNotification> {
        let previous_status = self.status_for(&thread_id);
        self.runtime_by_thread_id
            .entry(thread_id.clone())
            .or_default();
        self.status_changed_notification(thread_id, previous_status)
    }

    fn remove_thread(&mut self, thread_id: &str) -> Option<ThreadStatusChangedNotification> {
        self.runtime_by_thread_id.remove(thread_id);
        None
    }

    fn update_runtime<F>(
        &mut self,
        thread_id: &str,
        mutate: F,
    ) -> Option<ThreadStatusChangedNotification>
    where
        F: FnOnce(&mut RuntimeFacts),
    {
        let previous_status = self.status_for(thread_id);
        let runtime = self
            .runtime_by_thread_id
            .entry(thread_id.to_string())
            .or_default();
        mutate(runtime);
        self.status_changed_notification(thread_id.to_string(), previous_status)
    }

    fn status_for(&self, thread_id: &str) -> Option<LoadedThreadStatus> {
        self.runtime_by_thread_id
            .get(thread_id)
            .map(loaded_thread_status)
    }

    fn loaded_status_for_thread(&self, thread_id: &str) -> LoadedThreadStatus {
        self.status_for(thread_id)
            .unwrap_or(LoadedThreadStatus::Idle)
    }

    fn status_changed_notification(
        &self,
        thread_id: String,
        previous_status: Option<LoadedThreadStatus>,
    ) -> Option<ThreadStatusChangedNotification> {
        let status = self.status_for(&thread_id)?;

        if previous_status.as_ref() == Some(&status) {
            return None;
        }

        Some(ThreadStatusChangedNotification { thread_id, status })
    }
}

#[derive(Clone, Default)]
struct RuntimeFacts {
    running: bool,
    pending_permission_requests: u32,
    pending_user_input_requests: u32,
    last_terminal_outcome: Option<ThreadTerminalOutcome>,
}

fn loaded_thread_status(runtime: &RuntimeFacts) -> LoadedThreadStatus {
    let mut active_flags = Vec::new();
    if runtime.running {
        active_flags.push(ThreadActiveFlag::Running);
    }
    if runtime.pending_permission_requests > 0 {
        active_flags.push(ThreadActiveFlag::WaitingPermission);
    }
    if runtime.pending_user_input_requests > 0 {
        active_flags.push(ThreadActiveFlag::WaitingUserInput);
    }

    if !active_flags.is_empty() {
        return LoadedThreadStatus::Active { active_flags };
    }

    match runtime.last_terminal_outcome {
        Some(outcome) => LoadedThreadStatus::Terminal { outcome },
        None => LoadedThreadStatus::Idle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tokio::time::Duration;
    use tokio::time::timeout;

    const INTERACTIVE_THREAD_ID: &str = "00000000-0000-0000-0000-000000000001";
    const NON_INTERACTIVE_THREAD_ID: &str = "00000000-0000-0000-0000-000000000002";

    #[tokio::test]
    async fn loaded_status_defaults_to_idle_for_untracked_threads() {
        let manager = ThreadWatchManager::new();

        assert_eq!(
            manager
                .loaded_status_for_thread("00000000-0000-0000-0000-000000000003")
                .await,
            LoadedThreadStatus::Idle,
        );
    }

    #[tokio::test]
    async fn tracks_non_interactive_thread_status() {
        let manager = ThreadWatchManager::new();
        manager
            .upsert_thread(test_thread(
                NON_INTERACTIVE_THREAD_ID,
                codex_app_server_protocol::SessionSource::AppServer,
            ))
            .await;

        manager.note_turn_started(NON_INTERACTIVE_THREAD_ID).await;

        assert_eq!(
            manager
                .loaded_status_for_thread(NON_INTERACTIVE_THREAD_ID)
                .await,
            LoadedThreadStatus::Active {
                active_flags: vec![ThreadActiveFlag::Running],
            },
        );
    }

    #[tokio::test]
    async fn status_updates_track_single_thread() {
        let manager = ThreadWatchManager::new();
        manager
            .upsert_thread(test_thread(
                INTERACTIVE_THREAD_ID,
                codex_app_server_protocol::SessionSource::Cli,
            ))
            .await;

        manager.note_turn_started(INTERACTIVE_THREAD_ID).await;
        assert_eq!(
            manager
                .loaded_status_for_thread(INTERACTIVE_THREAD_ID)
                .await,
            LoadedThreadStatus::Active {
                active_flags: vec![ThreadActiveFlag::Running],
            },
        );

        let permission_guard = manager
            .note_permission_requested(INTERACTIVE_THREAD_ID)
            .await;
        assert_eq!(
            manager
                .loaded_status_for_thread(INTERACTIVE_THREAD_ID)
                .await,
            LoadedThreadStatus::Active {
                active_flags: vec![
                    ThreadActiveFlag::Running,
                    ThreadActiveFlag::WaitingPermission,
                ],
            },
        );

        let user_input_guard = manager
            .note_user_input_requested(INTERACTIVE_THREAD_ID)
            .await;
        assert_eq!(
            manager
                .loaded_status_for_thread(INTERACTIVE_THREAD_ID)
                .await,
            LoadedThreadStatus::Active {
                active_flags: vec![
                    ThreadActiveFlag::Running,
                    ThreadActiveFlag::WaitingPermission,
                    ThreadActiveFlag::WaitingUserInput,
                ],
            },
        );

        drop(permission_guard);
        wait_for_status(
            &manager,
            INTERACTIVE_THREAD_ID,
            LoadedThreadStatus::Active {
                active_flags: vec![
                    ThreadActiveFlag::Running,
                    ThreadActiveFlag::WaitingUserInput,
                ],
            },
        )
        .await;

        drop(user_input_guard);
        wait_for_status(
            &manager,
            INTERACTIVE_THREAD_ID,
            LoadedThreadStatus::Active {
                active_flags: vec![ThreadActiveFlag::Running],
            },
        )
        .await;

        manager
            .note_turn_completed(INTERACTIVE_THREAD_ID, false)
            .await;
        assert_eq!(
            manager
                .loaded_status_for_thread(INTERACTIVE_THREAD_ID)
                .await,
            LoadedThreadStatus::Terminal {
                outcome: ThreadTerminalOutcome::Completed,
            },
        );
    }

    #[tokio::test]
    async fn loaded_entries_default_to_idle_for_untracked_threads() {
        let manager = ThreadWatchManager::new();
        manager
            .upsert_thread(test_thread(
                INTERACTIVE_THREAD_ID,
                codex_app_server_protocol::SessionSource::Cli,
            ))
            .await;
        manager.note_turn_started(INTERACTIVE_THREAD_ID).await;

        let entries = manager
            .loaded_entries_for_threads(vec![
                test_thread(
                    INTERACTIVE_THREAD_ID,
                    codex_app_server_protocol::SessionSource::Cli,
                ),
                test_thread(
                    NON_INTERACTIVE_THREAD_ID,
                    codex_app_server_protocol::SessionSource::AppServer,
                ),
            ])
            .await;

        assert_eq!(
            entries,
            vec![
                LoadedThreadEntry {
                    thread: test_thread(
                        INTERACTIVE_THREAD_ID,
                        codex_app_server_protocol::SessionSource::Cli,
                    ),
                    status: LoadedThreadStatus::Active {
                        active_flags: vec![ThreadActiveFlag::Running],
                    },
                },
                LoadedThreadEntry {
                    thread: test_thread(
                        NON_INTERACTIVE_THREAD_ID,
                        codex_app_server_protocol::SessionSource::AppServer,
                    ),
                    status: LoadedThreadStatus::Idle,
                },
            ],
        );
    }

    #[tokio::test]
    async fn status_change_emits_notification() {
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(8);
        let manager = ThreadWatchManager::new_with_outgoing(Arc::new(OutgoingMessageSender::new(
            outgoing_tx,
        )));

        manager
            .upsert_thread(test_thread(
                INTERACTIVE_THREAD_ID,
                codex_app_server_protocol::SessionSource::Cli,
            ))
            .await;
        assert_eq!(
            recv_status_changed_notification(&mut outgoing_rx).await,
            ThreadStatusChangedNotification {
                thread_id: INTERACTIVE_THREAD_ID.to_string(),
                status: LoadedThreadStatus::Idle,
            },
        );

        manager.note_turn_started(INTERACTIVE_THREAD_ID).await;
        assert_eq!(
            recv_status_changed_notification(&mut outgoing_rx).await,
            ThreadStatusChangedNotification {
                thread_id: INTERACTIVE_THREAD_ID.to_string(),
                status: LoadedThreadStatus::Active {
                    active_flags: vec![ThreadActiveFlag::Running],
                },
            },
        );
    }

    async fn wait_for_status(
        manager: &ThreadWatchManager,
        thread_id: &str,
        expected_status: LoadedThreadStatus,
    ) {
        timeout(Duration::from_secs(1), async {
            loop {
                let status = manager.loaded_status_for_thread(thread_id).await;
                if status == expected_status {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("timed out waiting for status");
    }

    async fn recv_status_changed_notification(
        outgoing_rx: &mut mpsc::Receiver<OutgoingEnvelope>,
    ) -> ThreadStatusChangedNotification {
        let envelope = timeout(Duration::from_secs(1), outgoing_rx.recv())
            .await
            .expect("timed out waiting for outgoing notification")
            .expect("outgoing channel closed unexpectedly");
        let OutgoingEnvelope::Broadcast { message } = envelope else {
            panic!("expected broadcast notification");
        };
        let OutgoingMessage::AppServerNotification(ServerNotification::ThreadStatusChanged(
            notification,
        )) = message
        else {
            panic!("expected thread/status/changed notification");
        };
        notification
    }

    fn test_thread(thread_id: &str, source: codex_app_server_protocol::SessionSource) -> Thread {
        Thread {
            id: thread_id.to_string(),
            preview: String::new(),
            model_provider: "mock-provider".to_string(),
            created_at: 0,
            updated_at: 0,
            path: None,
            cwd: PathBuf::from("/tmp"),
            cli_version: "test".to_string(),
            source,
            git_info: None,
            turns: Vec::new(),
        }
    }
}
