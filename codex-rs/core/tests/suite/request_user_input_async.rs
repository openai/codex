use anyhow::Result;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::AgentMessageDelivery;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::AsyncUserInputQuestion;
use codex_protocol::items::TurnItem;
use codex_protocol::models::MessagePhase;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::openai_models::ToolMessage;
use codex_protocol::openai_models::ToolMessages;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;

#[test_case(SessionSource::Exec, false, false; "root_without_tool")]
#[test_case(SessionSource::Exec, true, true; "root_with_tool")]
#[test_case(SessionSource::SubAgent(SubAgentSource::Other("test".to_string())), true, false; "subagent")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn persistent_async_message_guidance_follows_tool_availability(
    session_source: SessionSource,
    model_supports_tool: bool,
    expect_tool_guidance: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_model_info_override("gpt-5.2", move |model| {
            model.tool_mode = Some(ToolMode::CodeModeOnly);
            model
                .experimental_supported_tools
                .retain(|tool| tool != "send_user_message_async");
            if model_supports_tool {
                model
                    .experimental_supported_tools
                    .push("send_user_message_async".to_string());
            }
            model
                .supported_reasoning_levels
                .push(ReasoningEffortPreset {
                    effort: ReasoningEffort::Persistent,
                    description: ReasoningEffort::Persistent.to_string(),
                });
            let messages = model
                .model_messages
                .as_mut()
                .expect("model instruction metadata");
            messages.persistent_instructions = None;
            messages.tools = Some(ToolMessages {
                send_user_message_async: Some(ToolMessage {
                    description: Some("Catalog async message description.".to_string()),
                }),
            });
        })
        .with_config(|config| {
            config.model_reasoning_effort = Some(ReasoningEffort::Persistent);
        })
        .build_with_auto_env(&server)
        .await?;
    let thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            session_source: Some(session_source),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?
        .thread;
    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Report progress.".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let request = responses.single_request();
    let persistent_instructions = request
        .message_input_texts("developer")
        .into_iter()
        .find(|text| text.starts_with("<persistent_mode>"))
        .expect("persistent guidance should remain available to every agent");
    assert_eq!(
        persistent_instructions.contains("via functions.send_user_message_async"),
        expect_tool_guidance,
    );
    assert_eq!(
        request.body_json()["tools"]
            .as_array()
            .expect("request tools")
            .iter()
            .any(|tool| tool["name"] == "request_user_input_async"),
        expect_tool_guidance,
    );

    Ok(())
}

