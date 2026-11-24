//! Guardian can change its own live channel subscriptions and cannot change another actor's.

use codex_guardian_v3::LiveDelivery;
use codex_guardian_v3::LiveRetrievalBinding;
use codex_guardian_v3::local_board;
use codex_message_board::AgentMessageBoard;
use codex_message_board::AppendRequest;
use codex_message_board::LocalBoard;
use codex_message_board::PostDestination;
use codex_message_board::SubscriptionChange::Subscribe;
use codex_message_board::SubscriptionRequest;
use codex_message_board::SubscriptionTarget;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_state::SqliteConfig;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;

use crate::invoke;
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
    let delivery = Arc::new(LiveDelivery::new(owner));
    // A Guardian reviewer must reject the ordinary board, even for the same root.
    assert!(
        LiveRetrievalBinding::new(Arc::new(agent_board), reviewer, path.clone(), &delivery)
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
    let board: Arc<dyn AgentMessageBoard> = Arc::new(storage.read_only().open(host.clone()).await?);
    // An upgraded board may already hold subscriptions from a previous reviewer.
    board
        .set_subscription(
            reviewer,
            SubscriptionRequest {
                target: SubscriptionTarget::Channel("release".into()),
                change: Subscribe,
                target_agent: None,
            },
        )
        .await?;
    let binding = LiveRetrievalBinding::new(board, reviewer, path.clone(), &delivery)?;
    let tools = binding.tools();
    let channels = async || -> anyhow::Result<serde_json::Value> {
        Ok(invoke(&binding, "subscribed_channels", json!({})).await?["results"].take())
    };
    assert_eq!(channels().await?, json!([]));
    for (name, arguments, succeeds) in [
        ("subscribe", json!({"channel_name":"release"}), true),
        ("unsubscribe", json!({"channel_name":"release"}), true),
        ("subscribe", json!({"channel_name":"later"}), true),
        ("subscribe", json!({"channel_name":"release "}), false),
        ("unsubscribe", json!({"channel_name":"missing"}), true),
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
            let response = result.code_mode_result(&call.payload).into_json();
            assert_eq!(result.log_output(), response.to_string());
            assert_eq!(
                response,
                json!({
                    "channel_name": arguments["channel_name"],
                    "thread_id": null,
                    "target_agent": path,
                    "enabled": name == "subscribe",
                })
            );
        } else {
            assert!(result.is_err());
        }
    }
    assert_eq!(channels().await?, json!(["later"]));
    assert!(
        host.notifications
            .lock()
            .map_err(|_| anyhow::anyhow!("notifications poisoned"))?
            .is_empty()
    );
    Ok(())
}
