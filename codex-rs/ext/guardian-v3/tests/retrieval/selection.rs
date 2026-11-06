//! Reviewer subscriptions stay inside the fixed Guardian catalog.

use codex_guardian_v3::ReviewerSubscriptions;
use codex_guardian_v3::local_board;
use codex_message_board::AppendRequest;
use codex_message_board::LocalBoard;
use codex_message_board::PostDestination;
use codex_message_board::SubscriptionChange::Subscribe;
use codex_message_board::SubscriptionRequest;
use codex_message_board::SubscriptionState;
use codex_message_board::SubscriptionTarget;
use codex_message_board::UpsertChannelRequest;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_state::SqliteConfig;
use codex_tools::ToolCall;
use codex_tools::ToolCallSource;
use codex_tools::ToolName;
use codex_tools::ToolPayload;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::AtomicUsize;

use crate::board_host;

fn host(owner: ThreadId, reviewer: ThreadId, path: &AgentPath) -> Arc<board_host::Host> {
    Arc::new(board_host::Host {
        clock: AtomicI64::default(),
        fail_clock: AtomicBool::new(false),
        agent_path_calls: AtomicUsize::default(),
        members: [(owner, AgentPath::root()), (reviewer, path.clone())].into(),
        active: AtomicBool::new(true),
        fail_notifications: AtomicBool::new(false),
        notifications: Mutex::default(),
    })
}

#[tokio::test]
async fn subscription_tools_only_change_the_caller_and_return_captured_post_ids()
-> anyhow::Result<()> {
    let policy = TruncationPolicyConfig::bytes(/*limit*/ 1024);
    let owner = ThreadId::new();
    let reviewer = ThreadId::new();
    let path = AgentPath::root()
        .join("reviewer")
        .map_err(anyhow::Error::msg)?;
    let host = host(owner, reviewer, &path);
    let dir = tempfile::tempdir()?;
    let sqlite = SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into()?);
    let agent_board = LocalBoard::new(sqlite.clone(), owner.into())
        .read_only()
        .open(host.clone())
        .await?;
    assert!(
        ReviewerSubscriptions::capture(Arc::new(agent_board), reviewer, path.clone())
            .await
            .is_err()
    );
    let storage = local_board(sqlite, owner.into());
    let writer = storage.read_write().open(host.clone()).await?;
    let original = writer
        .append(
            owner,
            AppendRequest {
                request_id: "original".into(),
                destination: PostDestination::NewChannel("release".into()),
                text: "Original".into(),
            },
        )
        .await?;
    let board = Arc::new(storage.read_only().open(host.clone()).await?);
    board
        .set_subscription(
            owner,
            SubscriptionRequest {
                target: SubscriptionTarget::Channel("release".into()),
                change: Subscribe,
                target_agent: None,
            },
        )
        .await?;
    let subscriptions =
        Arc::new(ReviewerSubscriptions::capture(Arc::clone(&board), reviewer, path.clone()).await?);
    writer
        .upsert_channel(
            owner,
            UpsertChannelRequest {
                channel_name: "later".into(),
                description: "Later channel".to_owned().try_into()?,
            },
        )
        .await?;
    let later = writer
        .append(
            owner,
            AppendRequest {
                request_id: "later".into(),
                destination: PostDestination::Channel("release".into()),
                text: "Later evidence".into(),
            },
        )
        .await?;
    assert_ne!(original.message_id, later.message_id);
    // Tool calls reuse the real backend but return only the captured catalog boundary.
    let tools = subscriptions.tools(Some("guardian"), "Guardian evidence");
    for (name, arguments, succeeds) in [
        ("subscribe", json!({"channel_name":"release"}), true),
        ("unsubscribe", json!({"channel_name":"release"}), true),
        ("subscribe", json!({"channel_name":"release"}), true),
        ("subscribe", json!({"channel_name":"later"}), false),
        (
            "unsubscribe",
            json!({"channel_name":"release","target_agent":"/"}),
            false,
        ),
    ] {
        let tool = tools
            .iter()
            .find(|tool| tool.tool_name().name == name && tool.exposure().is_direct())
            .ok_or_else(|| anyhow::anyhow!("missing direct subscription tool {name}"))?;
        let call = ToolCall {
            turn_id: "review".into(),
            call_id: name.into(),
            tool_name: ToolName::namespaced("guardian", name),
            model: "test".into(),
            codex_turn_metadata: None,
            truncation_policy: policy.into(),
            source: ToolCallSource::Direct,
            conversation_history: Default::default(),
            turn_item_emitter: Arc::new(codex_tools::NoopTurnItemEmitter),
            attachment_store: Arc::new(codex_attachment_store::InlineAttachmentStore),
            ephemeral: false,
            environments: Vec::new(),
            payload: ToolPayload::Function {
                arguments: arguments.to_string(),
            },
        };
        let result = tool.handle(call.clone()).await;
        if succeeds {
            let result = result?;
            let response = result.code_mode_result(&call.payload);
            assert_eq!(result.log_output(), response.to_string());
            let output: SubscriptionState = serde_json::from_value(response)?;
            assert_eq!(
                output,
                SubscriptionState {
                    channel_name: "release".into(),
                    thread_id: None,
                    target_agent: path.clone(),
                    enabled: name == "subscribe",
                    last_message_id: Some(original.message_id),
                }
            );
            if name == "unsubscribe" {
                assert!(subscriptions.channels().await?.is_empty());
            }
        } else {
            assert!(result.is_err());
        }
    }
    assert_eq!(
        subscriptions.channels().await?,
        BTreeSet::from(["release".into()])
    );
    assert_eq!(
        board
            .subscribed_channels(owner, Default::default())
            .await?
            .results,
        vec!["release".to_owned()]
    );
    assert!(
        host.notifications
            .lock()
            .map_err(|_| anyhow::anyhow!("notifications poisoned"))?
            .is_empty()
    );
    Ok(())
}
