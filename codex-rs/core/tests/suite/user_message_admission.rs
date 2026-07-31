use codex_core::UserMessageAdmission;
use codex_core::config::Constrained;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Barrier;
use tokio::sync::oneshot;
use tokio::time::timeout;

fn user_input(text: &str) -> Op {
    Op::UserInput {
        items: vec![UserInput::Text {
            text: text.to_string(),
            text_elements: Vec::new(),
        }],
        final_output_json_schema: None,
        responsesapi_client_metadata: None,
        additional_context: Default::default(),
        thread_settings: Default::default(),
    }
}

/// Concurrent submissions must start exactly one turn, steer the other message, and persist both client IDs.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_message_admission_reports_started_and_steered_for_concurrent_submissions() {
    let (release_response, response_gate) = oneshot::channel();
    let (server, _completions) = start_streaming_sse_server(vec![
        vec![
            StreamingSseChunk {
                gate: None,
                body: responses::sse(vec![ev_response_created("resp-1")]),
            },
            StreamingSseChunk {
                gate: Some(response_gate),
                body: responses::sse(vec![ev_completed("resp-1")]),
            },
        ],
        vec![StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        }],
    ])
    .await;
    let test = test_codex()
        .with_model("gpt-5.4")
        .with_history_mode(ThreadHistoryMode::Paginated)
        .build_with_streaming_server(&server)
        .await
        .expect("build user-message admission session");
    let codex = Arc::clone(&test.codex);
    let barrier = Arc::new(Barrier::new(3));

    let first_submission = tokio::spawn({
        let codex = Arc::clone(&codex);
        let barrier = Arc::clone(&barrier);
        async move {
            barrier.wait().await;
            codex
                .submit_user_input_and_wait_for_admission(
                    user_input("first message"),
                    /*trace*/ None,
                    Some("client-message-1".to_string()),
                )
                .await
        }
    });
    let second_submission = tokio::spawn({
        let codex = Arc::clone(&codex);
        let barrier = Arc::clone(&barrier);
        async move {
            barrier.wait().await;
            codex
                .submit_user_input_and_wait_for_admission(
                    user_input("second message"),
                    /*trace*/ None,
                    Some("client-message-2".to_string()),
                )
                .await
        }
    });
    barrier.wait().await;

    let (first_admission, second_admission) = timeout(Duration::from_secs(5), async {
        tokio::join!(first_submission, second_submission)
    })
    .await
    .expect("both concurrent admissions should resolve before sampling finishes");
    let first_admission = first_admission
        .expect("first submission task should finish")
        .expect("first user message should be admitted");
    let second_admission = second_admission
        .expect("second submission task should finish")
        .expect("second user message should be admitted");
    let (started_turn_id, steered_turn_id, started_message) =
        match (&first_admission, &second_admission) {
            (
                UserMessageAdmission::Started { turn_id: started },
                UserMessageAdmission::Steered { turn_id: steered },
            ) => (started, steered, "first message"),
            (
                UserMessageAdmission::Steered { turn_id: steered },
                UserMessageAdmission::Started { turn_id: started },
            ) => (started, steered, "second message"),
            _ => panic!(
                "concurrent messages must start exactly one turn and steer the other: \
             {first_admission:?}, {second_admission:?}"
            ),
        };
    assert_eq!(started_turn_id, steered_turn_id);

    release_response
        .send(())
        .expect("response gate should remain open");
    let mut observed_client_message_ids = Vec::new();
    loop {
        let event = timeout(Duration::from_secs(10), codex.next_event())
            .await
            .expect("admitted user-message events should arrive promptly")
            .expect("admitted user-message event stream should remain available");
        match event.msg {
            EventMsg::ItemStarted(item) => {
                if let TurnItem::UserMessage(message) = item.item {
                    observed_client_message_ids.push(message.client_id);
                }
            }
            EventMsg::Error(error) => panic!("admitted user message failed: {error:?}"),
            EventMsg::StreamError(error) => panic!("admitted response stream failed: {error:?}"),
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }
    observed_client_message_ids.sort();
    assert_eq!(
        observed_client_message_ids,
        vec![
            Some("client-message-1".to_string()),
            Some("client-message-2".to_string()),
        ]
    );

    let requests = server.requests().await;
    assert_eq!(requests.len(), 2);
    let request_bodies: Vec<Value> = requests
        .iter()
        .map(|request| serde_json::from_slice(request).expect("parse model request"))
        .collect();
    assert!(request_bodies[0].to_string().contains(started_message));
    assert!(request_bodies[1].to_string().contains("first message"));
    assert!(request_bodies[1].to_string().contains("second message"));

    codex
        .flush_rollout()
        .await
        .expect("flush persisted user-message history");
    let rollout_path = codex.rollout_path().expect("user-message rollout path");
    let rollout = tokio::fs::read_to_string(rollout_path)
        .await
        .expect("read persisted user-message rollout");
    let mut persisted_client_message_ids: Vec<_> = rollout
        .lines()
        .map(serde_json::from_str::<RolloutLine>)
        .collect::<Result<Vec<_>, _>>()
        .expect("parse persisted user-message rollout")
        .into_iter()
        .filter_map(|line| match line.item {
            RolloutItem::EventMsg(EventMsg::ItemCompleted(event)) => match event.item {
                TurnItem::UserMessage(message) => message.client_id,
                _ => None,
            },
            _ => None,
        })
        .collect();
    persisted_client_message_ids.sort();
    assert_eq!(
        persisted_client_message_ids,
        vec![
            "client-message-1".to_string(),
            "client-message-2".to_string()
        ]
    );
    server.shutdown().await;
}

