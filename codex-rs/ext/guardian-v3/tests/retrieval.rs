//! Reviewer tools search routed posts from their capture, including unsubscribed channels.

#![allow(clippy::expect_used)]

use codex_guardian_v3::RetrievalBinding;
use codex_guardian_v3::local_board;
use codex_message_board::AgentMessageBoard;
use codex_message_board::AppendRequest;
use codex_message_board::PageRequest;
use codex_message_board::PostDestination;
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

#[tokio::test]
async fn searches_unsubscribed_guardian_posts_only_through_the_review_cutoff() -> anyhow::Result<()>
{
    let owner = ThreadId::new();
    let dir = tempfile::tempdir()?;
    let host = retrieval_host([(owner, AgentPath::root())]);
    let board = local_board(
        SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into()?),
        owner.into(),
    )
    .read_write()
    .open(host)
    .await?;
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
    let view = board.read_view(owner).await?;
    let binding = RetrievalBinding::new(view.as_ref(), owner, AgentPath::root())?;
    board
        .append(
            owner,
            AppendRequest {
                request_id: "later".into(),
                destination: PostDestination::Channel("release".into()),
                text: routed(3, r#"Publish "prod" from C:\work\draft now."#),
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

    let tools = binding.tools();
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.tool_name())
            .collect::<Vec<_>>(),
        [
            "get_channels",
            "list_threads",
            "search_posts",
            "read_thread",
            "read_post"
        ]
        .map(|name| ToolName::namespaced("guardian_board", name))
    );
    let tool_name = ToolName::namespaced("guardian_board", "search_posts");
    let search = tools
        .iter()
        .find(|tool| tool.tool_name() == tool_name)
        .expect("board search");
    let both = [(&second, &call), (&first, &earlier)];
    for (query, expected) in [
        (r#"publish "prod""#, both.as_slice()),
        (r"C:\work\draft", both.as_slice()),
        (r"C:\work\plan-é", &both[..1]),
        ("plan-é", &both[..1]),
        (r#""recipient":"Alice""#, &both[..1]),
        (r#""type":"function_call""#, &both[..1]),
        ("\t", &both[..0]),
    ] {
        let payload = ToolPayload::Function {
            arguments: json!({"query":query}).to_string(),
        };
        let result = search
            .handle(ToolCall {
                turn_id: "review".into(),
                call_id: "search".into(),
                tool_name: tool_name.clone(),
                model: "test".into(),
                codex_turn_metadata: None,
                truncation_policy: TruncationPolicyConfig::bytes(/*limit*/ 8_000).into(),
                source: ToolCallSource::Direct,
                conversation_history: Default::default(),
                turn_item_emitter: Arc::new(codex_tools::NoopTurnItemEmitter),
                attachment_store: Arc::new(codex_attachment_store::InlineAttachmentStore),
                ephemeral: false,
                environments: Vec::new(),
                payload: payload.clone(),
            })
            .await?;
        let output = result.code_mode_result(&payload);
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
