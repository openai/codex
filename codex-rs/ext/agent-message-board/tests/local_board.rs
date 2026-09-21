//! Exercises real local storage through independent handles and host capabilities.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI64;
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
    clock: AtomicI64,
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
        Box::pin(async move {
            Ok(DateTime::parse_from_rfc3339("2026-09-18T12:00:00Z")
                .map_err(|error| CodexErr::Io(std::io::Error::other(error)))?
                .with_timezone(&Utc)
                + chrono::Duration::seconds(self.clock.fetch_add(1, Ordering::SeqCst)))
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
        clock: AtomicI64::default(),
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
    // Posting still succeeds when the subscriber lookup returns an empty array.
    resumed
        .post(
            root,
            PostRequest {
                request_id: "no-subscribers".into(),
                destination: PostDestination::Channel("proofs".into()),
                text: "saved without notifications".into(),
                agents_to_notify: Vec::new(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        *host.notifications.lock().unwrap(),
        vec![(child, metadata.clone())]
    );
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
        clock: AtomicI64::default(),
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

#[tokio::test]
async fn queries_enforce_page_and_preview_caps() {
    let dir = tempfile::tempdir().unwrap();
    let sqlite = SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into().unwrap());
    let root = ThreadId::new();
    let host = Arc::new(Host {
        fail_notifications: AtomicBool::new(false),
        clock: AtomicI64::default(),
        members: [(root, AgentPath::root())].into(),
        active: AtomicBool::new(false),
        notifications: Mutex::default(),
    });
    let board = LocalAgentMessageBoard::open(&sqlite, root.into(), host)
        .await
        .unwrap();
    board
        .create_channel(
            root,
            CreateChannelRequest {
                channel_name: "Straße".into(),
                subscription: SubscriptionChange::Unsubscribe,
            },
        )
        .await
        .unwrap();
    let text = format!("Straße{}", "x".repeat(1000));
    for index in 0..51 {
        board
            .post(
                root,
                PostRequest {
                    request_id: index.to_string(),
                    destination: PostDestination::Channel("Straße".into()),
                    text: text.clone(),
                    agents_to_notify: vec![],
                },
            )
            .await
            .unwrap();
    }
    let channels = board
        .list_channels(
            root,
            ChannelQuery {
                query: Some("STRASSE".into()),
                direction: SortDirection::NewestFirst,
                page: PageRequest::default(),
            },
        )
        .await
        .unwrap();
    assert_eq!(channels.results.len(), 1);
    let query = PostQuery {
        channel_name: None,
        query: Some("STRASSE".into()),
        after_message_id: None,
        author: None,
        page: PageRequest {
            cursor: None,
            limit: NonZeroU32::MAX,
        },
        max_chars_per_post: NonZeroU32::MAX,
    };
    let page = board.search_posts(root, query.clone()).await.unwrap();
    assert_eq!(page.results.len(), 50);
    assert_eq!(
        page.results
            .iter()
            .map(|post| post.text_preview.chars().count())
            .sum::<usize>(),
        20_000
    );
    assert!(page.results.iter().all(|post| post.truncated));
    let last = board
        .search_posts(
            root,
            PostQuery {
                page: PageRequest {
                    cursor: page.next_cursor,
                    limit: NonZeroU32::MAX,
                },
                ..query
            },
        )
        .await
        .unwrap();
    assert_eq!(last.results.len(), 1);
    assert_eq!(last.next_cursor, None);
}

#[tokio::test]
async fn queries_page_discussions_and_search_unicode() {
    let dir = tempfile::tempdir().unwrap();
    let sqlite = SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into().unwrap());
    let root = ThreadId::new();
    let host = Arc::new(Host {
        fail_notifications: AtomicBool::new(false),
        clock: AtomicI64::default(),
        members: [(root, AgentPath::root())].into(),
        active: AtomicBool::new(true),
        notifications: Mutex::default(),
    });
    let board: Arc<dyn AgentMessageBoard> = Arc::new(
        LocalAgentMessageBoard::open(&sqlite, SessionId::from(root), host)
            .await
            .unwrap(),
    );
    let request = PostRequest {
        request_id: "first".into(),
        destination: PostDestination::NewChannel("work".into()),
        text: "Éclair".into(),
        agents_to_notify: vec![],
    };
    let first = board.post(root, request).await.unwrap();
    let second = board
        .post(
            root,
            PostRequest {
                request_id: "second".into(),
                destination: PostDestination::Channel("work".into()),
                text: "second".into(),
                agents_to_notify: vec![],
            },
        )
        .await
        .unwrap();
    let one = NonZeroU32::new(1).unwrap();
    let query = ThreadQuery {
        channel_name: "work".into(),
        sort: ThreadSort::Activity,
        direction: SortDirection::OldestFirst,
        page: PageRequest {
            cursor: None,
            limit: one,
        },
        max_chars_per_post: one,
    };
    let page = board.list_threads(root, query.clone()).await.unwrap();
    assert_eq!(
        page.results,
        vec![ThreadSummary {
            thread_id: first.message_id,
            root_post: PostPreview {
                metadata: first.clone(),
                text_preview: "É".into(),
                n_chars: 6,
                truncated: true,
            },
            reply_count: 0,
            last_activity_at: first.created_at,
            latest_reply: None,
        }]
    );
    let next = board
        .list_threads(
            root,
            ThreadQuery {
                page: PageRequest {
                    cursor: page.next_cursor,
                    limit: one,
                },
                ..query.clone()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        next.results,
        vec![ThreadSummary {
            thread_id: second.message_id,
            root_post: PostPreview {
                metadata: second.clone(),
                text_preview: "s".into(),
                n_chars: 6,
                truncated: true,
            },
            reply_count: 0,
            last_activity_at: second.created_at,
            latest_reply: None,
        }]
    );
    assert_eq!(next.next_cursor, None);
    let reply = board
        .post(
            root,
            PostRequest {
                request_id: "reply".into(),
                destination: PostDestination::Thread(first.message_id),
                text: "ÉCLAIR🦀".into(),
                agents_to_notify: vec![],
            },
        )
        .await
        .unwrap();
    // A reply changes activity order without changing creation order.
    let newest = ThreadQuery {
        direction: SortDirection::NewestFirst,
        ..query
    };
    assert_eq!(
        board
            .list_threads(root, newest.clone())
            .await
            .unwrap()
            .results,
        vec![ThreadSummary {
            thread_id: first.message_id,
            root_post: PostPreview {
                metadata: first.clone(),
                text_preview: "É".into(),
                n_chars: 6,
                truncated: true,
            },
            reply_count: 1,
            last_activity_at: reply.created_at,
            latest_reply: Some(PostPreview {
                metadata: reply.clone(),
                text_preview: "É".into(),
                n_chars: 7,
                truncated: true,
            }),
        }]
    );
    let created = board
        .list_threads(
            root,
            ThreadQuery {
                sort: ThreadSort::Created,
                ..newest
            },
        )
        .await
        .unwrap();
    assert_eq!(created.results[0].thread_id, second.message_id);
    let search = board
        .search_posts(
            root,
            PostQuery {
                channel_name: Some("work".into()),
                query: Some("éclair".into()),
                after_message_id: Some(first.message_id),
                author: Some(AgentPath::root()),
                page: PageRequest::default(),
                max_chars_per_post: one,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        search,
        Page {
            results: vec![PostPreview {
                metadata: reply.clone(),
                text_preview: "É".into(),
                n_chars: 7,
                truncated: true
            }],
            next_cursor: None
        }
    );
    let thread = board
        .read_thread(
            root,
            ReadThreadRequest {
                thread_id: first.message_id,
                page: PageRequest::default(),
                max_chars_per_post: one,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        thread,
        ThreadPage {
            root_post: PostPreview {
                metadata: first,
                text_preview: "É".into(),
                n_chars: 6,
                truncated: true
            },
            replies: search
        }
    );
    let serialized = serde_json::to_value(thread).unwrap();
    assert_eq!(serialized["n_returned"], 1);
    assert_eq!(serialized["has_more"], false);
    let channels = board
        .list_channels(
            root,
            ChannelQuery {
                query: Some("WORK".into()),
                direction: SortDirection::NewestFirst,
                page: PageRequest::default(),
            },
        )
        .await
        .unwrap();
    assert_eq!(channels.results[0].message_count, 3);
    assert_eq!(channels.results[0].last_message_id, Some(reply.message_id));
    assert!(
        board
            .read_thread(
                root,
                ReadThreadRequest {
                    thread_id: reply.message_id,
                    page: PageRequest::default(),
                    max_chars_per_post: one
                }
            )
            .await
            .is_err()
    );
    assert!(
        board
            .list_channels(
                ThreadId::new(),
                ChannelQuery {
                    query: None,
                    direction: SortDirection::NewestFirst,
                    page: PageRequest::default()
                }
            )
            .await
            .is_err()
    );
}

fn board_tool_call(name: &str, args: serde_json::Value) -> codex_tools::ToolCall<'static> {
    codex_tools::ToolCall {
        turn_id: "turn-1".into(),
        call_id: name.into(),
        tool_name: codex_tools::ToolName::namespaced("collaboration", name),
        model: "test".into(),
        codex_turn_metadata: None,
        truncation_policy: codex_utils_output_truncation::TruncationPolicy::Bytes(20_000),
        source: codex_tools::ToolCallSource::Direct,
        conversation_history: codex_tools::ConversationHistory::default(),
        turn_item_emitter: Arc::new(codex_tools::NoopTurnItemEmitter),
        environments: vec![],
        payload: codex_tools::ToolPayload::Function {
            arguments: args.to_string(),
        },
    }
}

#[tokio::test]
async fn tools_cover_channel_discussions_subscriptions_and_escaped_previews() {
    use codex_tools::ToolName;
    use serde_json::json;
    let dir = tempfile::tempdir().unwrap();
    let sqlite = SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into().unwrap());
    let root = ThreadId::new();
    let child = ThreadId::new();
    let child_path = AgentPath::root().join("worker").unwrap();
    let host = Arc::new(Host {
        fail_notifications: AtomicBool::new(false),
        members: [(root, AgentPath::root()), (child, child_path.clone())].into(),
        clock: AtomicI64::default(),
        active: AtomicBool::new(true),
        notifications: Mutex::default(),
    });
    let board = Arc::new(
        LocalAgentMessageBoard::open(&sqlite, root.into(), host.clone())
            .await
            .unwrap(),
    );
    let tools = message_board_tools(
        board,
        root,
        AgentPath::root(),
        Some("collaboration"),
        "Shared tools",
    );
    let tool = |name: &str| {
        tools
            .iter()
            .find(|tool| tool.tool_name() == ToolName::namespaced("collaboration", name))
            .unwrap()
    };
    let created = tool("create_channel")
        .handle(board_tool_call(
            "create_channel",
            json!({"channel_name":"Workflow"}),
        ))
        .await
        .unwrap();
    let created: ChannelSummary = serde_json::from_str(&created.log_output()).unwrap();
    let channels = tool("get_channels")
        .handle(board_tool_call("get_channels", json!({"query":"WORK"})))
        .await
        .unwrap();
    let channels: Page<ChannelSummary> = serde_json::from_str(&channels.log_output()).unwrap();
    assert_eq!(
        channels,
        Page {
            results: vec![created],
            next_cursor: None
        }
    );
    for (name, enabled) in [("subscribe", true), ("unsubscribe", false)] {
        let result = tool(name)
            .handle(board_tool_call(
                name,
                json!({"channel_name":"Workflow","target_agent":"worker"}),
            ))
            .await
            .unwrap();
        let state: SubscriptionState = serde_json::from_str(&result.log_output()).unwrap();
        assert_eq!(
            state,
            SubscriptionState {
                channel_name: "Workflow".into(),
                thread_id: None,
                target_agent: child_path.clone(),
                enabled,
                last_message_id: None,
            }
        );
    }
    let mut last = None;
    let mut expected_roots = Vec::new();
    for index in 0..7 {
        let mut call = board_tool_call(
            "post",
            json!({"channel_name":"Workflow","text":"\u{1}".repeat(2000)}),
        );
        call.call_id = format!("root-{index}");
        let posted = tool("post").handle(call).await.unwrap();
        let post: PostMetadata = serde_json::from_str(&posted.log_output()).unwrap();
        expected_roots.push(post.thread_id.to_string());
        let mut call = board_tool_call(
            "post",
            json!({"thread_id":post.thread_id,"text":"\u{1}".repeat(2000)}),
        );
        call.call_id = format!("reply-{index}");
        let replied = tool("post").handle(call).await.unwrap();
        let reply: PostMetadata = serde_json::from_str(&replied.log_output()).unwrap();
        last = Some((post, reply));
    }
    // Default create_channel subscribes the author to roots; post subscribes it
    // to replies. The unsubscribed child receives neither.
    assert_eq!(
        host.notifications
            .lock()
            .unwrap()
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>(),
        vec![root; 14]
    );
    let (last, last_reply) = last.unwrap();
    assert!(
        tool("subscribe")
            .handle(board_tool_call(
                "subscribe",
                json!({"channel_name":"Workflow","thread_id":last.thread_id})
            ))
            .await
            .is_err()
    );
    let subscribed = tool("subscribe")
        .handle(board_tool_call(
            "subscribe",
            json!({"thread_id":last.thread_id,"target_agent":"worker"}),
        ))
        .await
        .unwrap();
    let subscribed: SubscriptionState = serde_json::from_str(&subscribed.log_output()).unwrap();
    assert_eq!(
        subscribed,
        SubscriptionState {
            channel_name: "Workflow".into(),
            thread_id: Some(last.thread_id),
            target_agent: child_path,
            enabled: true,
            last_message_id: Some(last_reply.message_id),
        }
    );
    for (name, args) in [
        ("list_threads", json!({"channel_name":"Workflow"})),
        ("read_thread", json!({"thread_id":last.thread_id})),
    ] {
        let output = tool(name)
            .handle(board_tool_call(name, args))
            .await
            .unwrap();
        assert!(output.log_output().len() <= 8000);
        let value: serde_json::Value = serde_json::from_str(&output.log_output()).unwrap();
        if name == "list_threads" {
            assert_eq!(value["results"][0]["thread_id"], json!(last.thread_id));
            assert!(
                value["results"][0]["root_post"]["truncated"]
                    .as_bool()
                    .unwrap()
            );
            assert!(
                value["results"][0]["latest_reply"]["truncated"]
                    .as_bool()
                    .unwrap()
            );
            let mut page = value;
            let mut roots = Vec::new();
            loop {
                let results = page["results"].as_array().unwrap();
                assert!(!results.is_empty());
                roots.extend(
                    results
                        .iter()
                        .map(|thread| thread["thread_id"].as_str().unwrap().to_string()),
                );
                assert!(roots.len() <= expected_roots.len());
                let Some(cursor) = page["next_cursor"].as_str() else {
                    break;
                };
                let output = tool(name)
                    .handle(board_tool_call(
                        name,
                        json!({"channel_name":"Workflow", "cursor":cursor}),
                    ))
                    .await
                    .unwrap();
                assert!(output.log_output().len() <= 8000);
                page = serde_json::from_str(&output.log_output()).unwrap();
            }
            assert_eq!(
                roots,
                expected_roots.iter().rev().cloned().collect::<Vec<_>>()
            );
        } else {
            assert_eq!(value["n_returned"], json!(1));
            assert_eq!(value["root_post"]["message_id"], json!(last.message_id));
            assert_eq!(
                value["results"][0]["message_id"],
                json!(last_reply.message_id)
            );
        }
    }
}

#[tokio::test]
async fn tools_validate_arguments_deduplicate_calls_and_bound_unicode_results() {
    use codex_tools::ToolCallSource;
    use codex_tools::ToolName;
    use codex_utils_output_truncation::TruncationPolicy;
    use serde_json::json;

    let dir = tempfile::tempdir().unwrap();
    let sqlite = SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into().unwrap());
    let root = ThreadId::new();
    let child = ThreadId::new();
    let child_path = AgentPath::root().join("worker").unwrap();
    let host = Arc::new(Host {
        fail_notifications: AtomicBool::new(false),
        members: [(root, AgentPath::root()), (child, child_path)].into(),
        clock: AtomicI64::default(),
        active: AtomicBool::new(true),
        notifications: Mutex::default(),
    });
    let board = Arc::new(
        LocalAgentMessageBoard::open(&sqlite, root.into(), host.clone())
            .await
            .unwrap(),
    );
    let tools = message_board_tools(
        board.clone(),
        root,
        AgentPath::root(),
        Some("collaboration"),
        "Shared tools",
    );
    let call = board_tool_call;
    let tool = |name: &str| {
        tools
            .iter()
            .find(|tool| tool.tool_name() == ToolName::namespaced("collaboration", name))
            .unwrap()
    };
    for (name, args) in [
        (
            "post",
            json!({"new_channel_name":"low-budget","text":"must not be stored"}),
        ),
        ("create_channel", json!({"channel_name":"low-budget"})),
    ] {
        let mut limited = call(name, args);
        limited.truncation_policy = TruncationPolicy::Bytes(1);
        assert!(tool(name).handle(limited).await.is_err());
    }
    assert!(
        tool("post")
            .handle(call(
                "post",
                json!({"text":"invalid","channel_name":"a","new_channel_name":"b"})
            ))
            .await
            .is_err()
    );
    assert!(
        tool("get_channels")
            .handle(call("get_channels", json!({"limit":0})))
            .await
            .is_err()
    );
    assert!(
        tool("get_channels")
            .handle(call("get_channels", json!({"typo":true})))
            .await
            .is_err()
    );
    assert!(
        board
            .list_channels(
                root,
                ChannelQuery {
                    query: None,
                    direction: SortDirection::NewestFirst,
                    page: PageRequest::default()
                }
            )
            .await
            .unwrap()
            .results
            .is_empty()
    );

    let post_call = call(
        "post",
        json!({"text":"🦀".repeat(8000),"new_channel_name":"work","agents_to_notify":["worker"]}),
    );
    let result = tool("post").handle(post_call.clone()).await.unwrap();
    let metadata: PostMetadata = serde_json::from_str(&result.log_output()).unwrap();
    assert_eq!(
        tool("post").handle(post_call).await.unwrap().log_output(),
        result.log_output()
    );
    assert_eq!(host.notifications.lock().unwrap().len(), 1);
    let preview = tool("search_posts")
        .handle(call("search_posts", json!({"query":"🦀"})))
        .await
        .unwrap();
    let preview: Page<PostPreview> = serde_json::from_str(&preview.log_output()).unwrap();
    assert_eq!(
        preview,
        Page {
            results: vec![PostPreview {
                metadata: metadata.clone(),
                text_preview: "🦀".repeat(1000),
                n_chars: 8000,
                truncated: true,
            }],
            next_cursor: None,
        }
    );
    let read_call = call("read_post", json!({"message_id":metadata.message_id}));
    let result = tool("read_post").handle(read_call.clone()).await.unwrap();
    assert!(result.contains_external_context());
    assert!(result.log_output().len() <= 8000);
    let first: PostContent = serde_json::from_str(&result.log_output()).unwrap();
    assert_eq!(first.text, "🦀".repeat(first.next_offset_chars));
    assert_eq!(first.n_chars, 8000);
    let result = tool("read_post").handle(call("read_post", json!({"message_id":metadata.message_id,"offset_chars":first.next_offset_chars,"limit_chars":2}))).await.unwrap();
    let second: PostContent = serde_json::from_str(&result.log_output()).unwrap();
    assert_eq!(second.text, "🦀🦀");
    assert_eq!(second.next_offset_chars, first.next_offset_chars + 2);

    let mut nested = call(
        "post",
        json!({"text":"reply","thread_id":metadata.thread_id}),
    );
    nested.source = ToolCallSource::CodeMode {
        cell_id: "cell-1".into(),
        runtime_tool_call_id: "nested-1".into(),
    };
    let first_nested = tool("post")
        .handle(nested.clone())
        .await
        .unwrap()
        .log_output();
    nested.source = ToolCallSource::CodeMode {
        cell_id: "cell-1".into(),
        runtime_tool_call_id: "nested-2".into(),
    };
    assert_ne!(
        tool("post").handle(nested).await.unwrap().log_output(),
        first_nested
    );
    let result = tool("search_posts")
        .handle(call(
            "search_posts",
            json!({"query":"reply","author":"/root"}),
        ))
        .await
        .unwrap();
    let page: Page<PostPreview> = serde_json::from_str(&result.log_output()).unwrap();
    assert_eq!(page.results.len(), 2);
    assert!(page.results.iter().all(|post| post.text_preview == "reply"));

    let mut nested_read = read_call;
    nested_read.source = ToolCallSource::CodeMode {
        cell_id: "cell-1".into(),
        runtime_tool_call_id: "nested-3".into(),
    };
    assert!(
        tool("read_post")
            .handle(nested_read)
            .await
            .unwrap()
            .log_output()
            .len()
            <= 8000
    );
}
