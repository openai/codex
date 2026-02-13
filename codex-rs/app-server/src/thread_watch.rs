use codex_app_server_protocol::LoadedThreadEntry;
use codex_app_server_protocol::LoadedThreadStatus;
use codex_app_server_protocol::SessionSource;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadActiveFlag;
use codex_app_server_protocol::ThreadTerminalOutcome;
use codex_app_server_protocol::ThreadWatchResponse;
use codex_app_server_protocol::ThreadWatchUpdate;
use codex_app_server_protocol::ThreadWatchUpdatedNotification;
use codex_protocol::protocol::SubAgentSource as CoreSubAgentSource;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::broadcast;

const THREAD_WATCH_CHANNEL_CAPACITY: usize = 256;

#[derive(Clone)]
pub(crate) struct ThreadWatchManager {
    state: Arc<Mutex<ThreadWatchState>>,
    update_tx: broadcast::Sender<ThreadWatchUpdatedNotification>,
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
        let (update_tx, _rx) = broadcast::channel(THREAD_WATCH_CHANNEL_CAPACITY);
        Self {
            state: Arc::new(Mutex::new(ThreadWatchState::default())),
            update_tx,
        }
    }

    pub(crate) async fn subscribe_with_snapshot(
        &self,
    ) -> (
        ThreadWatchResponse,
        broadcast::Receiver<ThreadWatchUpdatedNotification>,
    ) {
        let state = self.state.lock().await;
        let rx = self.update_tx.subscribe();
        (state.snapshot_response(), rx)
    }

    pub(crate) async fn current_version(&self) -> i64 {
        self.state.lock().await.version
    }

    pub(crate) async fn upsert_thread(&self, mut thread: Thread) {
        thread.turns.clear();
        self.mutate_and_publish(move |state| state.upsert_thread(thread))
            .await;
    }

    pub(crate) async fn remove_thread(&self, thread_id: &str) {
        let thread_id = thread_id.to_string();
        self.mutate_and_publish(move |state| state.remove_thread(&thread_id))
            .await;
    }

    pub(crate) async fn note_turn_started(&self, thread_id: &str) {
        let thread_id = thread_id.to_string();
        self.mutate_and_publish(move |state| {
            state.update_runtime(&thread_id, |runtime| {
                runtime.running = true;
                runtime.last_terminal_outcome = None;
            })
        })
        .await;
    }

    pub(crate) async fn note_turn_completed(&self, thread_id: &str, failed: bool) {
        let thread_id = thread_id.to_string();
        self.mutate_and_publish(move |state| {
            state.update_runtime(&thread_id, |runtime| {
                runtime.running = false;
                runtime.pending_permission_requests = 0;
                runtime.pending_user_input_requests = 0;
                runtime.last_terminal_outcome = Some(if failed {
                    ThreadTerminalOutcome::Failed
                } else {
                    ThreadTerminalOutcome::Completed
                });
            })
        })
        .await;
    }

    pub(crate) async fn note_turn_interrupted(&self, thread_id: &str) {
        let thread_id = thread_id.to_string();
        self.mutate_and_publish(move |state| {
            state.update_runtime(&thread_id, |runtime| {
                runtime.running = false;
                runtime.pending_permission_requests = 0;
                runtime.pending_user_input_requests = 0;
                runtime.last_terminal_outcome = Some(ThreadTerminalOutcome::Interrupted);
            })
        })
        .await;
    }

    pub(crate) async fn note_thread_shutdown(&self, thread_id: &str) {
        let thread_id = thread_id.to_string();
        self.mutate_and_publish(move |state| {
            state.update_runtime(&thread_id, |runtime| {
                runtime.running = false;
                runtime.pending_permission_requests = 0;
                runtime.pending_user_input_requests = 0;
                runtime.last_terminal_outcome = Some(ThreadTerminalOutcome::Shutdown);
            })
        })
        .await;
    }

    pub(crate) async fn note_permission_requested(
        &self,
        thread_id: &str,
    ) -> ThreadWatchActiveGuard {
        let thread_id = thread_id.to_string();
        let guard_thread_id = thread_id.clone();
        self.mutate_and_publish(move |state| {
            state.update_runtime(&thread_id, |runtime| {
                runtime.pending_permission_requests =
                    runtime.pending_permission_requests.saturating_add(1);
                runtime.last_terminal_outcome = None;
            })
        })
        .await;
        ThreadWatchActiveGuard::new(
            self.clone(),
            guard_thread_id,
            ThreadWatchActiveGuardType::Permission,
        )
    }

    pub(crate) async fn note_user_input_requested(
        &self,
        thread_id: &str,
    ) -> ThreadWatchActiveGuard {
        let thread_id = thread_id.to_string();
        let guard_thread_id = thread_id.clone();
        self.mutate_and_publish(move |state| {
            state.update_runtime(&thread_id, |runtime| {
                runtime.pending_user_input_requests =
                    runtime.pending_user_input_requests.saturating_add(1);
                runtime.last_terminal_outcome = None;
            })
        })
        .await;
        ThreadWatchActiveGuard::new(
            self.clone(),
            guard_thread_id,
            ThreadWatchActiveGuardType::UserInput,
        )
    }

    async fn mutate_and_publish<F>(&self, mutate: F)
    where
        F: FnOnce(&mut ThreadWatchState) -> Option<ThreadWatchUpdatedNotification>,
    {
        self.mutate_and_publish_with_pre_publish_hook(mutate, || {})
            .await;
    }

    async fn mutate_and_publish_with_pre_publish_hook<F, H>(&self, mutate: F, pre_publish_hook: H)
    where
        F: FnOnce(&mut ThreadWatchState) -> Option<ThreadWatchUpdatedNotification>,
        H: FnOnce(),
    {
        let mut state = self.state.lock().await;
        let notification = mutate(&mut state);
        pre_publish_hook();
        if let Some(notification) = notification {
            let _ = self.update_tx.send(notification);
        }
    }

    async fn note_active_guard_released(
        &self,
        thread_id: String,
        guard_type: ThreadWatchActiveGuardType,
    ) {
        self.mutate_and_publish(move |state| {
            state.update_runtime(&thread_id, |runtime| match guard_type {
                ThreadWatchActiveGuardType::Permission => {
                    runtime.pending_permission_requests =
                        runtime.pending_permission_requests.saturating_sub(1);
                }
                ThreadWatchActiveGuardType::UserInput => {
                    runtime.pending_user_input_requests =
                        runtime.pending_user_input_requests.saturating_sub(1);
                }
            })
        })
        .await;
    }
}

