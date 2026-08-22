use assert_matches::assert_matches;
use codex_core::StartThreadOptions;
use codex_core::SuspendTurnOutcome;
use codex_core::TurnInputRequest;
use codex_history::RolloutItem;
use codex_history::RolloutLine;
use std::sync::Arc;
use std::time::Duration;

use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_response_once;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use regex_lite::Regex;
use serde_json::json;

/// Integration test: spawn a long‑running exec_command tool via a mocked Responses SSE
/// function call, then interrupt the session and expect TurnAborted.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_long_running_tool_emits_turn_aborted() {
    let command = "sleep 60";

    let args = json!({
        "cmd": command,
        "yield_time_ms": 60_000
    })
    .to_string();
    let body = sse(vec![
        ev_function_call("call_sleep", "exec_command", &args),
        ev_completed("done"),
    ]);

    let server = start_mock_server().await;
    mount_sse_once(&server, body).await;

    let fixture = test_codex()
        .with_model("gpt-5.4")
        .build(&server)
        .await
        .unwrap();
    let codex = Arc::clone(&fixture.codex);

    // Kick off a turn that triggers the function call.
    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "start sleep".into(),
            text_elements: Vec::new(),
        }]))
        .await
        .unwrap();

    // Wait until the exec begins to avoid a race, then interrupt.
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandBegin(_))).await;

    codex.submit(Op::Interrupt).await.unwrap();

    // Expect TurnAborted soon after.
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnAborted(_))).await;
    codex.submit(Op::CleanBackgroundTerminals).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn root_turn_suspension_preserves_unfinished_turn_history() {
    let server = start_mock_server().await;
    // Waiting on the mocked response keeps the turn active on local and remote executors
    // without requiring an OS-specific command or a working sandboxed child process.
    mount_response_once(
        &server,
        sse_response(sse(vec![
            ev_response_created("suspended_response"),
            ev_completed("suspended_response"),
        ]))
        .set_delay(Duration::from_secs(60)),
    )
    .await;
    let test = test_codex()
        .with_model("gpt-5.4")
        .build_with_auto_env(&server)
        .await
        .expect("start persistent root thread");
    let codex = Arc::clone(&test.codex);
    let descendant = test
        .thread_manager
        .start_thread(StartThreadOptions {
            session_source: Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id: test.session_configured.thread_id,
                depth: 1,
                agent_path: None,
                agent_nickname: None,
                agent_role: None,
            })),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await
        .expect("start a currently loaded descendant");
    let submitted = codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "preserve this exact unfinished turn".into(),
            text_elements: Vec::new(),
        }]))
        .await
        .expect("start root turn");
    let codex_core::TurnInputSubmission::Started { turn_id } = submitted else {
        panic!("expected a started root turn");
    };
    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnStarted(_))).await;

    assert_eq!(
        codex
            .suspend_turn_and_shutdown()
            .await
            .expect("reject handoff while a descendant remains loaded"),
        SuspendTurnOutcome::HasLiveDescendants,
    );
    descendant
        .thread
        .shutdown_and_wait()
        .await
        .expect("stop the descendant before root handoff");
    test.thread_manager
        .remove_thread(&descendant.thread_id)
        .await
        .expect("remove the stopped descendant from the live thread inventory");

    // A previously admitted descendant no longer blocks handoff once it is stopped
    // and removed; suspension only consults the current live subtree.
    assert_eq!(
        codex
            .suspend_turn_and_shutdown()
            .await
            .expect("stop and close the old writer"),
        SuspendTurnOutcome::Suspended {
            turn_id: turn_id.clone(),
        },
    );
    let rollout_path = codex.rollout_path().expect("rollout path");
    let rollout = tokio::fs::read_to_string(&rollout_path)
        .await
        .expect("read durable rollout");
    let items = rollout
        .lines()
        .map(|line| {
            serde_json::from_str::<RolloutLine>(line)
                .expect("parse durable rollout")
                .item
        })
        .collect::<Vec<_>>();
    assert!(items.iter().all(|item| !matches!(
        item,
        RolloutItem::EventMsg(EventMsg::TurnAborted(_) | EventMsg::TurnComplete(_))
    )));
    test.thread_manager
        .remove_thread(&test.session_configured.thread_id)
        .await
        .expect("unload the suspended root");
    let recovery_server = start_mock_server().await;
    mount_sse_once(
        &recovery_server,
        sse(vec![
            ev_response_created("recovered_response"),
            ev_completed("recovered_response"),
        ]),
    )
    .await;
    let resumed = test_codex()
        .with_model("gpt-5.4")
        .resume(&recovery_server, Arc::clone(&test.home), rollout_path)
        .await
        .expect("resume the suspended root on a replacement runtime");

    assert_eq!(
        resumed
            .codex
            .recover_turn_if_idle(codex_core::RecoverTurnRequest {
                turn_id: turn_id.clone(),
                thread_settings: Default::default(),
                trace: None,
            })
            .await
            .expect("recover the unfinished turn"),
        codex_core::StartIfIdleSubmission::Started {
            turn_id: turn_id.clone(),
        },
    );
    let completed = wait_for_event(&resumed.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let EventMsg::TurnComplete(completed) = completed else {
        unreachable!("wait_for_event returned unexpected event");
    };
    assert_eq!(completed.turn_id, turn_id);
}

