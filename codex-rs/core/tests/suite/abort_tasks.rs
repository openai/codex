use std::sync::Arc;
use std::time::Duration;

use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use regex_lite::Regex;
use serde_json::json;
use tokio::time::Instant;
use tokio::time::sleep;

async fn wait_for_responses_request_count_to_stabilize(
    response_mock: &ResponseMock,
    expected_count: usize,
    timeout: Duration,
    settle_duration: Duration,
) -> Vec<ResponsesRequest> {
    let deadline = Instant::now() + timeout;
    let mut stable_since: Option<Instant> = None;

    loop {
        let requests = response_mock.requests();
        let request_count = requests.len();

        if request_count > expected_count {
            panic!("expected at most {expected_count} responses requests, got {request_count}");
        }

        if request_count == expected_count {
            match stable_since {
                Some(stable_since) if stable_since.elapsed() >= settle_duration => {
                    return requests;
                }
                Some(_) => {}
                None => stable_since = Some(Instant::now()),
            }
        } else {
            stable_since = None;
        }

        assert!(
            Instant::now() < deadline,
            "timed out waiting for {expected_count} responses requests; last count was {request_count}"
        );
        sleep(Duration::from_millis(10)).await;
    }
}

/// Integration test: spawn a long‑running shell_command tool via a mocked Responses SSE
/// function call, then interrupt the session and expect TurnAborted.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_long_running_tool_emits_turn_aborted() {
    let command = "sleep 60";

    let args = json!({
        "command": command,
        "timeout_ms": 60_000
    })
    .to_string();
    let body = sse(vec![
        ev_function_call("call_sleep", "shell_command", &args),
        ev_completed("done"),
    ]);

    let server = start_mock_server().await;
    mount_sse_once(&server, body).await;

    let codex = test_codex()
        .with_model("gpt-5.1")
        .build(&server)
        .await
        .unwrap()
        .codex;

    // Kick off a turn that triggers the function call.
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "start sleep".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    // Wait until the exec begins to avoid a race, then interrupt.
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandBegin(_))).await;

    codex.submit(Op::Interrupt).await.unwrap();

    // Expect TurnAborted soon after.
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnAborted(_))).await;

    codex.submit(Op::Shutdown).await.unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;
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
        "command": command,
        "timeout_ms": 60_000
    })
    .to_string();
    let first_body = sse(vec![
        ev_response_created("resp-history"),
        ev_function_call(call_id, "shell_command", &args),
        ev_completed("resp-history"),
    ]);
    let follow_up_body = sse(vec![
        ev_response_created("resp-followup"),
        ev_completed("resp-followup"),
    ]);

    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(&server, vec![first_body, follow_up_body]).await;

    let fixture = test_codex()
        .with_model("gpt-5.1")
        .build(&server)
        .await
        .unwrap();
    let codex = Arc::clone(&fixture.codex);

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "start history recording".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandBegin(_))).await;

    tokio::time::sleep(Duration::from_secs_f32(0.1)).await;
    codex.submit(Op::Interrupt).await.unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnAborted(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "follow up".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = wait_for_responses_request_count_to_stabilize(
        &response_mock,
        2,
        Duration::from_secs(2),
        Duration::from_millis(100),
    )
    .await;
    assert!(
        requests.len() == 2,
        "expected two calls to the responses API, got {}",
        requests.len()
    );

    assert!(
        response_mock.saw_function_call(call_id),
        "function call not recorded in responses payload"
    );
    let follow_up_request = &requests[1];
    let (output, success) = follow_up_request
        .function_call_output_content_and_success(call_id)
        .expect("missing function_call_output entry");
    let output = output.expect("missing function_call_output text");
    assert!(
        output.contains("aborted"),
        "expected aborted output in follow-up request, got {output:?}"
    );
    assert!(
        success != Some(true),
        "expected aborted output to avoid a success=true marker, got {success:?} with output {output:?}"
    );

    let wall_time =
        Regex::new(r"Wall time: ([0-9]+(?:\.[0-9]+)?) seconds").expect("compile wall time regex");
    let aborted_after =
        Regex::new(r"aborted by user after ([0-9]+(?:\.[0-9]+)?)s").expect("compile abort regex");

    let secs = wall_time
        .captures(&output)
        .and_then(|captures| captures.get(1))
        .or_else(|| {
            aborted_after
                .captures(&output)
                .and_then(|captures| captures.get(1))
        })
        .map(|capture| capture.as_str().parse::<f32>().unwrap());

    if let Some(secs) = secs {
        assert!(
            secs >= 0.1,
            "expected at least one tenth of a second of elapsed time, got {secs} in {output:?}"
        );
    }

    codex.submit(Op::Shutdown).await.unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;
}

