//! Runs existing board tools against a research-provisioned remote board.

use super::super::super::code_mode::custom_tool_output_last_non_empty_text;
use super::BoardClock;
use super::configure;
use super::done;
use anyhow::Context;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_features::RemoteMessageBoardConfigToml;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::context_snapshot::SnapshotEntry;
use core_test_support::responses;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::Notify;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_board_uses_the_existing_tools_and_session_identity() -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let board = responses::start_mock_server().await;
    let token = "research-board-credential-for-runtime-test";
    let url = board.uri();
    let root = test_codex()
        .with_config(move |config| {
            configure(config);
            config.ephemeral = true;
            config.current_time_reminder = Some(codex_core::config::CurrentTimeReminderConfig {
                clock_source: codex_features::CurrentTimeSource::External,
                ..Default::default()
            });
            config.multi_agent_v2.message_board_remote = Some(RemoteMessageBoardConfigToml {
                url,
                bearer_token: Some(token.into()),
                bearer_token_env_var: None,
            });
        })
        .with_model_info_override("gpt-5.5", |model| {
            model.multi_agent_version = Some(codex_protocol::protocol::MultiAgentVersion::V2);
        })
        .with_external_time_provider(Arc::new(BoardClock::Available))
        .build_with_auto_env(&server)
        .await?;
    let root_id = root.session_configured.thread_id;
    let post = json!({
        "message_id": "00000000-0000-4000-8000-000000000001",
        "thread_id": "00000000-0000-4000-8000-000000000001",
        "author": "/root", "channel_name": "design", "created_at": "2026-09-18T12:00:00Z",
    });
    Mock::given(method("POST"))
        .and(path(format!("/v1/boards/{root_id}/call")))
        .and(header("authorization", format!("Bearer {token}")))
        .and(body_partial_json(json!({
            "caller": root_id,
            "timestamp": "2026-09-18T12:00:00Z",
            "method": "post",
            "params": {
                "destination": {"NewChannel": "design"},
                "text": "A remote decision.", "agents_to_notify": []
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(&post))
        .expect(1)
        .mount(&board)
        .await;
    let connection = AtomicUsize::default();
    let interrupted_turn = OnceLock::new();
    let connecting = Arc::new(Notify::new());
    let connected = connecting.clone();
    let (release_handshake, receive_release) = std::sync::mpsc::channel();
    let receive_release = std::sync::Mutex::new(receive_release);
    Mock::given(method("POST"))
        .and(path(format!("/v1/boards/{root_id}/notifications")))
        .and(header("authorization", format!("Bearer {token}")))
        .and(body_partial_json(json!({"caller": root_id})))
        .respond_with(move |request: &wiremock::Request| {
            let attempt = connection.fetch_add(/*val*/ 1, Ordering::Relaxed);
            // Interrupt the first handshake. On the next turn, fail setup,
            // disconnect, fail a reconnect, then deliver on the same receiver.
            match attempt {
                0 | 4 => {}
                2 => {
                    return ResponseTemplate::new(200)
                        .insert_header("content-type", "text/event-stream")
                        .set_body_string("event: ready\ndata: {}\n\n");
                }
                _ => return ResponseTemplate::new(503),
            }
            let watch: Value = request.body_json().expect("notification watch");
            let notice = json!({
                "recipient": root_id, "turn_id": watch["turn_id"],
                "post": {
                    "message_id": "00000000-0000-4000-8000-000000000002",
                    "thread_id": "00000000-0000-4000-8000-000000000002",
                    "author": "/root/worker", "channel_name": "design",
                    "created_at": "2026-09-18T12:00:00Z",
                    "text_preview": "Worker's remote decision.", "n_chars": 25, "truncated": false
                }
            });
            let mut stale = notice.clone();
            stale["turn_id"] = interrupted_turn.get_or_init(|| watch["turn_id"].clone()).clone();
            let response = ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(format!(
                    "event: ready\ndata: {{}}\n\nevent: notification\ndata: invalid-json\n\nevent: notification\ndata: {stale}\n\nevent: notification\ndata: {notice}\n\n"
                ));
            if attempt == 0 {
                connected.notify_one();
                // Wiremock runs on its own thread; keep the handshake pending
                // until the test confirms interruption, regardless of runner speed.
                let _ = receive_release.lock().expect("handshake gate").recv();
            }
            response
        })
        .expect(5..)
        .mount(&board)
        .await;
    let model = responses::mount_sse_sequence(
        &server,
        vec![
            // Wait in the same model step so pending mail cannot be drained before the wait.
            responses::sse(vec![
                responses::ev_function_call_with_namespace(
                    "remote-post",
                    "collaboration",
                    "post",
                    &json!({"new_channel_name":"design", "text":"A remote decision."}).to_string(),
                ),
                responses::ev_function_call_with_namespace(
                    "await-notification",
                    "collaboration",
                    "wait_agent",
                    "{}",
                ),
                responses::ev_completed("remote-post"),
            ]),
            done(),
        ],
    )
    .await;
    root.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![super::super::text(
            "This turn will be interrupted during notification setup.",
        )]))
        .await?;
    connecting.notified().await;
    root.codex.submit(Op::Interrupt).await?;
    wait_for_event(&root.codex, |event| {
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;
    release_handshake.send(())?;
    root.submit_turn("Post the remote decision.").await?;
    let requests = model.requests();
    let last_request = requests.last().context("model request")?;
    let notices = last_request.inputs_of_type("agent_message");
    let mut notification = responses::strip_metadata_from_json(
        responses::strip_response_item_ids_from_json(json!(notices)),
    );
    notification.sort_all_objects();
    insta::assert_snapshot!(
        "remote_board_notification",
        serde_json::to_string_pretty(&notification)?
    );
    // Normalize tool JSON key order across Cargo and Bazel feature sets.
    let mut bodies = requests
        .iter()
        .map(responses::ResponsesRequest::body_json)
        .collect::<Vec<_>>();
    for body in &mut bodies {
        for item in body["input"].as_array_mut().context("request input")? {
            if item["type"] == "function_call_output" {
                let mut output: Value =
                    serde_json::from_str(item["output"].as_str().context("output")?)?;
                output.sort_all_objects();
                item["output"] = Value::String(output.to_string());
            }
        }
    }
    insta::assert_snapshot!(
        "remote_board_tools",
        context_snapshot::format_context_snapshot(
            "Remote board tools and active-turn notifications share the existing session identity.",
            &bodies.iter().map(SnapshotEntry::body).collect::<Vec<_>>(),
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    let request_count = board
        .received_requests()
        .await
        .context("board requests")?
        .len();
    // A completed turn must stop the receiver, including its next scheduled retry.
    tokio::time::sleep(Duration::from_millis(/*millis*/ 1_100)).await;
    assert_eq!(
        board
            .received_requests()
            .await
            .context("board requests")?
            .len(),
        request_count
    );
    root.codex.shutdown_and_wait().await?;
    Ok(())
}

#[test_case::test_case(false; "direct")]
#[test_case::test_case(true; "code_mode")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_board_effective_permissions_and_denials(code_mode: bool) -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let board = responses::start_mock_server().await;
    let url = board.uri();
    let root = test_codex()
        .with_model("gpt-6-astra")
        .with_config(move |config| {
            configure(config);
            config.ephemeral = true;
            for feature in [
                Feature::CodeMode,
                Feature::CodeModeOnly,
                Feature::CodeModeHost,
            ] {
                config
                    .features
                    .set_enabled(feature, code_mode)
                    .expect("configure code mode");
            }
            config.multi_agent_v2.message_board_remote = Some(RemoteMessageBoardConfigToml {
                url,
                bearer_token: Some("board-runtime-credential-for-acl-test".into()),
                bearer_token_env_var: None,
            });
        })
        .with_model_info_override("gpt-6-astra", move |model| {
            model.tool_mode = Some(if code_mode {
                ToolMode::CodeModeOnly
            } else {
                ToolMode::Direct
            });
        })
        .build_with_auto_env(&server)
        .await?;
    let root_id = root.session_configured.thread_id;
    let metadata = json!({"message_id":"00000000-0000-4000-8000-000000000002","thread_id":"00000000-0000-4000-8000-000000000001","author":"/root","channel_name":"policy","created_at":"2026-09-18T12:00:00Z"});
    let legacy = json!({"results":[{"channel_name":"policy","created_at":"2026-09-18T12:00:00Z","created_by":"/root","message_count":1,"last_message_id":metadata["thread_id"]}],"n_returned":1,"has_more":false,"next_cursor":null});
    let mut restricted = legacy.clone();
    restricted["board_permissions"] =
        json!({"create_channel":false,"edit_metadata":false,"manage_permissions":false});
    restricted["results"][0]["permissions"] =
        json!({"read":true,"post":false,"reply":true,"edit_metadata":false});
    let denied = json!({"code":"permission_denied","message":"You can reply, but cannot start threads in policy.","action":"channel.post","resource":"board:example/channel:policy"});
    let cases = [
        (
            "legacy",
            "get_channels",
            json!({"query":"legacy"}),
            json!({"method":"list_channels","params":{"query":"legacy"}}),
            200,
            legacy,
        ),
        (
            "permissions",
            "get_channels",
            json!({"query":"policy"}),
            json!({"method":"list_channels","params":{"query":"policy"}}),
            200,
            restricted,
        ),
        (
            "denied",
            "post",
            json!({"channel_name":"policy","text":"New policy."}),
            json!({"method":"post","params":{"destination":{"Channel":"policy"}}}),
            403,
            denied,
        ),
        (
            "reply",
            "post",
            json!({"thread_id":metadata["thread_id"],"text":"A clarification?"}),
            json!({"method":"post","params":{"destination":{"Thread":metadata["thread_id"]}}}),
            200,
            metadata,
        ),
    ];
    let mut steps = Vec::new();
    for (call_id, tool_name, arguments, request, status, result) in &cases {
        Mock::given(method("POST"))
            .and(path(format!("/v1/boards/{root_id}/call")))
            .and(header(
                "authorization",
                "Bearer board-runtime-credential-for-acl-test",
            ))
            .and(body_partial_json(json!({"caller":root_id})))
            .and(body_partial_json(request))
            .respond_with(ResponseTemplate::new(*status).set_body_json(result))
            .expect(/*r*/ 1)
            .mount(&board)
            .await;
        let mut arguments = arguments.clone();
        arguments.sort_all_objects();
        steps.push(if code_mode {
            responses::sse(vec![
                responses::ev_custom_tool_call(
                    call_id,
                    "exec",
                    &format!("text(await tools.collaboration__{tool_name}({arguments}));"),
                ),
                responses::ev_completed(call_id),
            ])
        } else {
            super::tool(call_id, tool_name, arguments)
        });
    }
    steps.push(done());
    let model = responses::mount_sse_sequence(&server, steps).await;
    root.submit_turn(
        "Check board permissions, try publishing, then ask a question in the existing thread.",
    )
    .await?;
    let requests = model.requests();
    let last = requests.last().context("last request")?;
    for (call_id, _, _, _, status, result) in &cases {
        let expected = if *status == 403 {
            json!({"error":result})
        } else {
            result.clone()
        };
        let output = if code_mode {
            custom_tool_output_last_non_empty_text(last, call_id)
        } else {
            last.function_call_output_text(call_id)
        }
        .context("tool output")?;
        assert_eq!(
            serde_json::from_str::<Value>(&output)?,
            expected,
            "{call_id}"
        );
    }
    // Snapshot the direct request history; both modes assert the tool results above.
    if !code_mode {
        let mut bodies = requests
            .iter()
            .map(responses::ResponsesRequest::body_json)
            .collect::<Vec<_>>();
        for body in &mut bodies {
            for item in body["input"].as_array_mut().context("request input")? {
                if item["type"] == "function_call_output" {
                    item["output"] = context_snapshot::normalize_json_lines(
                        item["output"].as_str().context("output")?,
                    )
                    .into();
                }
            }
        }
        insta::assert_snapshot!(
            "remote_board_permissions_direct",
            context_snapshot::format_context_snapshot(
                "Permission discovery preserves legacy output, exposes reply-only access, and returns a denied post before an allowed reply.",
                &bodies.iter().map(SnapshotEntry::body).collect::<Vec<_>>(),
                &ContextSnapshotOptions::default().rewrite_known_segments(),
            )
        );
    }
    root.codex.shutdown_and_wait().await?;
    Ok(())
}
