use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_fake_rollout;
use app_test_support::to_response;
use codex_app_server_protocol::ForkConversationParams;
use codex_app_server_protocol::ForkConversationResponse;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::NewConversationParams; // reused for overrides shape
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::SessionConfiguredNotification;
use codex_protocol::protocol::EventMsg;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fork_conversation_creates_new_rollout() -> Result<()> {
    let codex_home = TempDir::new()?;

    let preview = "Hello A";
    let conversation_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-02T12-00-00",
        "2025-01-02T12:00:00Z",
        preview,
        Some("openai"),
        None,
    )?;

    let original_path = codex_home
        .path()
        .join("sessions")
        .join("2025")
        .join("01")
        .join("02")
        .join(format!(
            "rollout-2025-01-02T12-00-00-{conversation_id}.jsonl"
        ));
    assert!(
        original_path.exists(),
        "expected original rollout to exist at {}",
        original_path.display()
    );
    let original_contents = std::fs::read_to_string(&original_path)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let fork_req_id = mcp
        .send_fork_conversation_request(ForkConversationParams {
            path: Some(original_path.clone()),
            conversation_id: None,
            overrides: Some(NewConversationParams {
                model: Some("o3".to_string()),
                ..Default::default()
            }),
        })
        .await?;

    // Expect a sessionConfigured notification for the forked session.
    let notification: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("sessionConfigured"),
    )
    .await??;
    let session_configured: ServerNotification = notification.try_into()?;
    let ServerNotification::SessionConfigured(SessionConfiguredNotification {
        model,
        session_id,
        rollout_path,
        initial_messages: session_initial_messages,
        ..
    }) = session_configured
    else {
        unreachable!("expected sessionConfigured notification");
    };

    assert_eq!(model, "o3");
    assert_ne!(
        session_id.to_string(),
        conversation_id,
        "expected a new conversation id when forking"
    );
    assert_ne!(
        rollout_path, original_path,
        "expected a new rollout path when forking"
    );
    assert!(
        rollout_path.exists(),
        "expected forked rollout to exist at {}",
        rollout_path.display()
    );

    let session_initial_messages =
        session_initial_messages.expect("expected initial messages when forking from rollout");
    match session_initial_messages.as_slice() {
        [EventMsg::UserMessage(message)] => {
            assert_eq!(message.message, preview);
        }
        other => panic!("unexpected initial messages from rollout fork: {other:#?}"),
    }

    // Then the response for forkConversation.
    let fork_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(fork_req_id)),
    )
    .await??;
    let ForkConversationResponse {
        conversation_id: forked_id,
        model: forked_model,
        initial_messages: response_initial_messages,
        rollout_path: response_rollout_path,
    } = to_response::<ForkConversationResponse>(fork_resp)?;

    assert_eq!(forked_model, "o3");
    assert_eq!(response_rollout_path, rollout_path);
    assert_ne!(forked_id.to_string(), conversation_id);

    let response_initial_messages =
        response_initial_messages.expect("expected initial messages in fork response");
    match response_initial_messages.as_slice() {
        [EventMsg::UserMessage(message)] => {
            assert_eq!(message.message, preview);
        }
        other => panic!("unexpected initial messages in fork response: {other:#?}"),
    }

    let after_contents = std::fs::read_to_string(&original_path)?;
    assert_eq!(
        after_contents, original_contents,
        "fork should not mutate the original rollout file"
    );

    Ok(())
}