#[derive(Default)]
struct ThreadWatchState {
    threads: HashMap<String, TrackedThread>,
    root_entries: HashMap<String, LoadedThreadEntry>,
    root_active_counts: HashMap<String, ActiveCounts>,
    version: i64,
}

impl ThreadWatchState {
    fn snapshot_response(&self) -> ThreadWatchResponse {
        let data: Vec<LoadedThreadEntry> = self.root_entries.values().cloned().collect();
        ThreadWatchResponse {
            snapshot_version: self.version,
            data,
        }
    }

    fn upsert_thread(&mut self, thread: Thread) -> Option<ThreadWatchUpdatedNotification> {
        let thread_id = thread.id.clone();
        if let Some(root_thread_id) = self
            .threads
            .get(&thread_id)
            .map(|tracked| tracked.root_thread_id.clone())
        {
            // Root lineage is immutable for a tracked thread; refresh only the thread payload.
            let previous_entry = self.root_entries.get(&root_thread_id).cloned();
            if let Some(tracked) = self.threads.get_mut(&thread_id) {
                tracked.thread = thread;
            }
            return self.refresh_root_entry(&root_thread_id, previous_entry);
        }

        let root_thread_id = if let Some(parent_thread_id) = infer_parent_thread_id(&thread) {
            let Some(parent_root_thread_id) = self
                .threads
                .get(parent_thread_id.as_str())
                .map(|tracked| tracked.root_thread_id.clone())
            else {
                // We rely on two invariants:
                // 1) a thread's root id is immutable once created;
                // 2) parents are discovered before children.
                // If (2) is violated, we intentionally drop this child from thread/watch and only
                // recover if a later upsert for the child is observed after the parent is present.
                return None;
            };
            parent_root_thread_id
        } else {
            thread_id.clone()
        };

        let previous_entry = self.root_entries.get(&root_thread_id).cloned();
        self.threads.insert(
            thread_id,
            TrackedThread {
                thread,
                root_thread_id: root_thread_id.clone(),
                runtime: RuntimeFacts::default(),
            },
        );

        self.refresh_root_entry(&root_thread_id, previous_entry)
    }

