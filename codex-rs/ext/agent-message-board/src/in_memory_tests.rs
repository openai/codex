//! Verifies that registry cleanup releases expired entries and preserves live boards.

use super::*;
use crate::NotificationDelivery;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;

struct UnusedHost;

impl MessageBoardHost for UnusedHost {
    fn agent_path(&self, _caller: ThreadId) -> BoxFuture<'_, Result<AgentPath>> {
        unreachable!("registry tests do not invoke host capabilities")
    }

    fn resolve_agent(&self, _path: AgentPath) -> BoxFuture<'_, Result<ThreadId>> {
        unreachable!("registry tests do not invoke host capabilities")
    }

    fn current_time(&self, _caller: ThreadId) -> BoxFuture<'_, Result<DateTime<Utc>>> {
        unreachable!("registry tests do not invoke host capabilities")
    }

    fn notify(
        &self,
        _recipient: ThreadId,
        _post: PostPreview,
    ) -> BoxFuture<'_, Result<NotificationDelivery>> {
        unreachable!("registry tests do not invoke host capabilities")
    }
}

#[tokio::test]
async fn opening_boards_prunes_expired_entries_and_preserves_live_state() {
    let boards = InMemoryMessageBoards::default();
    let identity = SessionId::new();
    let live = boards.open(identity, Arc::new(UnusedHost)).await;

    for _ in 0..16 {
        let next_identity = SessionId::new();
        let next = boards.open(next_identity, Arc::new(UnusedHost)).await;
        assert_eq!(
            boards
                .states
                .lock()
                .await
                .keys()
                .copied()
                .collect::<HashSet<_>>(),
            HashSet::from([identity, next_identity]),
        );
        drop(next);
    }

    let shared = boards.open(identity, Arc::new(UnusedHost)).await;
    assert!(Arc::ptr_eq(&live.state, &shared.state));
    assert_eq!(
        boards
            .states
            .lock()
            .await
            .keys()
            .copied()
            .collect::<HashSet<_>>(),
        HashSet::from([identity]),
    );
}
