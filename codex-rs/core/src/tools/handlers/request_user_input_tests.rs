use super::*;
use crate::session::step_context::StepContext;
use crate::session::tests::make_session_and_context;
use crate::session::tests::make_session_and_context_with_rx;
use crate::state::ActiveTurn;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::turn_diff_tracker::TurnDiffTracker;
use codex_protocol::ThreadId;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_utils_output_truncation::approx_token_count;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use test_case::test_case;
use tokio::sync::Mutex;

#[tokio::test]
async fn multi_agent_v2_request_user_input_rejects_subagent_threads() {
    let (session, mut turn) = make_session_and_context().await;
    turn.session_source = SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
        parent_thread_id: ThreadId::new(),
        depth: 1,
        agent_path: None,
        agent_nickname: None,
        agent_role: None,
    });
    let turn = Arc::new(turn);

    let result = RequestUserInputHandler {
        available_modes: Vec::new(),
    }
    .handle(ToolInvocation {
        session: Arc::new(session),
        step_context: StepContext::for_test(Arc::clone(&turn)),
        turn,
        cancellation_token: tokio_util::sync::CancellationToken::new(),
        tracker: Arc::new(Mutex::new(TurnDiffTracker::default())),
        call_id: "call-1".to_string(),
        tool_name: codex_tools::ToolName::plain(REQUEST_USER_INPUT_TOOL_NAME),
        source: crate::tools::context::ToolCallSource::Direct,
        payload: ToolPayload::Function {
            arguments: json!({
                "questions": [{
                    "header": "Hdr",
                    "question": "Pick one",
                    "id": "pick_one",
                    "options": [
                        {
                            "label": "A",
                            "description": "A"
                        },
                        {
                            "label": "B",
                            "description": "B"
                        }
                    ]
                }]
            })
            .to_string(),
        },
    })
    .await;

    let Err(err) = result else {
        panic!("sub-agent request_user_input should fail");
    };
    assert_eq!(
        err,
        FunctionCallError::RespondToModel(
            "request_user_input can only be used by the root thread".to_string(),
        )
    );
}

#[test_case(None; "empty")]
#[test_case(Some(("other_question", "A".to_owned())); "unrequested question")]
#[test_case(Some(("pick_one", " ".to_owned())); "blank answer")]
#[test_case(Some(("pick_one", "A".to_owned())); "genuine answer")]
#[test_case(Some(("pick_one", "x\n".repeat(900))); "multiline answer remains bounded")]
#[tokio::test]
async fn request_user_input_sets_non_blocking_outside_plan_mode(answer: Option<(&str, String)>) {
    let (session, turn, events) = make_session_and_context_with_rx().await;
    *session.active_turn.lock().await = Some(ActiveTurn::default());

    let request = tokio::spawn({
        let session = Arc::clone(&session);
        let turn = Arc::clone(&turn);
        async move {
            RequestUserInputHandler {
                available_modes: vec![ModeKind::Default],
            }
            .handle(ToolInvocation {
                session,
                step_context: StepContext::for_test(Arc::clone(&turn)),
                turn,
                cancellation_token: tokio_util::sync::CancellationToken::new(),
                tracker: Arc::new(Mutex::new(TurnDiffTracker::default())),
                call_id: "call-1".to_string(),
                tool_name: codex_tools::ToolName::plain(REQUEST_USER_INPUT_TOOL_NAME),
                source: crate::tools::context::ToolCallSource::Direct,
                payload: ToolPayload::Function {
                    arguments: json!({
                        "questions": [{
                            "header": "Hdr",
                            "question": "Pick one",
                            "id": "pick_one",
                            "options": [
                                {
                                    "label": "A",
                                    "description": "A"
                                },
                                {
                                    "label": "B",
                                    "description": "B"
                                }
                            ]
                        }]
                    })
                    .to_string(),
                },
            })
            .await
        }
    });

    let event = events.recv().await.expect("request_user_input event");
    let EventMsg::RequestUserInput(request_event) = event.msg else {
        panic!("expected request_user_input event");
    };
    assert_eq!(request_event.call_id, "call-1");
    assert!(!request_event.is_blocking);

    session
        .notify_user_input_response(
            &request_event.turn_id,
            RequestUserInputResponse {
                answers: answer
                    .iter()
                    .map(|(question_id, answer)| {
                        (
                            (*question_id).to_owned(),
                            RequestUserInputAnswer {
                                answers: vec![answer.clone()],
                            },
                        )
                    })
                    .collect(),
            },
        )
        .await;

    let output = request
        .await
        .expect("request_user_input handler task should finish")
        .expect("request_user_input handler should succeed");
    assert!(output.success_for_logging());
    let recorded_answer = session
        .services
        .thread_extension_data
        .get::<GuardianReviewEvidence>()
        .and_then(|evidence| evidence.user_input_for_call("call-1"));
    if let Some(("pick_one", answer)) = answer {
        if answer.contains('\n') {
            let fragment = recorded_answer.expect("genuine multiline answer should be recorded");
            assert!(approx_token_count(&fragment) <= MAX_GUARDIAN_USER_INPUT_TOKENS);
            assert!(fragment.contains("<truncated omitted_approx_tokens="));
        } else if !answer.trim().is_empty() {
            assert_eq!(
                recorded_answer,
                Some("assistant: Pick one\nassistant: A: A\nuser: A\n".to_owned())
            );
        } else {
            assert_eq!(recorded_answer, None);
        }
    } else {
        assert_eq!(recorded_answer, None);
    }
}