    fn remove_thread(&mut self, thread_id: &str) -> Option<ThreadWatchUpdatedNotification> {
        let tracked = self.threads.remove(thread_id)?;
        let root_thread_id = tracked.root_thread_id;
        let previous_entry = self.root_entries.get(&root_thread_id).cloned();
        let previous_active_flags = ActiveFlags::from_runtime(&tracked.runtime);
        self.apply_active_flags_change(
            &root_thread_id,
            previous_active_flags,
            ActiveFlags::default(),
        );
        self.refresh_root_entry(&root_thread_id, previous_entry)
    }

    fn update_runtime<F>(
        &mut self,
        thread_id: &str,
        mutate: F,
    ) -> Option<ThreadWatchUpdatedNotification>
    where
        F: FnOnce(&mut RuntimeFacts),
    {
        let root_thread_id = self.threads.get(thread_id)?.root_thread_id.clone();
        let previous_entry = self.root_entries.get(&root_thread_id).cloned();
        let (previous_active_flags, current_active_flags) = {
            let tracked = self.threads.get_mut(thread_id)?;
            let previous_active_flags = ActiveFlags::from_runtime(&tracked.runtime);
            mutate(&mut tracked.runtime);
            let current_active_flags = ActiveFlags::from_runtime(&tracked.runtime);
            (previous_active_flags, current_active_flags)
        };
        self.apply_active_flags_change(
            &root_thread_id,
            previous_active_flags,
            current_active_flags,
        );
        self.refresh_root_entry(&root_thread_id, previous_entry)
    }

    fn apply_active_flags_change(
        &mut self,
        root_thread_id: &str,
        previous_active_flags: ActiveFlags,
        current_active_flags: ActiveFlags,
    ) {
        if previous_active_flags == current_active_flags {
            return;
        }
        let should_remove = {
            let active_counts = self
                .root_active_counts
                .entry(root_thread_id.to_string())
                .or_default();
            active_counts.apply_flags_change(previous_active_flags, current_active_flags);
            active_counts.is_empty()
        };
        if should_remove {
            self.root_active_counts.remove(root_thread_id);
        }
    }

    fn refresh_root_entry(
        &mut self,
        root_thread_id: &str,
        previous_entry: Option<LoadedThreadEntry>,
    ) -> Option<ThreadWatchUpdatedNotification> {
        let current_entry = self.compute_root_entry(root_thread_id);
        if current_entry.as_ref() == previous_entry.as_ref() {
            return None;
        }

        match current_entry {
            Some(entry) => {
                self.root_entries
                    .insert(root_thread_id.to_string(), entry.clone());
                Some(self.next_upsert_notification(entry))
            }
            None => {
                self.root_entries.remove(root_thread_id);
                if previous_entry.is_some() {
                    Some(self.next_remove_notification(root_thread_id.to_string()))
                } else {
                    None
                }
            }
        }
    }

    fn compute_root_entry(&self, root_thread_id: &str) -> Option<LoadedThreadEntry> {
        let tracked_root = self.threads.get(root_thread_id)?;
        if tracked_root.root_thread_id != root_thread_id {
            return None;
        }

        let active_flags = self
            .root_active_counts
            .get(root_thread_id)
            .copied()
            .unwrap_or_default()
            .as_active_flags();
        let status = if active_flags.is_empty() {
            match tracked_root.runtime.last_terminal_outcome {
                Some(outcome) => LoadedThreadStatus::Terminal { outcome },
                None => LoadedThreadStatus::Idle,
            }
        } else {
            LoadedThreadStatus::Active {
                active_flags: active_flags.as_vec(),
            }
        };

        Some(LoadedThreadEntry {
            thread: tracked_root.thread.clone(),
            status,
        })
    }