/// After an interrupt we persist a model-visible `<turn_aborted>` marker in the conversation
/// history. This test asserts that the marker is included in the next `/responses` request.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_persists_turn_aborted_marker_in_next_request() {
    let command = "sleep 60";
    let call_id = "call-turn-aborted-marker";

    let args = json!({
        "command": command,
        "timeout_ms": 60_000
    })
    .to_string();
    let first_body = sse(vec![
        ev_response_created("resp-marker"),
        ev_function_call(call_id, "shell_command", &args),
        ev_completed("resp-marker"),
    ]);
    let follow_up_body = sse(vec![
        ev_response_created("resp-followup"),
        ev_completed("resp-followup"),
    ]);

    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(&server, vec![first_body, follow_up_body]).await;

    let fixture = test_codex()
        .with_model("gpt-5.1")
        .build(&server)
        .await
        .unwrap();
    let codex = Arc::clone(&fixture.codex);

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "start interrupt marker".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandBegin(_))).await;

    tokio::time::sleep(Duration::from_secs_f32(0.1)).await;
    codex.submit(Op::Interrupt).await.unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnAborted(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "follow up".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = wait_for_responses_request_count_to_stabilize(
        &response_mock,
        2,
        Duration::from_secs(2),
        Duration::from_millis(100),
    )
    .await;
    assert_eq!(requests.len(), 2, "expected two calls to the responses API");

    let follow_up_request = &requests[1];
    let user_texts = follow_up_request.message_input_texts("user");
    assert!(
        user_texts
            .iter()
            .any(|text| text.contains("<turn_aborted>")),
        "expected <turn_aborted> marker in follow-up request"
    );

    codex.submit(Op::Shutdown).await.unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;
}

/// Interrupting a turn while a tool-produced follow-up is pending must not
/// start another model request before the session reports TurnAborted.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_does_not_issue_follow_up_request() {
    let command = "sleep 60";
    let call_id = "call-no-follow-up";

    let args = json!({
        "command": command,
        "timeout_ms": 60_000
    })
    .to_string();
    let first_body = sse(vec![
        ev_response_created("resp-no-follow-up"),
        ev_function_call(call_id, "shell_command", &args),
        ev_completed("resp-no-follow-up"),
    ]);

    let server = start_mock_server().await;
    let response_mock = mount_sse_once(&server, first_body).await;

    let fixture = test_codex()
        .with_model("gpt-5.1")
        .build(&server)
        .await
        .unwrap();
    let codex = Arc::clone(&fixture.codex);

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "start interrupt follow-up guard".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandBegin(_))).await;

    tokio::time::sleep(Duration::from_secs_f32(0.1)).await;
    codex.submit(Op::Interrupt).await.unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnAborted(_))).await;

    let requests = wait_for_responses_request_count_to_stabilize(
        &response_mock,
        1,
        Duration::from_secs(2),
        Duration::from_millis(200),
    )
    .await;
    assert_eq!(
        requests.len(),
        1,
        "interrupt should not issue a follow-up responses request"
    );

    codex.submit(Op::Shutdown).await.unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;
}