#[tokio::test]
async fn guardian_user_input_evidence_is_bounded() {
    let (session, turn, _) = make_session_and_context_with_rx().await;
    let evidence = GuardianReviewEvidence::default();
    for index in 0..10 {
        let call_id = format!("call-{index}");
        session
            .record_conversation_items(
                turn.as_ref(),
                &[ResponseItem::FunctionCall {
                    id: None,
                    name: REQUEST_USER_INPUT_TOOL_NAME.to_owned(),
                    namespace: None,
                    arguments: "{}".to_owned(),
                    call_id: call_id.clone(),
                    encrypted_function_args: None,
                    internal_chat_message_metadata_passthrough: None,
                }],
            )
            .await;
        evidence.record_user_input(&call_id, format!("user: answer-{index}\n"));
    }
    let history = session.conversation_history_snapshot().await;
    assert_eq!(
        evidence.user_input_fragments(history.as_ref()),
        (2..10)
            .map(|index| format!("user: answer-{index}\n"))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        evidence
            .authorization_version(history.as_ref())
            .user_input_response_count,
        10
    );
}

#[tokio::test]
async fn request_user_input_sets_blocking_from_turn_mode() {
    let (session, mut turn, events) = make_session_and_context_with_rx().await;
    Arc::get_mut(&mut turn)
        .expect("turn context should be uniquely owned")
        .mode = ModeKind::Plan;
    *session.active_turn.lock().await = Some(ActiveTurn::default());

    let request = tokio::spawn({
        let session = Arc::clone(&session);
        let turn = Arc::clone(&turn);
        async move {
            RequestUserInputHandler {
                available_modes: vec![ModeKind::Plan],
            }
            .handle(ToolInvocation {
                session,
                step_context: StepContext::for_test(Arc::clone(&turn)),
                turn,
                cancellation_token: tokio_util::sync::CancellationToken::new(),
                tracker: Arc::new(Mutex::new(TurnDiffTracker::default())),
                call_id: "call-1".to_string(),
                tool_name: codex_tools::ToolName::plain(REQUEST_USER_INPUT_TOOL_NAME),
                source: crate::tools::context::ToolCallSource::Direct,
                payload: ToolPayload::Function {
                    arguments: json!({
                        "questions": [{
                            "header": "Hdr",
                            "question": "Pick one",
                            "id": "pick_one",
                            "options": [
                                {
                                    "label": "A",
                                    "description": "A"
                                },
                                {
                                    "label": "B",
                                    "description": "B"
                                }
                            ]
                        }]
                    })
                    .to_string(),
                },
            })
            .await
        }
    });

    let event = events.recv().await.expect("request_user_input event");
    let EventMsg::RequestUserInput(request_event) = event.msg else {
        panic!("expected request_user_input event");
    };
    assert_eq!(request_event.call_id, "call-1");
    assert!(request_event.is_blocking);

    session
        .notify_user_input_response(
            &request_event.turn_id,
            RequestUserInputResponse {
                answers: HashMap::new(),
            },
        )
        .await;

    request
        .await
        .expect("request_user_input handler task should finish")
        .expect("request_user_input handler should succeed");
}