#[test_case(SessionSource::Exec, None, &[], false; "root_without_opt_in")]
#[test_case(SessionSource::Exec, None, &[Feature::SendAsyncMessage], false; "root_with_retired_feature")]
#[test_case(SessionSource::Exec, Some("send_user_message_async"), &[], false; "root_with_legacy_question_tool")]
#[test_case(SessionSource::Exec, Some("request_user_input_async"), &[], false; "root_with_question_tool")]
#[test_case(SessionSource::Exec, Some("send_message_to_user_async"), &[], true; "root_with_catalog_opt_in")]
#[test_case(SessionSource::Exec, None, &[Feature::SendMessageToUserAsync], true; "root_with_feature_opt_in")]
#[test_case(SessionSource::Exec, Some("send_message_to_user_async"), &[Feature::SendMessageToUserAsync], true; "root_with_both_opt_ins")]
#[test_case(SessionSource::SubAgent(SubAgentSource::Other("test".to_string())), Some("send_message_to_user_async"), &[], false; "subagent_with_catalog_opt_in")]
#[test_case(SessionSource::SubAgent(SubAgentSource::Other("test".to_string())), None, &[Feature::SendMessageToUserAsync], false; "subagent_with_feature_opt_in")]
#[test_case(SessionSource::SubAgent(SubAgentSource::Other("test".to_string())), Some("send_message_to_user_async"), &[Feature::SendMessageToUserAsync], false; "subagent_with_both_opt_ins")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn freeform_async_message_requires_root_and_catalog_or_feature_opt_in(
    session_source: SessionSource,
    catalog_tool: Option<&'static str>,
    enabled_features: &'static [Feature],
    expected: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_model_info_override("gpt-5.2", move |model| {
            model.tool_mode = Some(ToolMode::CodeModeOnly);
            model.experimental_supported_tools =
                catalog_tool.map(str::to_string).into_iter().collect();
        })
        .with_config(move |config| {
            for &feature in enabled_features {
                config
                    .features
                    .enable(feature)
                    .expect("enable async messaging test feature");
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            session_source: Some(session_source),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?
        .thread;
    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Report progress.".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let body = responses.single_request().body_json();
    let tools = body["tools"].as_array().expect("request tools");
    assert_eq!(
        tools
            .iter()
            .filter(|tool| tool["name"] == "send_message_to_user_async")
            .count(),
        usize::from(expected),
    );
    Ok(())
}

#[test_case(Some("send_message_to_user_async"), &[]; "catalog_opt_in")]
#[test_case(None, &[Feature::SendMessageToUserAsync]; "feature_opt_in")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn freeform_async_message_emits_an_item_without_ending_the_turn(
    catalog_tool: Option<&'static str>,
    enabled_features: &'static [Feature],
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    const CALL_ID: &str = "freeform-message";
    const MESSAGE: &str = "I found a blocker that changes the plan.";
    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call_with_namespace(
                    CALL_ID,
                    "functions",
                    "send_message_to_user_async",
                    &json!({ "message": format!("  {MESSAGE}  ") }).to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_function_call_with_namespace(
                    "blank-message",
                    "functions",
                    "send_message_to_user_async",
                    &json!({ "message": "  " }).to_string(),
                ),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("final-message", "Finished."),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;
    let test = test_codex()
        .with_model_info_override("gpt-5.2", move |model| {
            model.tool_mode = Some(ToolMode::CodeModeOnly);
            model.experimental_supported_tools = vec!["request_user_input_async".to_string()];
            model
                .experimental_supported_tools
                .extend(catalog_tool.map(str::to_string));
            model
                .model_messages
                .as_mut()
                .expect("model instruction metadata")
                .tools = Some(ToolMessages {
                send_user_message_async: Some(ToolMessage {
                    description: Some("Questions only.".to_string()),
                }),
            });
        })
        .with_config(move |config| {
            for &feature in enabled_features {
                config
                    .features
                    .enable(feature)
                    .expect("enable async messaging test feature");
            }
        })
        .build_with_auto_env(&server)
        .await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Keep me updated.".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;

    let started = wait_for_event_match(test.codex.as_ref(), |event| match event {
        EventMsg::ItemStarted(event) if event.item.id() == CALL_ID => Some(event.item.clone()),
        _ => None,
    })
    .await;
    let completed = wait_for_event_match(test.codex.as_ref(), |event| match event {
        EventMsg::ItemCompleted(event) if event.item.id() == CALL_ID => Some(event.item.clone()),
        _ => None,
    })
    .await;
    let expected = serde_json::to_value(TurnItem::AgentMessage(AgentMessageItem {
        id: CALL_ID.to_string(),
        content: vec![AgentMessageContent::Text {
            text: MESSAGE.to_string(),
        }],
        phase: Some(MessagePhase::FinalAnswer),
        memory_citation: None,
        delivery: Some(AgentMessageDelivery::Async),
        questions: None,
    }))?;
    assert_eq!(serde_json::to_value(started)?, expected);
    assert_eq!(serde_json::to_value(completed)?, expected);
    wait_for_event(test.codex.as_ref(), |event| {
        let item = match event {
            EventMsg::ItemStarted(event) => Some(&event.item),
            EventMsg::ItemCompleted(event) => Some(&event.item),
            _ => None,
        };
        assert!(!matches!(item, Some(TurnItem::AgentMessage(message)) if message.delivery == Some(AgentMessageDelivery::Async)));
        matches!(event, EventMsg::TurnComplete(_))
    }).await;

    let requests = responses.requests();
    assert_eq!(requests.len(), 3);
    let body = requests[0].body_json();
    let tools = body["tools"].as_array().expect("request tools");
    let freeform_tool = tools
        .iter()
        .find(|tool| tool["name"] == "send_message_to_user_async")
        .expect("freeform tool is directly available");
    assert_eq!(freeform_tool["parameters"]["required"], json!(["message"]));
    assert!(
        freeform_tool["description"]
            .as_str()
            .expect("description")
            .contains("report a critical blocker")
    );
    let question_tool = tools
        .iter()
        .find(|tool| tool["name"] == "request_user_input_async")
        .expect("question tool is still directly available");
    assert_eq!(question_tool["description"], "Questions only.");
    assert_eq!(
        requests[1].function_call_output_text(CALL_ID),
        Some(r#"{"accepted":true}"#.to_string())
    );
    assert_eq!(
        requests[2].function_call_output_text("blank-message"),
        Some("message must not be empty".to_string())
    );
    assert!(
        !requests[1]
            .input()
            .into_iter()
            .any(|item| item["type"] == "message"
                && item["role"] == "assistant"
                && item.to_string().contains(MESSAGE))
    );
    Ok(())
}

#[test_case(None, "send_user_message_async"; "fallback_description")]
#[test_case(None, "request_user_input_async"; "current_catalog_name")]
#[test_case(Some(ToolMessages { send_user_message_async: None }), "send_user_message_async"; "missing_tool")]
#[test_case(Some(ToolMessages { send_user_message_async: Some(ToolMessage::default()) }), "send_user_message_async"; "missing_description")]
#[test_case(Some(ToolMessages { send_user_message_async: Some(ToolMessage { description: Some("Catalog async message description.".to_string()) }) }), "send_user_message_async"; "catalog_description")]
#[test_case(Some(ToolMessages { send_user_message_async: Some(ToolMessage { description: Some(String::new()) }) }), "send_user_message_async"; "empty_description")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn request_user_input_async_emits_item_and_does_not_end_the_turn(
    tool_messages: Option<ToolMessages>,
    catalog_tool_name: &'static str,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    const CALL_ID: &str = "async-message-call";
    const TITLE: &str = "Which environment should I use?";
    const MESSAGE: &str =
        "Which environment should I use?\n- Staging\n- Production\n\nWhat deadline should I use?";
    let questions = vec![
        AsyncUserInputQuestion {
            title: TITLE.to_string(),
            options: Some(vec!["Staging".to_string(), "Production".to_string()]),
        },
        AsyncUserInputQuestion {
            title: "What deadline should I use?".to_string(),
            options: None,
        },
    ];

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call_with_namespace(
                    CALL_ID,
                    "functions",
                    "request_user_input_async",
                    &json!({ "questions": [
                        {"title": TITLE, "options": ["Staging", "Production"]},
                        {"title": "What deadline should I use?"}
                    ] })
                    .to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("final-message", "Finished."),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let expected_description = tool_messages
        .as_ref()
        .and_then(|tools| tools.send_user_message_async.as_ref())
        .and_then(|tool| tool.description.as_deref())
        .unwrap_or(
            "Ask the user one or more questions during ongoing work. Use this tool only to request missing information, preferences, constraints, clarification, or approval. The tool returns immediately without ending the turn or waiting for a reply; any reply arrives asynchronously as a new user message. Keep questions concise, self-contained, and easy to understand, using a level of detail appropriate to the user and task. The UI always allows a free-text answer, including when suggested options are provided. A preselected option is not submitted automatically.",
        )
        .to_string();
    let test = test_codex()
        .with_model_info_override("gpt-5.2", move |model| {
            model.tool_mode = Some(ToolMode::CodeModeOnly);
            model.experimental_supported_tools.retain(|tool| {
                tool != "send_user_message_async" && tool != "request_user_input_async"
            });
            model
                .experimental_supported_tools
                .push(catalog_tool_name.to_string());
            model
                .model_messages
                .as_mut()
                .expect("model instruction metadata")
                .tools = tool_messages;
        })
        .build_with_auto_env(&server)
        .await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Keep me updated.".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;

    let started = wait_for_event_match(test.codex.as_ref(), |event| {
        let EventMsg::ItemStarted(event) = event else {
            return None;
        };
        let TurnItem::AgentMessage(message) = &event.item else {
            return None;
        };
        if message.delivery != Some(AgentMessageDelivery::Async) {
            return None;
        }
        Some(message.clone())
    })
    .await;
    assert_eq!(
        serde_json::to_value(&started)?,
        serde_json::to_value(AgentMessageItem {
            id: CALL_ID.to_string(),
            content: vec![AgentMessageContent::Text {
                text: MESSAGE.to_string(),
            }],
            phase: Some(MessagePhase::FinalAnswer),
            memory_citation: None,
            delivery: Some(AgentMessageDelivery::Async),
            questions: Some(questions),
        })?
    );

    let completed = wait_for_event_match(test.codex.as_ref(), |event| {
        let EventMsg::ItemCompleted(event) = event else {
            return None;
        };
        let TurnItem::AgentMessage(message) = &event.item else {
            return None;
        };
        if message.delivery != Some(AgentMessageDelivery::Async) {
            return None;
        }
        Some(message.clone())
    })
    .await;
    assert_eq!(
        serde_json::to_value(completed)?,
        serde_json::to_value(started)?
    );

    wait_for_event(test.codex.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    for request in &requests {
        let body = request.body_json();
        let tools = body["tools"].as_array().expect("request tools");
        let tool = tools
            .iter()
            .find(|tool| tool["type"] == "function" && tool["name"] == "request_user_input_async")
            .expect("the async message tool should be directly visible to the model");
        assert_eq!(tool["description"], expected_description);
        assert_eq!(tool["strict"], false);
        assert_eq!(tool["parameters"]["required"], json!(["questions"]));
        assert_eq!(tool["parameters"]["additionalProperties"], false);
        let schema = &tool["parameters"]["properties"]["questions"];
        assert_eq!(schema["minItems"], 1);
        assert_eq!(schema["items"]["required"], json!(["title"]));
        assert_eq!(schema["items"]["additionalProperties"], false);
        assert_eq!(schema["items"]["properties"]["options"]["minItems"], 1);
        assert_eq!(
            schema["items"]["properties"]["options"]["items"]["type"],
            "string"
        );
        assert!(
            tools
                .iter()
                .all(|tool| tool["name"] != "send_user_message_async")
        );
    }
    assert_eq!(
        requests[1].function_call_output_text(CALL_ID),
        Some(r#"{"accepted":true}"#.to_string())
    );
    let has_synthetic_assistant_message = requests[1].input().into_iter().any(|item| {
        item["type"] == "message" && item["role"] == "assistant" && item.to_string().contains(TITLE)
    });
    assert!(
        !has_synthetic_assistant_message,
        "the user-visible item should not inject a synthetic assistant message into model context"
    );

    Ok(())
}

#[test_case(json!({"questions": []}), "questions must not be empty"; "no_questions")]
#[test_case(json!({"questions": [{"title": " "}]}), "question titles must not be empty"; "blank_title")]
#[test_case(json!({"questions": [{"title": "Which?", "options": []}]}), "options must contain at least one non-empty answer"; "empty_options")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_async_questions_do_not_emit_an_item(
    arguments: serde_json::Value,
    expected_error: &str,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call_with_namespace(
                    "invalid",
                    "functions",
                    "request_user_input_async",
                    &arguments.to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;
    let test = test_codex()
        .with_model_info_override("gpt-5.2", |model| {
            model.tool_mode = Some(ToolMode::CodeModeOnly);
            model
                .experimental_supported_tools
                .push("request_user_input_async".to_string());
        })
        .build_with_auto_env(&server)
        .await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Get clarification.".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(test.codex.as_ref(), |event| {
        let item = match event {
            EventMsg::ItemStarted(event) => Some(&event.item),
            EventMsg::ItemCompleted(event) => Some(&event.item),
            _ => None,
        };
        assert!(!matches!(item, Some(TurnItem::AgentMessage(message)) if message.delivery == Some(AgentMessageDelivery::Async)));
        matches!(event, EventMsg::TurnComplete(_))
    }).await;
    assert_eq!(
        responses.requests()[1].function_call_output_text("invalid"),
        Some(expected_error.to_string())
    );
    Ok(())
}
