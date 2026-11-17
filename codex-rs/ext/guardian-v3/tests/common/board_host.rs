//! A local board host whose next write can be held until the test releases it.

use chrono::DateTime;
use chrono::Utc;
use codex_message_board::MessageBoardHost;
use codex_message_board::NotificationDelivery;
use codex_message_board::PostPreview;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::error::Result as CodexResult;
use futures::future::BoxFuture;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::sync::Notify;

#[path = "../../../../message-board/tests/common/mod.rs"]
mod shared;

pub(crate) fn board_host(root: ThreadId) -> Arc<shared::Host> {
    board_host_with_members([(root, AgentPath::root())])
}

pub(crate) fn board_host_with_members(
    members: impl IntoIterator<Item = (ThreadId, AgentPath)>,
) -> Arc<shared::Host> {
    Arc::new(shared::Host {
        clock: AtomicI64::default(),
        fail_clock: AtomicBool::new(false),
        agent_path_calls: AtomicUsize::default(),
        members: members.into_iter().collect(),
        active: AtomicBool::new(true),
        fail_notifications: AtomicBool::new(false),
        notifications: Mutex::default(),
    })
}

pub(crate) struct PausedClockHost {
    pub(crate) inner: Arc<shared::Host>,
    pub(crate) pause_next: AtomicBool,
    pub(crate) started: Notify,
    pub(crate) release: Notify,
}

impl MessageBoardHost for PausedClockHost {
    fn agent_path(&self, caller: ThreadId) -> BoxFuture<'_, CodexResult<AgentPath>> {
        self.inner.agent_path(caller)
    }

    fn resolve_agent(&self, path: AgentPath) -> BoxFuture<'_, CodexResult<ThreadId>> {
        self.inner.resolve_agent(path)
    }

    fn current_time(&self, caller: ThreadId) -> BoxFuture<'_, CodexResult<DateTime<Utc>>> {
        Box::pin(async move {
            if self.pause_next.swap(false, Ordering::SeqCst) {
                self.started.notify_one();
                self.release.notified().await;
            }
            self.inner.current_time(caller).await
        })
    }

    fn notify(
        &self,
        recipient: ThreadId,
        post: PostPreview,
    ) -> BoxFuture<'_, CodexResult<NotificationDelivery>> {
        self.inner.notify(recipient, post)
    }
}