/// After an interrupt we expect the next request to the model to include both
/// the original tool call and an `"aborted"` `function_call_output`. This test
/// exercises the follow-up flow: it sends another user turn, inspects the mock
/// responses server, and ensures the model receives the synthesized abort.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_tool_records_history_entries() {
    let command = "sleep 60";
    let call_id = "call-history";

    let args = json!({
        "cmd": command,
        "yield_time_ms": 60_000
    })
    .to_string();
    let first_body = sse(vec![
        ev_response_created("resp-history"),
        ev_function_call(call_id, "exec_command", &args),
        ev_completed("resp-history"),
    ]);
    let follow_up_body = sse(vec![
        ev_response_created("resp-followup"),
        ev_completed("resp-followup"),
    ]);

    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(&server, vec![first_body, follow_up_body]).await;

    let fixture = test_codex()
        .with_model("gpt-5.4")
        .build(&server)
        .await
        .unwrap();
    let codex = Arc::clone(&fixture.codex);

    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "start history recording".into(),
            text_elements: Vec::new(),
        }]))
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandBegin(_))).await;

    tokio::time::sleep(Duration::from_secs_f32(0.1)).await;
    codex.submit(Op::Interrupt).await.unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnAborted(_))).await;

    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "follow up".into(),
            text_elements: Vec::new(),
        }]))
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = response_mock.requests();
    assert!(
        requests.len() == 2,
        "expected two calls to the responses API, got {}",
        requests.len()
    );

    assert!(
        response_mock.saw_function_call(call_id),
        "function call not recorded in responses payload"
    );
    let output = response_mock
        .function_call_output_text(call_id)
        .expect("missing function_call_output text");
    let re = Regex::new(r"^Wall time: ([0-9]+(?:\.[0-9])?) seconds\naborted by user$")
        .expect("compile regex");
    let captures = re.captures(&output);
    assert_matches!(
        captures.as_ref(),
        Some(caps) if caps.get(1).is_some(),
        "aborted message with elapsed seconds"
    );
    let secs: f32 = captures
        .expect("aborted message with elapsed seconds")
        .get(1)
        .unwrap()
        .as_str()
        .parse()
        .unwrap();
    assert!(
        secs >= 0.1,
        "expected at least one tenth of a second of elapsed time, got {secs}"
    );
    codex.submit(Op::CleanBackgroundTerminals).await.unwrap();
}

/// After an interrupt we persist a model-visible `<turn_aborted>` marker in the conversation
/// history. This test asserts that the marker is included in the next `/responses` request.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_persists_turn_aborted_marker_in_next_request() {
    let command = "sleep 60";
    let call_id = "call-turn-aborted-marker";

    let args = json!({
        "cmd": command,
        "yield_time_ms": 60_000
    })
    .to_string();
    let first_body = sse(vec![
        ev_response_created("resp-marker"),
        ev_function_call(call_id, "exec_command", &args),
        ev_completed("resp-marker"),
    ]);
    let follow_up_body = sse(vec![
        ev_response_created("resp-followup"),
        ev_completed("resp-followup"),
    ]);

    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(&server, vec![first_body, follow_up_body]).await;

    let fixture = test_codex()
        .with_model("gpt-5.4")
        .build(&server)
        .await
        .unwrap();
    let codex = Arc::clone(&fixture.codex);

    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "start interrupt marker".into(),
            text_elements: Vec::new(),
        }]))
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandBegin(_))).await;

    tokio::time::sleep(Duration::from_secs_f32(0.1)).await;
    codex.submit(Op::Interrupt).await.unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnAborted(_))).await;

    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "follow up".into(),
            text_elements: Vec::new(),
        }]))
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2, "expected two calls to the responses API");

    let follow_up_request = &requests[1];
    let user_texts = follow_up_request.message_input_texts("user");
    assert!(
        user_texts
            .iter()
            .any(|text| text.contains("<turn_aborted>")),
        "expected <turn_aborted> marker in follow-up request"
    );
    codex.submit(Op::CleanBackgroundTerminals).await.unwrap();
}