/// Handler-side settings rejection must acknowledge failure once and leave the session usable.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_message_admission_reports_invalid_settings_and_recovers() {
    let (server, _completions) = start_streaming_sse_server(vec![vec![StreamingSseChunk {
        gate: None,
        body: responses::sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    }]])
    .await;
    let test = test_codex()
        .with_model("gpt-5.4")
        .with_config(|config| {
            config.permissions.approval_policy = Constrained::allow_only(AskForApproval::OnRequest);
        })
        .build_with_streaming_server(&server)
        .await
        .expect("build approval-constrained user-message admission session");
    let codex = &test.codex;
    let mut invalid_input = user_input("invalid approval policy");
    let Op::UserInput {
        thread_settings, ..
    } = &mut invalid_input
    else {
        unreachable!("test helper always produces a user-input operation");
    };
    thread_settings.approval_policy = Some(AskForApproval::Never);

    let error = timeout(
        Duration::from_secs(5),
        codex.submit_user_input_and_wait_for_admission(
            invalid_input,
            /*trace*/ None,
            Some("invalid-client-message".to_string()),
        ),
    )
    .await
    .expect("handler-side rejection should resolve promptly")
    .expect_err("disallowed approval settings should not be admitted");
    assert!(matches!(
        error.details(),
        CodexErrorDetails::InvalidRequest(_)
    ));

    let error_event = loop {
        let event = timeout(Duration::from_secs(5), codex.next_event())
            .await
            .expect("invalid settings should emit an error event promptly")
            .expect("event stream should remain available");
        match event.msg {
            EventMsg::Error(error) => break error,
            EventMsg::TurnComplete(_) => {
                panic!("invalid settings completed a turn without emitting an error")
            }
            EventMsg::StreamError(error) => {
                panic!("invalid settings unexpectedly started a model stream: {error:?}")
            }
            _ => {}
        }
    };
    assert_eq!(
        error_event.codex_error_info,
        Some(CodexErrorInfo::BadRequest)
    );

    let recovered = timeout(
        Duration::from_secs(5),
        codex.submit_user_input_and_wait_for_admission(
            user_input("valid message after rejected settings"),
            /*trace*/ None,
            Some("valid-client-message".to_string()),
        ),
    )
    .await
    .expect("session should accept a valid message after rejecting invalid settings")
    .expect("valid message should start a turn");
    assert!(matches!(recovered, UserMessageAdmission::Started { .. }));

    loop {
        let event = timeout(Duration::from_secs(10), codex.next_event())
            .await
            .expect("recovered turn should finish promptly")
            .expect("event stream should remain available after rejected settings");
        match event.msg {
            EventMsg::Error(error) => panic!("settings rejection emitted another error: {error:?}"),
            EventMsg::StreamError(error) => panic!("recovered turn stream failed: {error:?}"),
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }
    assert_eq!(server.requests().await.len(), 1);
    server.shutdown().await;
}

/// Non-user operations must fail validation immediately instead of waiting for a user-message acknowledgement.
#[tokio::test]
async fn user_message_admission_rejects_non_user_operations_without_waiting() {
    let (server, _completions) = start_streaming_sse_server(Vec::new()).await;
    let test = test_codex()
        .with_model("gpt-5.4")
        .build_with_streaming_server(&server)
        .await
        .expect("build user-message admission session");
    let codex = &test.codex;

    let error = timeout(
        Duration::from_secs(5),
        codex.submit_user_input_and_wait_for_admission(
            Op::Interrupt,
            /*trace*/ None,
            Some("client-message-1".to_string()),
        ),
    )
    .await
    .expect("non-user operation should be rejected promptly")
    .expect_err("non-user operation should not use message admission");
    assert!(matches!(
        error.details(),
        CodexErrorDetails::InvalidRequest(_)
    ));
    server.shutdown().await;
}

/// A terminated session must reject admission immediately instead of leaving a retained waiter unresolved.
#[tokio::test]
async fn user_message_admission_fails_promptly_after_session_shutdown() {
    let (server, _completions) = start_streaming_sse_server(Vec::new()).await;
    let test = test_codex()
        .with_model("gpt-5.4")
        .build_with_streaming_server(&server)
        .await
        .expect("build user-message admission session");
    let codex = &test.codex;
    codex
        .shutdown_and_wait()
        .await
        .expect("test session should shut down");

    let error = timeout(
        Duration::from_secs(5),
        codex.submit_user_input_and_wait_for_admission(
            user_input("after shutdown"),
            /*trace*/ None,
            Some("client-message-1".to_string()),
        ),
    )
    .await
    .expect("closed session should reject admission promptly")
    .expect_err("closed session should not accept user messages");
    assert!(matches!(
        error.details(),
        CodexErrorDetails::InternalAgentDied
    ));
    server.shutdown().await;
}
