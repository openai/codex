use async_trait::async_trait;
use codex_protocol::ThreadId;
use codex_state::StateRuntime;
use std::sync::Arc;

use crate::AgentGraphStore;
use crate::AgentGraphStoreError;
use crate::AgentGraphStoreResult;
use crate::ThreadSpawnEdgeStatus;

/// SQLite-backed implementation of [`AgentGraphStore`] using an existing state runtime.
#[derive(Clone)]
pub struct LocalAgentGraphStore {
    state_db: Arc<StateRuntime>,
}

impl std::fmt::Debug for LocalAgentGraphStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalAgentGraphStore")
            .field("codex_home", &self.state_db.codex_home())
            .finish_non_exhaustive()
    }
}

impl LocalAgentGraphStore {
    /// Create a local graph store from an already-initialized state runtime.
    pub fn new(state_db: Arc<StateRuntime>) -> Self {
        Self { state_db }
    }
}

#[async_trait]
impl AgentGraphStore for LocalAgentGraphStore {
    async fn upsert_thread_spawn_edge(
        &self,
        parent_thread_id: ThreadId,
        child_thread_id: ThreadId,
        status: ThreadSpawnEdgeStatus,
    ) -> AgentGraphStoreResult<()> {
        self.state_db
            .upsert_thread_spawn_edge(parent_thread_id, child_thread_id, to_state_status(status))
            .await
            .map_err(internal_error)
    }

    async fn set_thread_spawn_edge_status(
        &self,
        child_thread_id: ThreadId,
        status: ThreadSpawnEdgeStatus,
    ) -> AgentGraphStoreResult<()> {
        self.state_db
            .set_thread_spawn_edge_status(child_thread_id, to_state_status(status))
            .await
            .map_err(internal_error)
    }

    async fn list_thread_spawn_children(
        &self,
        parent_thread_id: ThreadId,
        status_filter: Option<ThreadSpawnEdgeStatus>,
    ) -> AgentGraphStoreResult<Vec<ThreadId>> {
        if let Some(status) = status_filter {
            return self
                .state_db
                .list_thread_spawn_children_with_status(parent_thread_id, to_state_status(status))
                .await
                .map_err(internal_error);
        }

        let mut open_children = self
            .state_db
            .list_thread_spawn_children_with_status(
                parent_thread_id,
                codex_state::DirectionalThreadSpawnEdgeStatus::Open,
            )
            .await
            .map_err(internal_error)?;
        let closed_children = self
            .state_db
            .list_thread_spawn_children_with_status(
                parent_thread_id,
                codex_state::DirectionalThreadSpawnEdgeStatus::Closed,
            )
            .await
            .map_err(internal_error)?;
        open_children.extend(closed_children);
        open_children.sort_by_key(std::string::ToString::to_string);
        Ok(open_children)
    }

    async fn list_thread_spawn_descendants(
        &self,
        root_thread_id: ThreadId,
        status_filter: Option<ThreadSpawnEdgeStatus>,
    ) -> AgentGraphStoreResult<Vec<ThreadId>> {
        match status_filter {
            Some(status) => self
                .state_db
                .list_thread_spawn_descendants_with_status(root_thread_id, to_state_status(status))
                .await
                .map_err(internal_error),
            None => self
                .state_db
                .list_thread_spawn_descendants(root_thread_id)
                .await
                .map_err(internal_error),
        }
    }
}

fn to_state_status(status: ThreadSpawnEdgeStatus) -> codex_state::DirectionalThreadSpawnEdgeStatus {
    match status {
        ThreadSpawnEdgeStatus::Open => codex_state::DirectionalThreadSpawnEdgeStatus::Open,
        ThreadSpawnEdgeStatus::Closed => codex_state::DirectionalThreadSpawnEdgeStatus::Closed,
    }
}