    fn next_upsert_notification(
        &mut self,
        entry: LoadedThreadEntry,
    ) -> ThreadWatchUpdatedNotification {
        self.version += 1;
        ThreadWatchUpdatedNotification {
            version: self.version,
            update: ThreadWatchUpdate::Upsert { entry },
        }
    }

    fn next_remove_notification(&mut self, thread_id: String) -> ThreadWatchUpdatedNotification {
        self.version += 1;
        ThreadWatchUpdatedNotification {
            version: self.version,
            update: ThreadWatchUpdate::Remove { thread_id },
        }
    }
}

fn infer_parent_thread_id(thread: &Thread) -> Option<String> {
    match &thread.source {
        SessionSource::SubAgent(CoreSubAgentSource::ThreadSpawn {
            parent_thread_id, ..
        }) => Some(parent_thread_id.to_string()),
        _ => None,
    }
}

#[derive(Clone)]
struct TrackedThread {
    thread: Thread,
    root_thread_id: String,
    runtime: RuntimeFacts,
}

#[derive(Clone, Default)]
struct RuntimeFacts {
    running: bool,
    pending_permission_requests: u32,
    pending_user_input_requests: u32,
    last_terminal_outcome: Option<ThreadTerminalOutcome>,
}

#[derive(Clone, Copy, Default)]
struct ActiveCounts {
    running: u32,
    waiting_permission: u32,
    waiting_user_input: u32,
}

impl ActiveCounts {
    fn apply_flags_change(&mut self, previous: ActiveFlags, current: ActiveFlags) {
        self.running = apply_flag_count_change(self.running, previous.running, current.running);
        self.waiting_permission = apply_flag_count_change(
            self.waiting_permission,
            previous.waiting_permission,
            current.waiting_permission,
        );
        self.waiting_user_input = apply_flag_count_change(
            self.waiting_user_input,
            previous.waiting_user_input,
            current.waiting_user_input,
        );
    }

    fn is_empty(self) -> bool {
        self.running == 0 && self.waiting_permission == 0 && self.waiting_user_input == 0
    }

    fn as_active_flags(self) -> ActiveFlags {
        ActiveFlags {
            running: self.running > 0,
            waiting_permission: self.waiting_permission > 0,
            waiting_user_input: self.waiting_user_input > 0,
        }
    }
}

