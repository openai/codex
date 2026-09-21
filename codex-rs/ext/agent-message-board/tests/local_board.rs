//! Exercises real local storage through independent handles and host capabilities.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use chrono::DateTime;
use chrono::Utc;
use codex_agent_message_board_extension::*;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use codex_state::SqliteConfig;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;

struct Host {
    members: HashMap<ThreadId, AgentPath>,
    active: AtomicBool,
    fail_notifications: AtomicBool,
    notifications: Mutex<Vec<(ThreadId, PostMetadata)>>,
}

impl MessageBoardHost for Host {
    fn agent_path(&self, caller: ThreadId) -> BoxFuture<'_, Result<AgentPath>> {
        Box::pin(async move {
            self.members
                .get(&caller)
                .cloned()
                .ok_or_else(|| CodexErr::ThreadNotFound(caller))
        })
    }

    fn resolve_agent(&self, path: AgentPath) -> BoxFuture<'_, Result<ThreadId>> {
        Box::pin(async move {
            self.members
                .iter()
                .find_map(|(id, member)| (*member == path).then_some(*id))
                .ok_or_else(|| CodexErr::InvalidRequest("unknown agent".into()))
        })
    }

    fn current_time(&self, _caller: ThreadId) -> BoxFuture<'_, Result<DateTime<Utc>>> {
        Box::pin(async {
            Ok(DateTime::parse_from_rfc3339("2026-09-18T12:00:00Z")
                .map_err(|error| CodexErr::Io(std::io::Error::other(error)))?
                .with_timezone(&Utc))
        })
    }

    fn notify(
        &self,
        recipient: ThreadId,
        post: PostMetadata,
    ) -> BoxFuture<'_, Result<NotificationDelivery>> {
        Box::pin(async move {
            if self.fail_notifications.load(Ordering::SeqCst) {
                return Err(CodexErr::Io(std::io::Error::other(
                    "notification transport failed",
                )));
            }
            if !self.active.load(Ordering::SeqCst) {
                return Ok(NotificationDelivery::SkippedInactive);
            }
            self.notifications
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push((recipient, post));
            Ok(NotificationDelivery::Accepted)
        })
    }
}