fn internal_error(err: impl std::fmt::Display) -> AgentGraphStoreError {
    AgentGraphStoreError::Internal {
        message: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_state::DirectionalThreadSpawnEdgeStatus;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    struct TestRuntime {
        state_db: Arc<StateRuntime>,
        _codex_home: TempDir,
    }

    fn thread_id(suffix: u128) -> ThreadId {
        ThreadId::from_string(&format!("00000000-0000-0000-0000-{suffix:012}"))
            .expect("valid thread id")
    }

    async fn state_runtime() -> TestRuntime {
        let codex_home = TempDir::new().expect("tempdir should be created");
        let state_db =
            StateRuntime::init(codex_home.path().to_path_buf(), "test-provider".to_string())
                .await
                .expect("state db should initialize");
        TestRuntime {
            state_db,
            _codex_home: codex_home,
        }
    }

    #[tokio::test]
    async fn local_store_upserts_and_lists_direct_children_with_status_filters() {
        let fixture = state_runtime().await;
        let state_db = fixture.state_db;
        let store = LocalAgentGraphStore::new(state_db.clone());
        let parent_thread_id = thread_id(/*suffix*/ 1);
        let first_child_thread_id = thread_id(/*suffix*/ 2);
        let second_child_thread_id = thread_id(/*suffix*/ 3);

        store
            .upsert_thread_spawn_edge(
                parent_thread_id,
                second_child_thread_id,
                ThreadSpawnEdgeStatus::Closed,
            )
            .await
            .expect("closed child edge should insert");
        store
            .upsert_thread_spawn_edge(
                parent_thread_id,
                first_child_thread_id,
                ThreadSpawnEdgeStatus::Open,
            )
            .await
            .expect("open child edge should insert");

        let all_children = store
            .list_thread_spawn_children(parent_thread_id, /*status_filter*/ None)
            .await
            .expect("all children should load");
        assert_eq!(
            all_children,
            vec![first_child_thread_id, second_child_thread_id]
        );

        let open_children = store
            .list_thread_spawn_children(parent_thread_id, Some(ThreadSpawnEdgeStatus::Open))
            .await
            .expect("open children should load");
        let state_open_children = state_db
            .list_thread_spawn_children_with_status(
                parent_thread_id,
                DirectionalThreadSpawnEdgeStatus::Open,
            )
            .await
            .expect("state open children should load");
        assert_eq!(open_children, state_open_children);
        assert_eq!(open_children, vec![first_child_thread_id]);

        let closed_children = store
            .list_thread_spawn_children(parent_thread_id, Some(ThreadSpawnEdgeStatus::Closed))
            .await
            .expect("closed children should load");
        assert_eq!(closed_children, vec![second_child_thread_id]);
    }

    #[tokio::test]
    async fn local_store_updates_edge_status() {
        let fixture = state_runtime().await;
        let state_db = fixture.state_db;
        let store = LocalAgentGraphStore::new(state_db);
        let parent_thread_id = thread_id(/*suffix*/ 10);
        let child_thread_id = thread_id(/*suffix*/ 11);

        store
            .upsert_thread_spawn_edge(
                parent_thread_id,
                child_thread_id,
                ThreadSpawnEdgeStatus::Open,
            )
            .await
            .expect("child edge should insert");
        store
            .set_thread_spawn_edge_status(child_thread_id, ThreadSpawnEdgeStatus::Closed)
            .await
            .expect("child edge should close");

        let open_children = store
            .list_thread_spawn_children(parent_thread_id, Some(ThreadSpawnEdgeStatus::Open))
            .await
            .expect("open children should load");
        assert_eq!(open_children, Vec::<ThreadId>::new());

        let closed_children = store
            .list_thread_spawn_children(parent_thread_id, Some(ThreadSpawnEdgeStatus::Closed))
            .await
            .expect("closed children should load");
        assert_eq!(closed_children, vec![child_thread_id]);
    }

    #[tokio::test]
    async fn local_store_lists_descendants_breadth_first_with_status_filters() {
        let fixture = state_runtime().await;
        let state_db = fixture.state_db;
        let store = LocalAgentGraphStore::new(state_db.clone());
        let root_thread_id = thread_id(/*suffix*/ 20);
        let later_child_thread_id = thread_id(/*suffix*/ 22);
        let earlier_child_thread_id = thread_id(/*suffix*/ 21);
        let closed_grandchild_thread_id = thread_id(/*suffix*/ 23);
        let open_grandchild_thread_id = thread_id(/*suffix*/ 24);
        let closed_child_thread_id = thread_id(/*suffix*/ 25);
        let closed_great_grandchild_thread_id = thread_id(/*suffix*/ 26);

        for (parent_thread_id, child_thread_id, status) in [
            (
                root_thread_id,
                later_child_thread_id,
                ThreadSpawnEdgeStatus::Open,
            ),
            (
                root_thread_id,
                earlier_child_thread_id,
                ThreadSpawnEdgeStatus::Open,
            ),
            (
                earlier_child_thread_id,
                open_grandchild_thread_id,
                ThreadSpawnEdgeStatus::Open,
            ),
            (
                later_child_thread_id,
                closed_grandchild_thread_id,
                ThreadSpawnEdgeStatus::Closed,
            ),
            (
                root_thread_id,
                closed_child_thread_id,
                ThreadSpawnEdgeStatus::Closed,
            ),
            (
                closed_child_thread_id,
                closed_great_grandchild_thread_id,
                ThreadSpawnEdgeStatus::Closed,
            ),
        ] {
            store
                .upsert_thread_spawn_edge(parent_thread_id, child_thread_id, status)
                .await
                .expect("edge should insert");
        }

        let all_descendants = store
            .list_thread_spawn_descendants(root_thread_id, /*status_filter*/ None)
            .await
            .expect("all descendants should load");
        assert_eq!(
            all_descendants,
            vec![
                earlier_child_thread_id,
                later_child_thread_id,
                closed_child_thread_id,
                closed_grandchild_thread_id,
                open_grandchild_thread_id,
                closed_great_grandchild_thread_id,
            ]
        );

        let open_descendants = store
            .list_thread_spawn_descendants(root_thread_id, Some(ThreadSpawnEdgeStatus::Open))
            .await
            .expect("open descendants should load");
        let state_open_descendants = state_db
            .list_thread_spawn_descendants_with_status(
                root_thread_id,
                DirectionalThreadSpawnEdgeStatus::Open,
            )
            .await
            .expect("state open descendants should load");
        assert_eq!(open_descendants, state_open_descendants);
        assert_eq!(
            open_descendants,
            vec![
                earlier_child_thread_id,
                later_child_thread_id,
                open_grandchild_thread_id,
            ]
        );

        let closed_descendants = store
            .list_thread_spawn_descendants(root_thread_id, Some(ThreadSpawnEdgeStatus::Closed))
            .await
            .expect("closed descendants should load");
        assert_eq!(
            closed_descendants,
            vec![closed_child_thread_id, closed_great_grandchild_thread_id]
        );
    }
}
