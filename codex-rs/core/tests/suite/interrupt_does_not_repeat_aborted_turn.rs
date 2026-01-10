#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use wiremock::Match;
use wiremock::Mock;
use wiremock::Request;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path_regex;

#[derive(Clone, Default)]
struct RequestsCapture(Arc<Mutex<Vec<Request>>>);

impl RequestsCapture {
    fn requests(&self) -> Vec<Request> {
        self.0.lock().unwrap().clone()
    }
}

impl Match for RequestsCapture {
    fn matches(&self, request: &Request) -> bool {
        self.0.lock().unwrap().push(request.clone());
        true
    }
}

#[derive(Debug, Default)]
struct ResponderState {
    call_count: usize,
}

#[derive(Clone)]
struct RepeatUnlessTurnAbortedResponder {
    state: Arc<Mutex<ResponderState>>,
    side_effect_path: PathBuf,
}

impl RepeatUnlessTurnAbortedResponder {
    fn request_contains_turn_aborted_marker(request: &Request) -> bool {
        let Ok(body) = request.body_json::<Value>() else {
            return false;
        };
        let Some(input) = body.get("input").and_then(Value::as_array) else {
            return false;
        };
        input.iter().any(|item| {
            item.get("type").and_then(Value::as_str) == Some("message")
                && item.get("role").and_then(Value::as_str) == Some("user")
                && item
                    .get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter(|span| span.get("type").and_then(Value::as_str) == Some("input_text"))
                    .any(|span| {
                        span.get("text")
                            .and_then(Value::as_str)
                            .is_some_and(|text| text.contains("<turn_aborted>"))
                    })
        })
    }

    fn sse_response(body: String) -> ResponseTemplate {
        ResponseTemplate::new(200)
            .insert_header("content-type", "text/event-stream")
            .set_body_string(body)
    }
}

impl Respond for RepeatUnlessTurnAbortedResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let mut state = self.state.lock().unwrap();
        let call_num = state.call_count;
        state.call_count += 1;

        // First request: return a long-running tool call that performs an immediate side-effect.
        if call_num == 0 {
            let script = format!(
                "echo run >> \"{}\"; sleep 60",
                self.side_effect_path.display()
            );
            let args = serde_json::json!({
                "command": script,
                "timeout_ms": 60_000
            })
            .to_string();

            return Self::sse_response(sse(vec![
                ev_response_created("resp-1"),
                ev_function_call("call-1", "shell_command", &args),
                ev_completed("resp-1"),
            ]));
        }

        // Follow-up after the repeated tool call.
        if call_num == 2 {
            return Self::sse_response(sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-3", "ok"),
                ev_completed("resp-3"),
            ]));
        }

        // Second request: if Codex includes a turn-aborted marker in the prompt,
        // behave and do not repeat the previous tool call.
        if Self::request_contains_turn_aborted_marker(request) {
            return Self::sse_response(sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "moving on"),
                ev_completed("resp-2"),
            ]));
        }

        // Otherwise, simulate the model mistakenly repeating the earlier action.
        let command = format!(
            "bash -lc 'echo run >> \"{}\"'",
            self.side_effect_path.display()
        );
        let args = serde_json::json!({ "command": command }).to_string();
        Self::sse_response(sse(vec![
            ev_response_created("resp-2"),
            ev_function_call("call-2", "shell_command", &args),
            ev_completed("resp-2"),
        ]))
    }
}

async fn wait_for_side_effect(path: &std::path::Path) {
    let check = async {
        loop {
            if let Ok(contents) = std::fs::read_to_string(path)
                && contents.lines().any(|line| line == "run")
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    };

    tokio::time::timeout(Duration::from_secs(5), check)
        .await
        .expect("side effect should be written before interrupt");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_should_prevent_repeating_aborted_work() {
    let server = start_mock_server().await;

    let fixture = test_codex()
        .with_model("gpt-5.1")
        .build(&server)
        .await
        .unwrap();

    let side_effect_path = fixture.workspace_path("side_effect.txt");
    let capture = RequestsCapture::default();
    let responder = RepeatUnlessTurnAbortedResponder {
        state: Arc::new(Mutex::new(ResponderState::default())),
        side_effect_path: side_effect_path.clone(),
    };

    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .and(capture.clone())
        .respond_with(responder)
        .up_to_n_times(10)
        .mount(&server)
        .await;

    let session_model = fixture.session_configured.model.clone();
    let cwd = fixture.cwd_path().to_path_buf();
    let codex = fixture.codex.clone();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "start first task".into(),
            }],
            final_output_json_schema: None,
            cwd: cwd.clone(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model.clone(),
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandBegin(_))).await;
    wait_for_side_effect(&side_effect_path).await;
    codex.submit(Op::Interrupt).await.unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnAborted(_))).await;

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "do something else".into(),
            }],
            final_output_json_schema: None,
            cwd,
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let contents = std::fs::read_to_string(&side_effect_path).unwrap_or_default();
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(
        lines,
        vec!["run"],
        "Codex should not repeat work from an aborted turn"
    );

    // Sanity check request count so we do not accidentally introduce infinite follow-ups.
    assert_eq!(
        capture.requests().len(),
        2,
        "expected one aborted request + one follow-up request"
    );
}
