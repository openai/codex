//! Guardian can change its own live channel subscriptions and cannot change another actor's.

use codex_guardian_v3::LiveRetrievalBinding;
use codex_guardian_v3::local_board;
use codex_message_board::AgentMessageBoard;
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
use codex_state::SqliteConfig;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;

use crate::retrieval_host;
use crate::reviewer_tool_call;

#[tokio::test]
async fn subscription_tools_only_change_the_callers_channels() -> anyhow::Result<()> {
    let owner = ThreadId::new();
    let reviewer = ThreadId::new();
    let path = AgentPath::root()
        .join("reviewer")
        .map_err(anyhow::Error::msg)?;
    let host = retrieval_host([(owner, AgentPath::root()), (reviewer, path.clone())]);
    let dir = tempfile::tempdir()?;
    let sqlite = SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into()?);
    let agent_board = LocalBoard::new(sqlite.clone(), owner.into())
        .read_only()
        .open(host.clone())
        .await?;
    // A Guardian reviewer must reject the ordinary board, even for the same root.
    assert!(LiveRetrievalBinding::new(Arc::new(agent_board), reviewer, path.clone()).is_err());
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
    let board: Arc<dyn AgentMessageBoard> = Arc::new(storage.read_only().open(host.clone()).await?);
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
    let tools = LiveRetrievalBinding::new(board.clone(), reviewer, path.clone())?.tools();
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
    for (name, arguments, succeeds) in [
        ("subscribe", json!({"channel_name":"release"}), true),
        ("unsubscribe", json!({"channel_name":"release"}), true),
        ("subscribe", json!({"channel_name":"release"}), true),
        ("subscribe", json!({"channel_name":"later"}), true),
        ("unsubscribe", json!({"channel_name":"later"}), true),
        (
            "subscribe",
            json!({"thread_id": original.message_id}),
            false,
        ),
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
        let call = reviewer_tool_call(&tool.tool_name(), &arguments, /*byte_limit*/ 1024);
        let result = tool.handle(call.clone()).await;
        if succeeds {
            let result = result?;
            let response = result.code_mode_result(&call.payload);
            assert_eq!(result.log_output(), response.to_string());
            let output: SubscriptionState = serde_json::from_value(response)?;
            let channel = arguments["channel_name"]
                .as_str()
                .expect("channel subscription");
            let last_message_id = (channel == "release").then_some(later.message_id);
            assert_eq!(
                output,
                SubscriptionState {
                    channel_name: channel.into(),
                    thread_id: None,
                    target_agent: path.clone(),
                    enabled: name == "subscribe",
                    last_message_id,
                }
            );
            if name == "unsubscribe" && channel == "release" {
                assert!(
                    board
                        .subscribed_channels(reviewer, Default::default())
                        .await?
                        .results
                        .is_empty()
                );
            }
        } else {
            assert!(result.is_err());
        }
    }
    assert_eq!(
        board
            .subscribed_channels(reviewer, Default::default())
            .await?
            .results,
        vec!["release".to_owned()]
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
