use assert_matches::assert_matches;
use codex_core::SuspendTurnOutcome;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_history::RolloutItem;
use std::sync::Arc;
use std::time::Duration;

use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_response_once;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use regex_lite::Regex;
use serde_json::json;
use test_case::test_case;

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

/// Suspending a root preserves its unfinished turn and initiates child shutdown. The host
/// separately waits for tree cleanup, including any child writer failure, before recovery.
#[test_case(false; "recovers root after stopping child")]
#[test_case(true; "reports child writer failure separately")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn root_turn_suspension_preserves_unfinished_turn_history(fail_child_writer: bool) {
    let server = start_mock_server().await;
    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("spawn_response"),
            ev_function_call_with_namespace(
                "spawn_worker",
                "collaboration",
                "spawn_agent",
                &json!({
                    "message": "keep working until shutdown",
                    "task_name": "worker",
                    "fork_turns": "none",
                })
                .to_string(),
            ),
            ev_completed("spawn_response"),
        ]),
    )
    .await;
    // Waiting on the mocked response keeps the turn active on local and remote executors
    // without requiring an OS-specific command or a working sandboxed child process.
    for response_id in ["pending_child", "pending_root"] {
        mount_response_once(
            &server,
            sse_response(sse(vec![
                ev_response_created(response_id),
                ev_completed(response_id),
            ]))
            .set_delay(Duration::from_secs(60)),
        )
        .await;
    }
    let test = test_codex()
        .with_model("gpt-5.6-sol")
        .with_config(|config| {
            config
                .features
                .enable(Feature::Collab)
                .expect("enable collaboration for the suspension test");
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("enable multi-agent v2 for the suspension test");
        })
        .build_with_auto_env(&server)
        .await
        .expect("start persistent root thread");
    let codex = Arc::clone(&test.codex);
    let mut created_threads = test.thread_manager.subscribe_thread_created();
    let submitted = codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "preserve this exact unfinished turn".into(),
            text_elements: Vec::new(),
        }]))
        .await
        .expect("start root turn");
    let codex_core::TurnInputSubmission::Started { turn_id, .. } = submitted else {
        panic!("expected a started root turn");
    };
    let child_id = tokio::time::timeout(Duration::from_secs(10), created_threads.recv())
        .await
        .expect("child creation notification must arrive")
        .expect("spawned child");
    let descendant = test
        .thread_manager
        .get_thread(child_id)
        .await
        .expect("loaded child");
    wait_for_event(&descendant, |event| {
        matches!(event, EventMsg::TurnStarted(_))
    })
    .await;

    if fail_child_writer {
        test.thread_store
            .shutdown_thread(child_id)
            .await
            .expect("close the child writer before shutdown to inject a persistence failure");
    }
    let suspension =
        tokio::time::timeout(Duration::from_secs(10), codex.suspend_turn_and_shutdown())
            .await
            .expect("suspension must not wait for the child turn to finish naturally");
    assert_eq!(
        suspension.expect("suspend the root independently of child cleanup"),
        SuspendTurnOutcome::Suspended {
            turn_id: turn_id.clone(),
        },
    );
    // Observe child termination before requesting the completion handle, so the test proves
    // that root suspension itself initiated child shutdown.
    tokio::time::timeout(Duration::from_secs(10), descendant.wait_until_terminated())
        .await
        .expect("root suspension must initiate child shutdown");
    let shutdown = test
        .thread_manager
        .request_agent_tree_shutdown(test.session_configured.thread_id)
        .await
        .expect("get the tree cleanup completion handle");
    let shutdown_result = tokio::time::timeout(Duration::from_secs(10), shutdown.wait_detailed())
        .await
        .expect("tree cleanup must finish");
    if fail_child_writer {
        let report =
            shutdown_result.expect_err("tree cleanup must report the child writer failure");
        assert!(
            report
                .failures
                .iter()
                .any(|failure| failure.thread_id == Some(child_id))
        );
        return;
    }
    shutdown_result.expect("finish tree cleanup before handing off the root");
    let child_rollout =
        tokio::fs::read_to_string(descendant.rollout_path().expect("child rollout"))
            .await
            .expect("read closed child writer");
    assert!(child_rollout.lines().any(|line| matches!(
        codex_rollout::parse_rollout_line(line)
            .expect("parse child rollout")
            .item,
        RolloutItem::EventMsg(EventMsg::TurnAborted(_))
    )));
    let rollout_path = codex.rollout_path().expect("rollout path");
    let rollout = tokio::fs::read_to_string(&rollout_path)
        .await
        .expect("read durable rollout");
    let items = rollout
        .lines()
        .map(|line| {
            codex_rollout::parse_rollout_line(line)
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
                prompt_cache_ttl: None,
                cyber_access_program: None,
            })
            .await
            .expect("recover the unfinished turn"),
        codex_core::StartIfIdleSubmission::Started {
            root_turn_id: turn_id.clone(),
            turn_id: turn_id.clone()
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
    assert_eq!(
        resumed.thread_manager.list_thread_ids().await,
        vec![resumed.session_configured.thread_id],
        "root recovery must not automatically restart the interrupted child"
    );
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
