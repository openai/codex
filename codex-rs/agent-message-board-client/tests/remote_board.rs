//! Exercise the HTTP adapter's clock, authentication and turn-bound stream.

use chrono::DateTime;
use chrono::Utc;
use codex_agent_message_board_client::AccessToken;
use codex_agent_message_board_client::BoardNotification;
use codex_agent_message_board_client::RemoteAgentMessageBoard;
use codex_agent_message_board_extension::AgentMessageBoard;
use codex_agent_message_board_extension::PostContent;
use codex_agent_message_board_extension::PostDestination;
use codex_agent_message_board_extension::PostMetadata;
use codex_agent_message_board_extension::PostPreview;
use codex_agent_message_board_extension::PostRequest;
use codex_agent_message_board_extension::ReadPostRequest;
use codex_http_client::HttpClientBuilder;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::CodexErrorDetails;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_json;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn remote_board_preserves_the_host_contract() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let http = HttpClientBuilder::new().build_direct()?;
    let board = SessionId::new();
    let caller = ThreadId::new();
    let members = HashMap::from([(caller, AgentPath::root())]);
    let endpoint = format!("{}/board-service", server.uri());
    let base = format!("/board-service/v1/boards/{board}");
    let admin_token = AccessToken::new("administrator-credential-for-integration".into())?;
    for invalid in [
        "not a URL",
        "file:///tmp/board",
        "https://board.test/?query",
        "https://board.test/#fragment",
    ] {
        assert!(
            RemoteAgentMessageBoard::new(http.clone(), invalid, board, admin_token.clone())
                .is_err()
        );
    }
    let admin =
        RemoteAgentMessageBoard::new(http.clone(), &format!("{endpoint}/"), board, admin_token)?;
    Mock::given(method("PUT"))
        .and(path(&base))
        .and(header(
            "authorization",
            "Bearer administrator-credential-for-integration",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(board))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(admin.create_board().await?, board);
    let invalid_registration = Mock::given(method("POST"))
        .and(path(format!("{base}/members")))
        .respond_with(ResponseTemplate::new(200).set_body_json("too-short"))
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    let error = admin.register_members(members.clone()).await.unwrap_err();
    assert!(matches!(error.details(), CodexErrorDetails::Json(_)));
    drop(invalid_registration);
    Mock::given(method("POST"))
        .and(path(format!("{base}/members")))
        .and(body_json(json!({"members": members})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json("runtime-credential-for-integration-32"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let token = admin.register_members(members).await?;
    let now: DateTime<Utc> = "2026-09-24T12:00:00Z".parse()?;
    let clock_available = Arc::new(AtomicBool::new(true));
    let clock = clock_available.clone();
    let client =
        RemoteAgentMessageBoard::new(http, &endpoint, board, token)?.with_clock(move |_| {
            let available = clock.load(Ordering::Relaxed);
            Box::pin(async move {
                if available {
                    Ok(now)
                } else {
                    Err(CodexErr::Io(std::io::Error::other("clock unavailable")))
                }
            })
        });
    let metadata: PostMetadata = serde_json::from_value(json!({
        "message_id": "00000000-0000-4000-8000-000000000001", "thread_id": "00000000-0000-4000-8000-000000000001",
        "author": "/root", "channel_name": "results", "created_at": now,
    }))?;
    let post = PostRequest {
        request_id: "call-1".into(),
        destination: PostDestination::NewChannel("results".into()),
        text: "Straße 🦀".into(),
        agents_to_notify: vec![AgentPath::root()],
    };
    Mock::given(method("POST"))
        .and(path(format!("{base}/call")))
        .and(header(
            "authorization",
            "Bearer runtime-credential-for-integration-32",
        ))
        .and(body_json(
            json!({"caller": caller, "timestamp": now, "method": "post", "params": post}),
        ))
        // The post committed, but a gateway failed to return its result.
        .respond_with(
            ResponseTemplate::new(503)
                .set_body_json(json!({"code": "unavailable", "message": "response unavailable"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let board_handle: &dyn AgentMessageBoard = &client;
    assert_eq!(board_handle.identity(), board);
    assert!(board_handle.post(caller, post.clone()).await.is_err());
    clock_available.store(false, Ordering::Relaxed);
    Mock::given(method("POST"))
        .and(path(format!("{base}/call")))
        .and(body_json(
            json!({"caller": caller, "timestamp": null, "method": "post", "params": post}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(&metadata))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(client.post(caller, post).await?, metadata);
    let request = ReadPostRequest {
        message_id: metadata.message_id,
        offset_chars: 7,
        limit_chars: NonZeroU32::MIN,
    };
    let content = PostContent {
        metadata: metadata.clone(),
        text: "🦀".into(),
        n_chars: 8,
        next_offset_chars: 8,
    };
    Mock::given(method("POST"))
        .and(path(format!("{base}/call")))
        .and(body_json(
            json!({"caller": caller, "timestamp": null, "method": "read_post", "params": request}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(&content))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(client.read_post(caller, request).await?, content);
    let notice = BoardNotification {
        recipient: caller,
        turn_id: "turn-1".into(),
        post: PostPreview {
            metadata,
            text_preview: "Straße 🦀".into(),
            n_chars: 8,
            truncated: false,
        },
    };
    let mut stale = notice.clone();
    stale.turn_id = "turn-0".into();
    let mut oversized = notice.clone();
    oversized.post.text_preview = "🦀".repeat(151);
    let events = [&notice, &stale, &oversized]
        .into_iter()
        .map(|event| {
            Ok(format!(
                "event: notification\ndata: {}\n\n",
                serde_json::to_string(event)?
            ))
        })
        .collect::<anyhow::Result<String>>()?;
    // The limit is per frame, not per connection; support all SSE line endings.
    let heartbeats = ": heartbeat\r\n\r\n: heartbeat\r\r: heartbeat\n\n".repeat(16 * 1024);
    Mock::given(method("POST"))
        .and(path(format!("{base}/notifications")))
        .and(body_json(json!({"caller": caller, "turn_id": "turn-1"})))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(format!("{heartbeats}event: ready\ndata: {{}}\n\n{events}")),
        )
        .expect(1)
        .mount(&server)
        .await;
    let mut receiver = client.notifications(caller, "turn-1".into()).await?;
    assert_eq!(receiver.next().await?, Some(notice));
    assert!(receiver.next().await.is_err());
    assert!(receiver.next().await.is_err());
    for body in [
        format!(
            "event: ready\nid: {}\ndata: {{}}\n\n",
            "x".repeat(512 * 1024)
        ),
        format!("event: ready\ndata: {{}}\n\nid: {}", "x".repeat(512 * 1024)),
        format!(
            "event: ready\ndata: {{}}\n\n{}",
            "data: x\n".repeat(64 * 1024 + 1)
        ),
    ] {
        let _response = Mock::given(method("POST"))
            .and(path(format!("{base}/notifications")))
            .and(body_json(json!({"caller": caller, "turn_id": "limit"})))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(body),
            )
            .expect(1)
            .mount_as_scoped(&server)
            .await;
        let error = match client.notifications(caller, "limit".into()).await {
            Ok(mut receiver) => receiver.next().await.unwrap_err(),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("board SSE frame exceeds the service limit")
        );
    }
    let invalid_response = Mock::given(method("DELETE"))
        .and(path(&base))
        .respond_with(ResponseTemplate::new(502).set_body_string("not JSON"))
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    let error = admin.delete_board().await.unwrap_err();
    assert!(error.to_string().contains("HTTP 502"));
    assert!(
        anyhow::Error::new(error)
            .chain()
            .any(<dyn std::error::Error>::is::<serde_json::Error>)
    );
    drop(invalid_response);
    Mock::given(method("DELETE"))
        .and(path(base))
        .respond_with(
            ResponseTemplate::new(503)
                .set_body_json(json!({"code": "unavailable", "message": "storage unavailable"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    assert!(
        admin
            .delete_board()
            .await
            .unwrap_err()
            .to_string()
            .contains("storage unavailable")
    );
    Ok(())
}
