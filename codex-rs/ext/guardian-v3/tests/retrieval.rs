//! Live Guardian tools can search unsubscribed channels and manage only the caller’s subscriptions.

#![allow(clippy::expect_used)]

use codex_guardian_v3::LiveDelivery;
use codex_guardian_v3::LiveRetrievalBinding;
use codex_guardian_v3::local_board;
use codex_message_board::AppendRequest;
use codex_message_board::Page;
use codex_message_board::PostDestination;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_state::SqliteConfig;
use codex_tools::ToolName;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::AtomicUsize;

#[path = "../../../message-board/tests/common/mod.rs"]
mod board_host;
#[path = "retrieval/guidance.rs"]
mod guidance;
#[path = "retrieval/subscriptions.rs"]
mod subscriptions;
#[path = "common/reviewer_tool_call.rs"]
mod tool_calls;

use tool_calls::invoke;
use tool_calls::reviewer_tool_call;

fn retrieval_host(
    members: impl IntoIterator<Item = (ThreadId, AgentPath)>,
) -> Arc<board_host::Host> {
    Arc::new(board_host::Host {
        clock: AtomicI64::default(),
        fail_clock: AtomicBool::default(),
        agent_path_calls: AtomicUsize::default(),
        members: members.into_iter().collect(),
        active: AtomicBool::new(true),
        fail_notifications: AtomicBool::default(),
        notifications: Mutex::default(),
    })
}

