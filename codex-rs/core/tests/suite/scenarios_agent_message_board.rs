//! Exercises board tools through the runtime, including resume and active-only notices.

use anyhow::Context;
use codex_agent_message_board_extension::PostMetadata;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::user_input::UserInput;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::context_snapshot::SnapshotEntry;
use core_test_support::responses;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio::sync::oneshot;

enum BoardClock {
    Available,
    Unavailable,
}

impl codex_core::TimeProvider for BoardClock {
    fn current_time(&self, _thread_id: codex_protocol::ThreadId) -> codex_core::TimeFuture<'_> {
        Box::pin(async move {
            match self {
                Self::Available => Ok(chrono::DateTime::parse_from_rfc3339(
                    "2026-09-18T12:00:00Z",
                )?
                .with_timezone(&chrono::Utc)),
                Self::Unavailable => Err(anyhow::anyhow!("board clock unavailable")),
            }
        })
    }

    fn sleep(
        &self,
        _thread_id: codex_protocol::ThreadId,
        _duration: std::time::Duration,
    ) -> codex_core::SleepFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

fn tool(call: &str, name: &str, arguments: Value) -> String {
    sse(vec![
        ev_function_call_with_namespace(call, "collaboration", name, &arguments.to_string()),
        ev_completed(call),
    ])
}

fn done() -> String {
    sse(vec![
        ev_assistant_message("done", "Done."),
        ev_completed("done"),
    ])
}

fn configure(config: &mut codex_core::config::Config) {
    super::configure_scenario_catalog(config);
    config
        .features
        .enable(Feature::AgentMessageBoard)
        .expect("enable board");
    config
        .features
        .enable(Feature::MultiAgentV2)
        .expect("enable agent paths");
}