#[tokio::test]
async fn shared_handles_resume_posts_and_preserve_subscription_rules() {
    let dir = tempfile::tempdir().unwrap();
    let sqlite = SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into().unwrap());
    let root = ThreadId::new();
    let child = ThreadId::new();
    let child_path = AgentPath::root().join("worker").unwrap();
    let host = Arc::new(Host {
        members: [(root, AgentPath::root()), (child, child_path.clone())].into(),
        fail_notifications: AtomicBool::new(false),
        active: AtomicBool::new(true),
        notifications: Mutex::default(),
    });
    let tree = SessionId::from(root);
    let first = LocalAgentMessageBoard::open(&sqlite, tree, host.clone())
        .await
        .unwrap();
    first
        .create_channel(
            child,
            CreateChannelRequest {
                channel_name: "proofs".into(),
                subscription: SubscriptionChange::Subscribe,
            },
        )
        .await
        .unwrap();
    let request = PostRequest {
        request_id: "call-1".into(),
        destination: PostDestination::Channel("proofs".into()),
        text: "é🦀 proof".into(),
        agents_to_notify: vec![child_path.clone(), child_path.clone()],
    };
    let second = LocalAgentMessageBoard::open(&sqlite, tree, host.clone())
        .await
        .unwrap();
    let (metadata, duplicate) = tokio::join!(
        first.post(root, request.clone()),
        second.post(root, request.clone()),
    );
    let metadata = metadata.unwrap();
    assert_eq!(duplicate.unwrap(), metadata);
    drop(second);
    assert_eq!(
        *host.notifications.lock().unwrap(),
        vec![(child, metadata.clone())]
    );
    drop(first);

    let resumed = LocalAgentMessageBoard::open(&sqlite, tree, host.clone())
        .await
        .unwrap();
    assert_eq!(resumed.post(root, request).await.unwrap(), metadata);
    assert_eq!(host.notifications.lock().unwrap().len(), 1);
    let content = resumed
        .read_post(
            child,
            ReadPostRequest {
                message_id: metadata.message_id,
                offset_chars: 1,
                limit_chars: NonZeroU32::new(2).unwrap(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        content,
        PostContent {
            metadata: metadata.clone(),
            text: "🦀 ".into(),
            n_chars: 8,
            next_offset_chars: 3,
        }
    );
    resumed
        .set_subscription(
            root,
            SubscriptionRequest {
                target: SubscriptionTarget::Channel("proofs".into()),
                target_agent: Some(child_path.clone()),
                change: SubscriptionChange::Unsubscribe,
            },
        )
        .await
        .unwrap();
    resumed
        .set_subscription(
            root,
            SubscriptionRequest {
                target: SubscriptionTarget::Thread(metadata.message_id),
                target_agent: Some(child_path),
                change: SubscriptionChange::Subscribe,
            },
        )
        .await
        .unwrap();
    let reply = resumed
        .post(
            root,
            PostRequest {
                request_id: "reply".into(),
                destination: PostDestination::Thread(metadata.message_id),
                text: "done".into(),
                agents_to_notify: Vec::new(),
            },
        )
        .await
        .unwrap();
    let mut received = host.notifications.lock().unwrap().clone();
    received.sort_by_key(|(id, post)| (id.to_string(), post.message_id));
    let mut expected = vec![
        (child, metadata.clone()),
        (root, reply.clone()),
        (child, reply),
    ];
    expected.sort_by_key(|(id, post)| (id.to_string(), post.message_id));
    assert_eq!(received, expected);

    let other = LocalAgentMessageBoard::open(&sqlite, SessionId::new(), host.clone())
        .await
        .unwrap();
    assert!(
        other
            .read_post(
                root,
                ReadPostRequest {
                    message_id: metadata.message_id,
                    offset_chars: 0,
                    limit_chars: NonZeroU32::new(20).unwrap(),
                }
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn failed_requests_do_not_create_channels_or_notify_inactive_agents() {
    let dir = tempfile::tempdir().unwrap();
    let sqlite = SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into().unwrap());
    let root = ThreadId::new();
    let host = Arc::new(Host {
        members: [(root, AgentPath::root())].into(),
        fail_notifications: AtomicBool::new(false),
        active: AtomicBool::new(false),
        notifications: Mutex::default(),
    });
    let board = LocalAgentMessageBoard::open(&sqlite, SessionId::from(root), host.clone())
        .await
        .unwrap();
    let mut request = PostRequest {
        request_id: "post".into(),
        destination: PostDestination::NewChannel("work".into()),
        text: "first".into(),
        agents_to_notify: vec![AgentPath::root().join("unknown").unwrap()],
    };
    assert!(board.post(root, request.clone()).await.is_err());
    request.agents_to_notify = vec![AgentPath::root()];
    let posted = board.post(root, request.clone()).await.unwrap();
    assert_eq!(*host.notifications.lock().unwrap(), Vec::new());
    host.active.store(true, Ordering::SeqCst);
    assert_eq!(board.post(root, request.clone()).await.unwrap(), posted);
    assert_eq!(*host.notifications.lock().unwrap(), Vec::new());
    request.text = "different".into();
    assert!(board.post(root, request).await.is_err());

    // Notification failure must not turn a committed write into a failed post.
    host.fail_notifications.store(true, Ordering::SeqCst);
    let reply = PostRequest {
        request_id: "reply".into(),
        destination: PostDestination::Thread(posted.thread_id),
        text: "saved even if the notice fails".into(),
        agents_to_notify: vec![],
    };
    let saved = board.post(root, reply.clone()).await.unwrap();
    host.fail_notifications.store(false, Ordering::SeqCst);
    assert_eq!(board.post(root, reply).await.unwrap(), saved);
    assert_eq!(*host.notifications.lock().unwrap(), Vec::new());
    assert_eq!(
        board
            .read_post(
                root,
                ReadPostRequest {
                    message_id: saved.message_id,
                    offset_chars: 0,
                    limit_chars: NonZeroU32::new(100).unwrap(),
                }
            )
            .await
            .unwrap(),
        PostContent {
            metadata: saved,
            text: "saved even if the notice fails".into(),
            n_chars: 30,
            next_offset_chars: 30,
        }
    );
}
