//! Live and captured reviewer tools can search unsubscribed evidence channels.

#![allow(clippy::expect_used)]

use codex_guardian_v3::LiveRetrievalBinding;
use codex_guardian_v3::RetrievalBinding;
use codex_guardian_v3::local_board;
use codex_message_board::AgentMessageBoard;
use codex_message_board::AppendRequest;
use codex_message_board::CreateChannelRequest;
use codex_message_board::Page;
use codex_message_board::PageRequest;
use codex_message_board::PostDestination;
use codex_message_board::SubscriptionChange;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_state::SqliteConfig;
use codex_tools::ToolCall;
use codex_tools::ToolCallSource;
use codex_tools::ToolName;
use codex_tools::ToolPayload;
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
#[path = "retrieval/delivery.rs"]
mod delivery;
#[path = "retrieval/evidence_board.rs"]
mod evidence_board;
#[path = "retrieval/planning.rs"]
mod planning;
#[path = "retrieval/selection.rs"]
mod selection;

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

#[derive(Copy, Clone, PartialEq, Eq)]
enum BoardReads {
    Captured,
    Live,
}

#[test_case::test_case(BoardReads::Captured; "captured_tools_search_only_through_the_cutoff")]
#[test_case::test_case(BoardReads::Live; "live_tools_search_later_and_unsubscribed_posts")]
#[tokio::test]
async fn searches_guardian_evidence(reads: BoardReads) -> anyhow::Result<()> {
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
    let tools = match reads {
        BoardReads::Captured => {
            let view = reader.read_view(owner).await?;
            RetrievalBinding::new(view.as_ref(), owner, AgentPath::root())?.tools()
        }
        BoardReads::Live => LiveRetrievalBinding::new(reader, owner, AgentPath::root())?.tools(),
    };
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
    assert!(
        board
            .subscribed_channels(owner, PageRequest::default())
            .await?
            .results
            .is_empty()
    );

    let mut expected_names = vec![
        "get_channels",
        "list_threads",
        "search_posts",
        "read_thread",
        "read_post",
    ];
    let tool_names = tools
        .iter()
        .map(|tool| tool.tool_name())
        .collect::<Vec<_>>();
    if reads == BoardReads::Live {
        expected_names.extend(["subscribed_channels", "subscribe", "unsubscribe"]);
        assert_eq!(
            LiveRetrievalBinding::tool_names().collect::<Vec<_>>(),
            tool_names
        );
    }
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
    let all_visible = match reads {
        BoardReads::Captured => &ordered[1..],
        BoardReads::Live => &ordered[..],
    };
    for (query, expected) in [
        (r#"publish "prod""#, all_visible),
        (r"C:\work\draft", all_visible),
        (r"C:\work\plan-é", &ordered[1..2]),
        ("plan-é", &ordered[1..2]),
        (r#""recipient":"Alice""#, &ordered[1..2]),
        (r#""type":"function_call""#, &ordered[1..2]),
        ("\t", &ordered[..0]),
    ] {
        let call = reviewer_tool_call(&tool_name, json!({ "query": query }));
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

fn reviewer_tool_call(name: &ToolName, arguments: Value) -> ToolCall<'static> {
    ToolCall {
        turn_id: "review".into(),
        call_id: name.name.clone(),
        tool_name: name.clone(),
        model: "test".into(),
        codex_turn_metadata: None,
        truncation_policy: TruncationPolicyConfig::bytes(/*limit*/ 8_000).into(),
        source: ToolCallSource::Direct,
        conversation_history: Default::default(),
        turn_item_emitter: Arc::new(codex_tools::NoopTurnItemEmitter),
        attachment_store: Arc::new(codex_attachment_store::InlineAttachmentStore),
        ephemeral: false,
        environments: Vec::new(),
        payload: ToolPayload::Function {
            arguments: arguments.to_string(),
        },
    }
}

#[tokio::test]
async fn lists_only_the_reviewers_own_subscriptions_across_pages() -> anyhow::Result<()> {
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
    let writer = storage.read_write().open(host.clone()).await?;
    for (owner, channel) in [(actor, "build"), (root, "private"), (actor, "release")] {
        writer
            .create_channel(
                owner,
                CreateChannelRequest {
                    channel_name: channel.into(),
                    description: None,
                    subscription: SubscriptionChange::Subscribe,
                },
            )
            .await?;
    }
    let board = Arc::new(storage.read_only().open(host).await?);
    let name = ToolName::namespaced("guardian_board", "subscribed_channels");
    let tool = LiveRetrievalBinding::new(board, actor, path)?
        .tools()
        .into_iter()
        .find(|tool| tool.tool_name() == name)
        .expect("live reviewer subscription lookup");
    let list = async |arguments: Value| -> anyhow::Result<Page<String>> {
        let call = reviewer_tool_call(&name, arguments);
        let result = tool.handle(call.clone()).await?;
        Ok(serde_json::from_value(
            result.code_mode_result(&call.payload),
        )?)
    };

    let first = list(json!({ "limit": 1 })).await?;
    let last = list(json!({ "limit": 1, "cursor": first.next_cursor })).await?;
    assert_eq!(
        (first.results, last),
        (
            vec!["build".to_owned()],
            Page {
                results: vec!["release".to_owned()],
                next_cursor: None,
            }
        )
    );
    assert!(list(json!({ "target_agent": "/" })).await.is_err());
    Ok(())
}