#[test_case::test_case(false, true, false; "feature_off")]
#[test_case::test_case(true, false, false; "legacy_agents")]
#[test_case::test_case(true, true, true; "ephemeral")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn board_requires_persistent_v2_runtime(
    board_enabled: bool,
    multi_agent_v2: bool,
    ephemeral: bool,
) -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let mock = responses::mount_sse_once(&server, done()).await;
    let test = test_codex()
        .with_config(move |config| {
            configure(config);
            if !board_enabled {
                config
                    .features
                    .disable(Feature::AgentMessageBoard)
                    .expect("disable board");
            }
            if !multi_agent_v2 {
                config
                    .features
                    .disable(Feature::MultiAgentV2)
                    .expect("disable agent paths");
            }
            config.ephemeral = ephemeral;
        })
        .build_with_auto_env(&server)
        .await?;
    test.submit_turn("Hello.").await?;
    assert!(
        responses::namespace_child_tool(
            &mock.single_request().body_json(),
            "collaboration",
            "post"
        )
        .is_none()
    );
    assert!(
        !test
            .config
            .sqlite_config()
            .home()
            .join("agent_message_board_1.sqlite")
            .exists()
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn board_post_and_reads_reach_model_context_without_self_notices() -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let mock = responses::mount_sse_sequence(&server, vec![
        tool("post-decision", "post", json!({"new_channel_name":"design", "text":"A shared decision.", "agents_to_notify":["/root"]})),
        tool("find-decision", "search_posts", json!({"channel_name":"design","query":"decision"})),
        done(),
    ]).await;
    let test = test_codex()
        .with_config(|config| {
            configure(config);
            config.current_time_reminder = Some(codex_core::config::CurrentTimeReminderConfig {
                clock_source: codex_features::CurrentTimeSource::External,
                ..Default::default()
            });
        })
        .with_external_time_provider(std::sync::Arc::new(BoardClock::Available))
        .build_with_auto_env(&server)
        .await?;
    test.submit_turn("Record the decision on the board, notify me, and read it.")
        .await?;
    let requests = mock.requests();
    assert_eq!(requests.len(), 3);
    assert!(
        responses::namespace_child_tool(&requests[0].body_json(), "collaboration", "post")
            .is_some()
    );
    let output = requests[1]
        .function_call_output_text("post-decision")
        .expect("post result");
    let post: Value =
        serde_json::from_str(&output).with_context(|| format!("post result: {output}"))?;
    assert_eq!(post["author"], "/root");
    assert_eq!(post["created_at"], "2026-09-18T12:00:00Z");
    let result: Value = serde_json::from_str(
        &requests[2]
            .function_call_output_text("find-decision")
            .expect("search result"),
    )?;
    assert_eq!(result["results"][0]["text_preview"], "A shared decision.");
    assert!(
        requests
            .iter()
            .all(|request| !request.body_contains_text("Message Type: CHANNEL_POST"))
    );
    // Cargo and Bazel can enable different serde_json ordering features. Normalize only
    // the snapshot copies so the assertion compares JSON content, not object key order.
    let mut bodies = requests
        .iter()
        .map(ResponsesRequest::body_json)
        .collect::<Vec<_>>();
    for body in &mut bodies {
        for item in body["input"].as_array_mut().context("request input")? {
            if item["type"] == "function_call_output" {
                let mut output: Value =
                    serde_json::from_str(item["output"].as_str().context("board output")?)?;
                output.sort_all_objects();
                item["output"] = Value::String(output.to_string());
            }
        }
    }
    insta::assert_snapshot!(
        "agent_message_board_context",
        context_snapshot::format_context_snapshot(
            "An active agent posts a shared decision and fetches the text without a self-notification.",
            &bodies.iter().map(SnapshotEntry::body).collect::<Vec<_>>(),
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn board_is_shared_with_children_survives_resume_and_skips_idle_notices() -> anyhow::Result<()>
{
    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(configure);
    let root = builder.build_with_auto_env(&server).await?;
    responses::mount_sse_sequence(
        &server,
        vec![
            tool(
                "create-design",
                "create_channel",
                json!({"channel_name":"design"}),
            ),
            done(),
        ],
    )
    .await;
    root.submit_turn("Create the design channel and subscribe.")
        .await?;
    responses::mount_sse_sequence(
        &server,
        vec![
            tool(
                "spawn-worker",
                "spawn_agent",
                json!({"task_name":"worker","message":"Say ready.","fork_turns":"none"}),
            ),
            done(),
            done(),
        ],
    )
    .await;
    root.submit_turn("Spawn a worker and finish your turn.")
        .await?;
    let child_id = root
        .thread_manager
        .list_thread_ids()
        .await
        .into_iter()
        .find(|id| *id != root.session_configured.thread_id)
        .expect("child runtime");
    let child = root.thread_manager.get_thread(child_id).await?;
    wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    let child_post = responses::mount_sse_sequence(
        &server,
        vec![
            tool(
                "worker-post",
                "post",
                json!({"channel_name":"design","text":"Worker's durable decision."}),
            ),
            done(),
        ],
    )
    .await;
    child
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Post your decision.".into(),
            text_elements: vec![],
        }]))
        .await?;
    wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    let requests = child_post.requests();
    let output = requests[1]
        .function_call_output_text("worker-post")
        .expect("child post result");
    let post: Value =
        serde_json::from_str(&output).with_context(|| format!("child post result: {output}"))?;
    assert_eq!(post["author"], "/root/worker");
    assert!(matches!(
        root.codex.agent_status().await,
        codex_protocol::protocol::AgentStatus::Completed(_)
    ));
    let read = responses::mount_sse_sequence(
        &server,
        vec![
            tool(
                "root-read",
                "search_posts",
                json!({"channel_name":"design"}),
            ),
            done(),
        ],
    )
    .await;
    root.submit_turn("Read the worker's decision.").await?;
    assert!(
        read.requests()
            .iter()
            .all(|request| !request.body_contains_text("Message Type: CHANNEL_POST"))
    );
    let output = read.requests()[1]
        .function_call_output_text("root-read")
        .expect("read result");
    let result: Value =
        serde_json::from_str(&output).with_context(|| format!("root read result: {output}"))?;
    assert_eq!(result["results"][0]["message_id"], post["message_id"]);
    child.shutdown_and_wait().await?;
    let resumed = test_codex()
        .with_config(configure)
        .restart(&server, &root)
        .await?;
    let read = responses::mount_sse_sequence(
        &server,
        vec![
            tool(
                "resumed-read",
                "read_post",
                json!({"message_id":post["message_id"]}),
            ),
            done(),
        ],
    )
    .await;
    resumed.submit_turn("Read the saved decision.").await?;
    let output = read.requests()[1]
        .function_call_output_text("resumed-read")
        .expect("resumed read result");
    let result: Value =
        serde_json::from_str(&output).with_context(|| format!("resumed read result: {output}"))?;
    assert_eq!(result["text"], "Worker's durable decision.");
    assert_eq!(result["author"], "/root/worker");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn board_unsubscribe_survives_post_and_resume_until_resubscribed() -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let root = test_codex()
        .with_config(configure)
        .build_with_auto_env(&server)
        .await?;
    let posted = responses::mount_sse_sequence(
        &server,
        vec![
            tool(
                "start-discussion",
                "post",
                json!({"new_channel_name":"design","text":"A shared decision."}),
            ),
            done(),
        ],
    )
    .await;
    root.submit_turn("Start a discussion.").await?;
    let post: Value = serde_json::from_str(
        &posted
            .function_call_output_text("start-discussion")
            .context("initial post result")?,
    )?;
    let thread_id = &post["thread_id"];
    let opted_out = responses::mount_sse_sequence(
        &server,
        vec![
            tool("opt-out", "unsubscribe", json!({"thread_id":thread_id})),
            tool(
                "last-detail",
                "post",
                json!({"thread_id":thread_id,"text":"One last detail."}),
            ),
            done(),
        ],
    )
    .await;
    root.submit_turn("Unsubscribe and add one last detail.")
        .await?;
    let subscription: Value = serde_json::from_str(
        &opted_out
            .function_call_output_text("opt-out")
            .context("unsubscribe result")?,
    )?;
    assert_eq!(subscription["enabled"], false);
    let _: PostMetadata = serde_json::from_str(
        &opted_out
            .function_call_output_text("last-detail")
            .context("post after unsubscribe")?,
    )?;

    let response = |body| vec![StreamingSseChunk { gate: None, body }];
    let (release_wait, wait_gate) = oneshot::channel();
    let mut streams = vec![
        vec![
            StreamingSseChunk {
                gate: None,
                body: sse(vec![ev_function_call_with_namespace(
                    "spawn-worker",
                    "collaboration",
                    "spawn_agent",
                    &json!({"task_name":"worker","message":"Say ready.","fork_turns":"none"})
                        .to_string(),
                )]),
            },
            StreamingSseChunk {
                // Dispatch wait only after spawn, while keeping the completion in this batch.
                gate: Some(wait_gate),
                body: sse(vec![
                    ev_function_call_with_namespace(
                        "settle-worker",
                        "collaboration",
                        "wait_agent",
                        "{}",
                    ),
                    ev_completed("spawn-and-wait"),
                ]),
            },
        ],
        response(done()),
        response(done()),
    ];
    let mut phases = Vec::new();
    for (phase, subscribed) in [("opted-out", false), ("resubscribed", true)] {
        if subscribed {
            streams.push(response(tool(
                "opt-in",
                "subscribe",
                json!({"thread_id":thread_id}),
            )));
        }
        let (release, gate) = oneshot::channel();
        let mut working = ev_assistant_message(phase, "Waiting for the worker's reply.");
        working["item"]["phase"] = json!("commentary");
        streams.push(vec![
            StreamingSseChunk {
                gate: None,
                body: sse(vec![ev_response_created(phase), working]),
            },
            StreamingSseChunk {
                gate: Some(gate),
                // TurnComplete precedes parent notification. Drain the worker's final mail
                // before advancing phases, and force a follow-up so silence is observable.
                body: tool(&format!("checkpoint-{phase}"), "wait_agent", json!({})),
            },
        ]);
        streams.push(response(tool(
            &format!("reply-{phase}"),
            "post",
            json!({"thread_id":thread_id,"text":format!("Reply while {phase}.")}),
        )));
        streams.push(response(done()));
        streams.push(response(done()));
        phases.push((phase, subscribed, release));
    }
    let (streaming, _) = start_streaming_sse_server(streams).await;
    let base_url = format!("{}/v1", streaming.uri());
    let resumed = test_codex()
        .with_config(move |config| {
            configure(config);
            config.model_provider.base_url = Some(base_url);
            config
                .features
                .disable(Feature::EnableRequestCompression)
                .expect("read raw request bodies from the gated mock");
        })
        .restart(&server, &root)
        .await?;
    resumed
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Spawn a worker and finish your turn.".into(),
            text_elements: vec![],
        }]))
        .await?;
    let child_id = wait_for_event_match(&resumed.codex, |event| match event {
        EventMsg::ItemCompleted(event) => match &event.item {
            TurnItem::SubAgentActivity(activity) if activity.id == "spawn-worker" => {
                Some(activity.agent_thread_id)
            }
            _ => None,
        },
        _ => None,
    })
    .await;
    release_wait.send(()).expect("release wait after spawn");
    wait_for_event(&resumed.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let child = resumed.thread_manager.get_thread(child_id).await?;
    wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    assert!(
        streaming
            .requests()
            .await
            .iter()
            .any(|body| { String::from_utf8_lossy(body).contains("Message Type: FINAL_ANSWER") })
    );

    for (phase_index, (phase, subscribed, release)) in phases.into_iter().enumerate() {
        resumed
            .codex
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: format!("Observe the discussion while {phase}."),
                text_elements: vec![],
            }]))
            .await?;
        let turn_id = wait_for_event_match(&resumed.codex, |event| match event {
            EventMsg::TurnStarted(event) => Some(event.turn_id.clone()),
            _ => None,
        })
        .await;
        // The gate keeps the recipient active until the child's post and fanout finish.
        wait_for_event(&resumed.codex, |event| {
            matches!(event, EventMsg::ItemCompleted(event)
                if matches!(&event.item, TurnItem::AgentMessage(message) if message.id == phase))
        })
        .await;
        child
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: format!("Reply to the discussion while {phase}."),
                text_elements: vec![],
            }]))
            .await?;
        wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
        let requests = streaming
            .requests()
            .await
            .iter()
            .map(|body| serde_json::from_slice::<Value>(body))
            .collect::<serde_json::Result<Vec<_>>>()?;
        let reply_call = format!("reply-{phase}");
        let output = requests
            .iter()
            .filter_map(|request| request["input"].as_array())
            .flatten()
            .find(|item| item["type"] == "function_call_output" && item["call_id"] == reply_call)
            .context("child post output")?;
        let reply: PostMetadata =
            serde_json::from_str(output["output"].as_str().context("child post result")?)?;
        assert_eq!(reply.author.as_str(), "/root/worker");
        release.send(()).expect("release active recipient");
        wait_for_event(&resumed.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;

        let requests = streaming
            .requests()
            .await
            .iter()
            .map(|body| serde_json::from_slice::<Value>(body))
            .collect::<serde_json::Result<Vec<_>>>()?;
        // A delivered notice can start the recipient's next request before the child finishes.
        let request = requests
            .iter()
            .rfind(|request| request["client_metadata"]["turn_id"] == turn_id)
            .context("recipient request")?;
        let input = request["input"].as_array().context("recipient input")?;
        assert_eq!(
            input
                .iter()
                .filter(|item| {
                    item["type"] == "agent_message"
                        && item["author"] == "/root/worker"
                        && item["content"]
                            .to_string()
                            .contains("Message Type: FINAL_ANSWER")
                })
                .count(),
            phase_index + 2,
            "worker completion must be drained before leaving phase: {phase}"
        );
        let notices = input
            .iter()
            .filter(|item| {
                item["type"] == "agent_message"
                    && item["content"]
                        .to_string()
                        .contains("Message Type: CHANNEL_POST")
            })
            .map(|item| {
                (
                    item["author"].clone(),
                    item["recipient"].clone(),
                    item["content"].clone(),
                )
            })
            .collect::<Vec<_>>();
        let expected = if subscribed {
            vec![(
                json!("/root/worker"),
                json!("/root"),
                json!([{"type":"input_text","text":format!(
                    "Message Type: CHANNEL_POST\nSender: /root/worker\nChannel: design\nMessage ID: {}\nThread ID: {}\nPayload:\nReply while {phase}.",
                    reply.message_id, reply.thread_id,
                )}]),
            )]
        } else {
            Vec::new()
        };
        assert_eq!(notices, expected, "phase: {phase}");
    }
    child.shutdown_and_wait().await?;
    resumed.codex.shutdown_and_wait().await?;
    streaming.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn board_clock_failure_does_not_fall_back_or_create_a_channel() -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let mock = responses::mount_sse_sequence(
        &server,
        vec![
            tool(
                "failed-post",
                "post",
                json!({"new_channel_name":"design","text":"Not accepted."}),
            ),
            tool("channels-after-failure", "get_channels", json!({})),
            done(),
        ],
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            configure(config);
            config.include_environment_context = false;
            config.current_time_reminder = Some(codex_core::config::CurrentTimeReminderConfig {
                clock_source: codex_features::CurrentTimeSource::External,
                ..Default::default()
            });
        })
        .with_external_time_provider(std::sync::Arc::new(BoardClock::Unavailable))
        .build_with_auto_env(&server)
        .await?;
    test.submit_turn("Post the decision and list channels.")
        .await?;
    let requests = mock.requests();
    assert!(
        requests[1]
            .function_call_output_text("failed-post")
            .expect("clock error")
            .contains("board clock unavailable")
    );
    let result: Value = serde_json::from_str(
        &requests[2]
            .function_call_output_text("channels-after-failure")
            .expect("channels result"),
    )?;
    assert_eq!(
        result,
        json!({"results":[],"n_returned":0,"has_more":false,"next_cursor":null})
    );
    Ok(())
}
