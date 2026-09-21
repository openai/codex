//! Verifies pool sharing and release without exposing storage internals.

use super::*;
use crate::NotificationDelivery;
use crate::PostPreview;
use futures::future::BoxFuture;
use pretty_assertions::assert_ne;

struct UnusedHost;

impl MessageBoardHost for UnusedHost {
    fn agent_path(&self, _caller: ThreadId) -> BoxFuture<'_, Result<AgentPath>> {
        unreachable!("pool tests do not invoke host capabilities")
    }

    fn resolve_agent(&self, _path: AgentPath) -> BoxFuture<'_, Result<ThreadId>> {
        unreachable!("pool tests do not invoke host capabilities")
    }

    fn current_time(&self, _caller: ThreadId) -> BoxFuture<'_, Result<DateTime<Utc>>> {
        unreachable!("pool tests do not invoke host capabilities")
    }

    fn notify(
        &self,
        _recipient: ThreadId,
        _post: PostPreview,
    ) -> BoxFuture<'_, Result<NotificationDelivery>> {
        unreachable!("pool tests do not invoke host capabilities")
    }
}

#[tokio::test]
async fn concurrent_handles_share_and_release_the_pool() {
    let dir = tempfile::tempdir().unwrap();
    let sqlite = SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into().unwrap());
    let (first, second) = tokio::join!(
        LocalAgentMessageBoard::open(&sqlite, SessionId::new(), Arc::new(UnusedHost)),
        LocalAgentMessageBoard::open(&sqlite, SessionId::new(), Arc::new(UnusedHost)),
    );
    let first = first.unwrap();
    let second = second.unwrap();
    assert!(Arc::ptr_eq(&first.pool, &second.pool));
    let original = Arc::downgrade(&first.pool);
    drop(first);
    assert!(Arc::ptr_eq(&original.upgrade().unwrap(), &second.pool));
    drop(second);
    assert!(original.upgrade().is_none());

    let reopened = LocalAgentMessageBoard::open(&sqlite, SessionId::new(), Arc::new(UnusedHost))
        .await
        .unwrap();
    assert_ne!(original.as_ptr(), Arc::as_ptr(&reopened.pool));
    reopened.pool.close().await;
}