fn apply_flag_count_change(count: u32, previous: bool, current: bool) -> u32 {
    match (previous, current) {
        (false, true) => count.saturating_add(1),
        (true, false) => count.saturating_sub(1),
        _ => count,
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct ActiveFlags {
    running: bool,
    waiting_permission: bool,
    waiting_user_input: bool,
}

impl ActiveFlags {
    fn from_runtime(runtime: &RuntimeFacts) -> Self {
        Self {
            running: runtime.running,
            waiting_permission: runtime.pending_permission_requests > 0,
            waiting_user_input: runtime.pending_user_input_requests > 0,
        }
    }

    fn is_empty(self) -> bool {
        !self.running && !self.waiting_permission && !self.waiting_user_input
    }

    fn as_vec(self) -> Vec<ThreadActiveFlag> {
        let mut flags = Vec::new();
        if self.running {
            flags.push(ThreadActiveFlag::Running);
        }
        if self.waiting_permission {
            flags.push(ThreadActiveFlag::WaitingPermission);
        }
        if self.waiting_user_input {
            flags.push(ThreadActiveFlag::WaitingUserInput);
        }
        flags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::ThreadId;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use tokio::time::Duration;
    use tokio::time::timeout;

    const ROOT_THREAD_ID: &str = "00000000-0000-0000-0000-000000000001";
    const CHILD_THREAD_ID: &str = "00000000-0000-0000-0000-000000000002";
    const GRANDCHILD_THREAD_ID: &str = "00000000-0000-0000-0000-000000000003";

    #[tokio::test]
    async fn snapshot_and_subscription_track_nested_subagent_flags() {
        let manager = ThreadWatchManager::new();
        manager
            .upsert_thread(test_thread(ROOT_THREAD_ID, None))
            .await;
        manager
            .upsert_thread(test_thread(CHILD_THREAD_ID, Some(ROOT_THREAD_ID)))
            .await;
        manager
            .upsert_thread(test_thread(GRANDCHILD_THREAD_ID, Some(CHILD_THREAD_ID)))
            .await;

        let (snapshot, mut updates_rx) = manager.subscribe_with_snapshot().await;
        assert_eq!(snapshot.data.len(), 1);
        assert_eq!(snapshot.data[0].thread.id, ROOT_THREAD_ID);
        assert_eq!(snapshot.data[0].status, LoadedThreadStatus::Idle);

        manager.note_turn_started(CHILD_THREAD_ID).await;
        assert_root_status(
            recv_update(&mut updates_rx).await,
            ROOT_THREAD_ID,
            LoadedThreadStatus::Active {
                active_flags: vec![ThreadActiveFlag::Running],
            },
        );

        let permission_guard = manager
            .note_permission_requested(GRANDCHILD_THREAD_ID)
            .await;
        assert_root_status(
            recv_update(&mut updates_rx).await,
            ROOT_THREAD_ID,
            LoadedThreadStatus::Active {
                active_flags: vec![
                    ThreadActiveFlag::Running,
                    ThreadActiveFlag::WaitingPermission,
                ],
            },
        );

        let user_input_guard = manager
            .note_user_input_requested(GRANDCHILD_THREAD_ID)
            .await;
        assert_root_status(
            recv_update(&mut updates_rx).await,
            ROOT_THREAD_ID,
            LoadedThreadStatus::Active {
                active_flags: vec![
                    ThreadActiveFlag::Running,
                    ThreadActiveFlag::WaitingPermission,
                    ThreadActiveFlag::WaitingUserInput,
                ],
            },
        );

        drop(permission_guard);
        assert_root_status(
            recv_update(&mut updates_rx).await,
            ROOT_THREAD_ID,
            LoadedThreadStatus::Active {
                active_flags: vec![
                    ThreadActiveFlag::Running,
                    ThreadActiveFlag::WaitingUserInput,
                ],
            },
        );

        drop(user_input_guard);
        assert_root_status(
            recv_update(&mut updates_rx).await,
            ROOT_THREAD_ID,
            LoadedThreadStatus::Active {
                active_flags: vec![ThreadActiveFlag::Running],
            },
        );

        manager.note_turn_interrupted(CHILD_THREAD_ID).await;
        assert_root_status(
            recv_update(&mut updates_rx).await,
            ROOT_THREAD_ID,
            LoadedThreadStatus::Idle,
        );
    }

    #[tokio::test]
    async fn child_terminal_failure_does_not_change_root_terminal_outcome() {
        let manager = ThreadWatchManager::new();
        manager
            .upsert_thread(test_thread(ROOT_THREAD_ID, None))
            .await;
        manager
            .upsert_thread(test_thread(CHILD_THREAD_ID, Some(ROOT_THREAD_ID)))
            .await;
        let (_snapshot, mut updates_rx) = manager.subscribe_with_snapshot().await;

        manager.note_turn_completed(ROOT_THREAD_ID, false).await;
        assert_root_status(
            recv_update(&mut updates_rx).await,
            ROOT_THREAD_ID,
            LoadedThreadStatus::Terminal {
                outcome: ThreadTerminalOutcome::Completed,
            },
        );

        manager.note_turn_completed(CHILD_THREAD_ID, true).await;
        assert!(
            timeout(Duration::from_millis(100), updates_rx.recv())
                .await
                .is_err()
        );

        let (snapshot, _updates_rx) = manager.subscribe_with_snapshot().await;
        assert_eq!(snapshot.data.len(), 1);
        assert_eq!(snapshot.data[0].thread.id, ROOT_THREAD_ID);
        assert_eq!(
            snapshot.data[0].status,
            LoadedThreadStatus::Terminal {
                outcome: ThreadTerminalOutcome::Completed,
            }
        );
    }

    #[tokio::test]
    async fn removing_root_does_not_promote_child_to_root_entry() {
        let manager = ThreadWatchManager::new();
        manager
            .upsert_thread(test_thread(ROOT_THREAD_ID, None))
            .await;
        manager
            .upsert_thread(test_thread(CHILD_THREAD_ID, Some(ROOT_THREAD_ID)))
            .await;

        let (_snapshot, mut updates_rx) = manager.subscribe_with_snapshot().await;
        manager.remove_thread(ROOT_THREAD_ID).await;

        let removed = recv_update(&mut updates_rx).await;

        let ThreadWatchUpdate::Remove { thread_id } = removed.update else {
            panic!("expected remove update");
        };
        assert_eq!(thread_id, ROOT_THREAD_ID);
        assert!(
            timeout(Duration::from_millis(100), updates_rx.recv())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn removing_intermediate_parent_preserves_original_root_for_grandchild() {
        let manager = ThreadWatchManager::new();
        manager
            .upsert_thread(test_thread(ROOT_THREAD_ID, None))
            .await;
        manager
            .upsert_thread(test_thread(CHILD_THREAD_ID, Some(ROOT_THREAD_ID)))
            .await;
        manager
            .upsert_thread(test_thread(GRANDCHILD_THREAD_ID, Some(CHILD_THREAD_ID)))
            .await;

        let (_snapshot, mut updates_rx) = manager.subscribe_with_snapshot().await;
        manager.remove_thread(CHILD_THREAD_ID).await;
        assert!(
            timeout(Duration::from_millis(100), updates_rx.recv())
                .await
                .is_err()
        );

        manager.note_turn_started(GRANDCHILD_THREAD_ID).await;
        assert_root_status(
            recv_update(&mut updates_rx).await,
            ROOT_THREAD_ID,
            LoadedThreadStatus::Active {
                active_flags: vec![ThreadActiveFlag::Running],
            },
        );
    }

    #[tokio::test]
    async fn thread_with_missing_parent_requires_replay_after_parent_appears() {
        let manager = ThreadWatchManager::new();
        manager
            .upsert_thread(test_thread(CHILD_THREAD_ID, Some(ROOT_THREAD_ID)))
            .await;
        let (snapshot, mut updates_rx) = manager.subscribe_with_snapshot().await;
        assert_eq!(snapshot.data, Vec::<LoadedThreadEntry>::new());

        manager.note_turn_started(CHILD_THREAD_ID).await;
        assert!(
            timeout(Duration::from_millis(100), updates_rx.recv())
                .await
                .is_err()
        );

        manager
            .upsert_thread(test_thread(ROOT_THREAD_ID, None))
            .await;
        assert_root_status(
            recv_update(&mut updates_rx).await,
            ROOT_THREAD_ID,
            LoadedThreadStatus::Idle,
        );

        manager
            .upsert_thread(test_thread(CHILD_THREAD_ID, Some(ROOT_THREAD_ID)))
            .await;
        manager.note_turn_started(CHILD_THREAD_ID).await;
        assert_root_status(
            recv_update(&mut updates_rx).await,
            ROOT_THREAD_ID,
            LoadedThreadStatus::Active {
                active_flags: vec![ThreadActiveFlag::Running],
            },
        );
    }

    #[tokio::test]
    async fn snapshot_version_precedes_subscription_updates() {
        let manager = ThreadWatchManager::new();
        manager
            .upsert_thread(test_thread(ROOT_THREAD_ID, None))
            .await;

        let (snapshot, mut updates_rx) = manager.subscribe_with_snapshot().await;
        manager.note_turn_started(ROOT_THREAD_ID).await;
        let running_update = recv_update(&mut updates_rx).await;
        assert_eq!(running_update.version, snapshot.snapshot_version + 1);
        assert_root_status(
            running_update,
            ROOT_THREAD_ID,
            LoadedThreadStatus::Active {
                active_flags: vec![ThreadActiveFlag::Running],
            },
        );

        manager.note_turn_interrupted(ROOT_THREAD_ID).await;
        let interrupted_update = recv_update(&mut updates_rx).await;
        assert_eq!(interrupted_update.version, snapshot.snapshot_version + 2);
        assert_root_status(
            interrupted_update,
            ROOT_THREAD_ID,
            LoadedThreadStatus::Terminal {
                outcome: ThreadTerminalOutcome::Interrupted,
            },
        );
    }

    #[tokio::test]
    async fn upsert_discards_turn_history_from_snapshot_entries() {
        let manager = ThreadWatchManager::new();
        let mut thread = test_thread(ROOT_THREAD_ID, None);
        thread.turns.push(codex_app_server_protocol::Turn {
            id: "turn-1".to_string(),
            items: Vec::new(),
            status: codex_app_server_protocol::TurnStatus::Completed,
            error: None,
        });
        manager.upsert_thread(thread).await;

        let (snapshot, _updates_rx) = manager.subscribe_with_snapshot().await;
        assert_eq!(snapshot.data.len(), 1);
        assert!(snapshot.data[0].thread.turns.is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn subscribe_during_publish_receives_snapshot_without_duplicate_update() {
        let manager = ThreadWatchManager::new();
        manager
            .upsert_thread(test_thread(ROOT_THREAD_ID, None))
            .await;

        let manager_for_subscribe = manager.clone();
        let (snapshot_tx, snapshot_rx) = tokio::sync::oneshot::channel::<(
            ThreadWatchResponse,
            broadcast::Receiver<ThreadWatchUpdatedNotification>,
        )>();
        manager
            .mutate_and_publish_with_pre_publish_hook(
                |state| {
                    state.update_runtime(ROOT_THREAD_ID, |runtime| {
                        runtime.running = true;
                        runtime.last_terminal_outcome = None;
                    })
                },
                move || {
                    tokio::spawn(async move {
                        let snapshot_and_rx = manager_for_subscribe.subscribe_with_snapshot().await;
                        let _ = snapshot_tx.send(snapshot_and_rx);
                    });
                    std::thread::sleep(Duration::from_millis(20));
                },
            )
            .await;

        let (snapshot, mut updates_rx) = timeout(Duration::from_secs(1), snapshot_rx)
            .await
            .expect("timed out waiting for snapshot subscriber")
            .expect("snapshot subscriber dropped without returning snapshot");

        assert_eq!(snapshot.snapshot_version, manager.current_version().await);
        assert_eq!(snapshot.data.len(), 1);
        assert_eq!(snapshot.data[0].thread.id, ROOT_THREAD_ID);
        assert_eq!(
            snapshot.data[0].status,
            LoadedThreadStatus::Active {
                active_flags: vec![ThreadActiveFlag::Running],
            }
        );
        assert!(
            timeout(Duration::from_millis(100), updates_rx.recv())
                .await
                .is_err()
        );
    }

    async fn recv_update(
        updates_rx: &mut broadcast::Receiver<ThreadWatchUpdatedNotification>,
    ) -> ThreadWatchUpdatedNotification {
        timeout(Duration::from_secs(1), updates_rx.recv())
            .await
            .expect("timed out waiting for thread/watch update")
            .expect("thread/watch channel unexpectedly closed")
    }

    fn assert_root_status(
        notification: ThreadWatchUpdatedNotification,
        expected_thread_id: &str,
        expected_status: LoadedThreadStatus,
    ) {
        let ThreadWatchUpdate::Upsert { entry } = notification.update else {
            panic!("expected upsert update");
        };
        assert_eq!(entry.thread.id, expected_thread_id);
        assert_eq!(entry.status, expected_status);
    }

    fn test_thread(thread_id: &str, parent_thread_id: Option<&str>) -> Thread {
        let source = parent_thread_id.map_or(SessionSource::AppServer, |parent_thread_id| {
            SessionSource::SubAgent(CoreSubAgentSource::ThreadSpawn {
                parent_thread_id: ThreadId::from_string(parent_thread_id)
                    .expect("test thread ids should be valid UUIDs"),
                depth: 1,
            })
        });

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