#[tokio::test]
async fn searches_live_and_unsubscribed_guardian_posts() -> anyhow::Result<()> {
    let owner = ThreadId::new();
    let dir = tempfile::tempdir()?;
    let host = retrieval_host([(owner, AgentPath::root())]);
    let storage = local_board(
        SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into()?),
        owner.into(),
    );
    let board = storage.read_write().open(host.clone()).await?;
    let routed = |ordinal: u64, text: &str| {
        json!({
            "id": {"ordinal": ordinal},
            "original": {"type": "response_item", "payload": {
                "type": "message", "role": "user",
                "content": [{"type": "input_text", "text": text}]
            }}
        })
        .to_string()
    };
    let earlier = routed(1, r#"Do not publish "prod" from C:\work\draft."#);
    let first = board
        .append(
            owner,
            AppendRequest {
                request_id: "first".into(),
                destination: PostDestination::NewChannel("release".into()),
                text: earlier.clone(),
            },
        )
        .await?;
    let call = json!({
        "id": {"ordinal": 2},
        "original": {"type": "response_item", "payload": {
            "type": "function_call",
            "name": "exec_command",
            "call_id": "call-2",
            "arguments": r#"{"recipient":"Alice","cmd":"publish \"prod\" from C:\\work\\draft","paths":["C:\\tmp","C:\\work\\plan-\u00c9"]}"#
        }}
    })
    .to_string();
    let second = board
        .append(
            owner,
            AppendRequest {
                request_id: "second".into(),
                destination: PostDestination::Channel("release".into()),
                text: call.clone(),
            },
        )
        .await?;
    let reader = Arc::new(storage.read_only().open(host).await?);
    let delivery = Arc::new(LiveDelivery::new(owner));
    let binding = LiveRetrievalBinding::new(reader, owner, AgentPath::root(), &delivery)?;
    let tools = binding.tools();
    let latest = routed(3, r#"Publish "prod" from C:\work\draft now."#);
    let third = board
        .append(
            owner,
            AppendRequest {
                request_id: "later".into(),
                destination: PostDestination::Channel("release".into()),
                text: latest.clone(),
            },
        )
        .await?;
    assert_eq!(
        invoke(&binding, "subscribed_channels", json!({})).await?["results"],
        json!([])
    );

    let expected_names = [
        "get_channels",
        "list_threads",
        "search_posts",
        "read_thread",
        "read_post",
        "subscribed_channels",
        "subscribe",
        "unsubscribe",
    ];
    let tool_names = tools
        .iter()
        .map(|tool| tool.tool_name())
        .collect::<Vec<_>>();
    assert_eq!(
        LiveRetrievalBinding::tool_names().collect::<Vec<_>>(),
        tool_names
    );
    assert_eq!(
        tool_names,
        expected_names
            .into_iter()
            .map(|name| ToolName::namespaced("guardian_board", name))
            .collect::<Vec<_>>()
    );
    let tool_name = ToolName::namespaced("guardian_board", "search_posts");
    let search = tools
        .iter()
        .find(|tool| tool.tool_name() == tool_name)
        .expect("board search");
    let ordered = [(&third, &latest), (&second, &call), (&first, &earlier)];
    for (query, expected) in [
        (r#"publish "prod""#, ordered.as_slice()),
        (r"C:\work\draft", ordered.as_slice()),
        (r"C:\work\plan-é", &ordered[1..2]),
        ("plan-é", &ordered[1..2]),
        (r#""recipient":"Alice""#, &ordered[1..2]),
        (r#""type":"function_call""#, &ordered[1..2]),
        ("\t", &ordered[..0]),
    ] {
        let call = reviewer_tool_call(
            &tool_name,
            &json!({ "query": query }),
            /*byte_limit*/ 8_000,
        );
        let result = search.handle(call.clone()).await?;
        let output = result.code_mode_result(&call.payload);
        assert_eq!(result.log_output(), output.to_string());
        let page: codex_message_board::Page<codex_message_board::PostPreview> =
            serde_json::from_value(output)?;
        assert_eq!(
            page,
            codex_message_board::Page {
                results: expected
                    .iter()
                    .copied()
                    .map(|(metadata, text)| codex_message_board::PostPreview {
                        metadata: metadata.clone(),
                        text_preview: text.clone(),
                        n_chars: text.chars().count(),
                        truncated: false,
                    })
                    .collect(),
                next_cursor: None,
            }
        );
    }
    Ok(())
}

#[tokio::test]
async fn lists_only_the_reviewers_own_subscriptions_across_pages() -> anyhow::Result<()> {
    let build = "build-instructions-for-release";
    let release = "release-approvals-and-revocations";
    let root = ThreadId::new();
    let actor = ThreadId::new();
    let path = AgentPath::root()
        .join("worker")
        .map_err(anyhow::Error::msg)?;
    let host = retrieval_host([(root, AgentPath::root()), (actor, path.clone())]);
    let dir = tempfile::tempdir()?;
    let storage = local_board(
        SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into()?),
        root.into(),
    );
    let board = Arc::new(storage.read_only().open(host).await?);
    let delivery = Arc::new(LiveDelivery::new(root));
    let reviewer = LiveRetrievalBinding::new(board.clone(), actor, path, &delivery)?;
    let other = LiveRetrievalBinding::new(board, root, AgentPath::root(), &delivery)?;
    for (binding, channel) in [
        (&reviewer, build),
        (&other, "private"),
        (&reviewer, release),
    ] {
        invoke(binding, "subscribe", json!({"channel_name":channel})).await?;
    }
    let name = ToolName::namespaced("guardian_board", "subscribed_channels");
    let tool = reviewer
        .tools()
        .into_iter()
        .find(|tool| tool.tool_name() == name)
        .expect("subscription listing");
    let list = async |arguments: Value| -> anyhow::Result<Page<String>> {
        // The output budget fits one channel name and its cursor.
        let call = reviewer_tool_call(&name, &arguments, /*byte_limit*/ 96);
        let result = tool.handle(call.clone()).await?;
        Ok(serde_json::from_value(
            result.code_mode_result(&call.payload),
        )?)
    };

    let first = list(json!({ "limit": 2 })).await?;
    let last = list(json!({ "limit": 1, "cursor": first.next_cursor })).await?;
    assert_eq!(
        (first.results, last),
        (
            vec![build.to_owned()],
            Page {
                results: vec![release.to_owned()],
                next_cursor: None,
            }
        )
    );
    assert!(list(json!({ "target_agent": "/" })).await.is_err());
    Ok(())
}
