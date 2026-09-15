use super::*;
use crate::GuardianAuthorizationVersion;
use crate::session::step_context::StepContext;
use crate::session::tests::make_session_and_context;
use crate::session::tests::make_session_and_context_with_auth_and_config_and_rx;
use crate::session::tests::make_session_and_context_with_rx;
use crate::session::tests::update_selected_settings_for_test;
use crate::session::tests::update_turn_settings_for_test;
use crate::state::ActiveTurn;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::turn_diff_tracker::TurnDiffTracker;
use codex_guardian_context::RenderedVerifiedAnswers;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
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

#[test_case(None, RenderedVerifiedAnswers { fragments: vec![], complete: true }; "empty")]
#[test_case(Some(("other_question", "A".to_owned())), RenderedVerifiedAnswers { fragments: vec![], complete: true }; "unrequested question")]
#[test_case(Some(("pick_one", " ".to_owned())), RenderedVerifiedAnswers { fragments: vec![], complete: true }; "blank answer")]
#[test_case(Some(("pick_one", "A".to_owned())), RenderedVerifiedAnswers {
    fragments: vec!["assistant: Pick one\nassistant: A: A\nuser: A\n".to_owned()],
    complete: true,
}; "genuine answer")]
#[test_case(Some(("pick_one", "A\nB".to_owned())), RenderedVerifiedAnswers {
    fragments: vec!["assistant: Pick one\nuser: A\nuser: B\n".to_owned()],
    complete: true,
}; "short multiline answer")]
#[test_case(Some(("pick_one", "x\n".repeat(/*n*/ 900))), RenderedVerifiedAnswers {
    fragments: vec!["Host notice: some verified user answers are unavailable within the evidence budget. Do not treat the remaining answers as complete authorization for an action.\n".to_owned()],
    complete: false,
}; "oversized multiline answer")]
#[tokio::test]
async fn request_user_input_sets_non_blocking_outside_plan_mode(
    answer: Option<(&str, String)>,
    expected: RenderedVerifiedAnswers,
) {
    for thread_context_enabled in [false, true] {
        let (session, turn, events) = make_session_and_context_with_auth_and_config_and_rx(
            codex_login::CodexAuth::from_api_key("Test API Key"),
            Vec::new(),
            |config| {
                config
                    .features
                    .set_enabled(Feature::GuardianThreadContext, thread_context_enabled)
                    .expect("test context mode");
            },
        )
        .await;
        session
            .services
            .thread_extension_data
            .insert(GuardianReviewEvidence::default());
        let original_history = session.conversation_history_snapshot().await;
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
        let history = session.conversation_history_snapshot().await;
        if thread_context_enabled {
            let RenderedVerifiedAnswers {
                fragments,
                complete,
            } = codex_guardian_context::render_verified_answers(
                history.retained_context().expect("host context snapshot"),
            );
            let expected_fragments = expected
                .fragments
                .iter()
                .map(|fragment| {
                    if expected.complete {
                        format!("Retained source order: 0\n{fragment}")
                    } else {
                        fragment.clone()
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(
                (&fragments, complete),
                (&expected_fragments, expected.complete)
            );
        } else {
            assert_eq!(
                history.retained_context(),
                original_history.retained_context()
            );
        }
        let evidence = session
            .services
            .thread_extension_data
            .get_or_init(GuardianReviewEvidence::default);
        let legacy_answer = evidence.user_input_for_call(history.as_ref(), "call-1");
        let has_answer = matches!(&answer, Some(("pick_one", answer)) if !answer.trim().is_empty());
        assert_eq!(
            evidence.authorization_version(history.as_ref()),
            GuardianAuthorizationVersion {
                user_message_revision: original_history.user_message_revision()
                    + u64::from(thread_context_enabled && has_answer),
                user_input_response_count: usize::from(!thread_context_enabled && has_answer),
                retained_context_complete: !thread_context_enabled || expected.complete,
            },
        );
        match &answer {
            Some(("pick_one", answer)) if !thread_context_enabled && !answer.trim().is_empty() => {
                let fragment = legacy_answer.expect("legacy answer");
                assert!(codex_utils_output_truncation::approx_token_count(&fragment) <= 900);
                assert_eq!(
                    fragment.contains("<truncated omitted_approx_tokens="),
                    !expected.complete,
                );
            }
            _ => assert_eq!(
                legacy_answer,
                expected
                    .fragments
                    .first()
                    .cloned()
                    .filter(|_| expected.complete)
            ),
        }
    }
}

#[tokio::test]
async fn request_user_input_sets_blocking_from_turn_mode() {
    let (session, mut turn, events) = make_session_and_context_with_rx().await;
    update_turn_settings_for_test(
        Arc::get_mut(&mut turn).expect("turn context should be uniquely owned"),
        |settings| {
            update_selected_settings_for_test(settings, |selected| {
                selected.collaboration_mode.mode = ModeKind::Plan;
            });
        },
    );
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
