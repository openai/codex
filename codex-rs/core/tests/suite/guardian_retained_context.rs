//! Retained-instruction and answer lifecycles through real sessions and durable checkpoints.
//! Real user input covers steering and compaction. Resume and child forks use production paths.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_core::CodexThread;
use codex_core::ForkSnapshot;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_core::config::ThreadStoreConfig;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadIdleInput;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolLifecycleFuture;
use codex_extension_api::ToolStartInput;
use codex_features::Feature;
use codex_history::GuardianHistoryCheckpoint;
use codex_history::InitialHistory;
use codex_history::ResumedHistory;
use codex_history::RetainedContext;
use codex_history::RetainedContextEntry;
use codex_history::RetainedContextEvent;
use codex_history::RetainedContextOrder;
use codex_history::RolloutItem;
use codex_history::VerifiedAnswer;
use codex_history::VerifiedQuestionAnswer;
use codex_protocol::ResponseItemId;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::mcp::ClientMcpExtensions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_protocol::user_input::UserInput;
use codex_thread_store::ForkBoundary;
use codex_thread_store::LoadThreadHistoryParams;
use codex_thread_store::PrepareForkParams;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_message_item_added;
use core_test_support::responses::ev_output_text_delta;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;
use tokio::sync::Notify;
use tokio::sync::oneshot;
use wiremock::MockServer;
use wiremock::matchers::header;

// Keep child completion out of the parent's history and wait for parent turn cleanup
// before checking inherited authorization.
#[derive(Default)]
struct ForkTestLifecycle {
    entered: Notify,
    release: Notify,
}

impl ToolLifecycleContributor for ForkTestLifecycle {
    fn on_tool_start<'a>(&'a self, input: ToolStartInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async move {
            if input.call_id == "child-pause" {
                self.entered.notify_one();
                self.release.notified().await;
            }
        })
    }
}

#[derive(Default)]
struct ThreadIdle(Notify);

impl ThreadLifecycleContributor<Config> for ForkTestLifecycle {
    fn on_thread_idle<'a>(&'a self, input: ThreadIdleInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            input
                .thread_store
                .get_or_init(ThreadIdle::default)
                .0
                .notify_one();
        })
    }
}

async fn wait_for_thread_idle(thread: &CodexThread) {
    // TurnComplete precedes active-turn cleanup. Consume each turn's idle notification
    // separately, and scope it to the thread so child completion cannot satisfy it.
    let idle = thread
        .thread_extension_data()
        .get_or_init(ThreadIdle::default);
    tokio::time::timeout(Duration::from_secs(10), idle.0.notified())
        .await
        .expect("thread should become idle");
}

async fn record_answer(
    thread: &CodexThread,
    server: &MockServer,
    call_id: &str,
    answer: &str,
) -> Result<VerifiedAnswer> {
    let question = format!("May I publish {call_id}?");
    mount_sse_sequence(
        server,
        vec![
            sse(vec![
                ev_function_call(
                    call_id,
                    "request_user_input",
                    &json!({"questions": [{
                        "id": "publish", "header": "Publish", "question": question,
                        "options": [
                            {"label": "Yes", "description": "Publish privately."},
                            {"label": "No", "description": "Keep local."}
                        ]
                    }]})
                    .to_string(),
                ),
                ev_completed(&format!("ask-{call_id}")),
            ]),
            sse(vec![ev_completed(&format!("answered-{call_id}"))]),
        ],
    )
    .await;
    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: format!("Check authorization for {call_id}."),
            text_elements: Vec::new(),
        }]))
        .await?;
    let request = wait_for_event_match(thread, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    thread
        .submit(Op::UserInputAnswer {
            id: request.turn_id.clone(),
            response: RequestUserInputResponse {
                answers: HashMap::from([(
                    "publish".to_owned(),
                    RequestUserInputAnswer {
                        answers: vec![answer.to_owned()],
                    },
                )]),
            },
        })
        .await?;
    wait_for_event(thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    let retained = VerifiedAnswer {
        turn_id: request.turn_id,
        call_id: call_id.to_owned(),
        questions: vec![VerifiedQuestionAnswer {
            question,
            answer: answer.to_owned(),
        }],
    };
    thread.ensure_rollout_materialized().await;
    let history = thread.conversation_history_snapshot().await;
    let acceptance_order = history
        .retained_context()
        .context("retained answer context")?
        .ordered_entries()
        .find_map(|(order, entry)| match (order, entry) {
            (RetainedContextOrder::Local(order), RetainedContextEntry::VerifiedAnswer(answer))
                if answer == &retained =>
            {
                Some(order)
            }
            _ => None,
        })
        .context("recorded answer order")?;
    let event = RolloutItem::RetainedContext(RetainedContextEvent::VerifiedAnswer {
        answer: retained.clone(),
        acceptance_order: Some(acceptance_order),
    });
    // Repeated delivery must not duplicate evidence during replay.
    thread.append_rollout_items(&[event.clone(), event]).await?;
    Ok(retained)
}

async fn load_context(test: &TestCodex, thread: &CodexThread) -> Result<Vec<RolloutItem>> {
    Ok(test
        .thread_store
        .load_latest_model_context(LoadThreadHistoryParams {
            thread_id: thread.startup_metadata().thread_id,
            include_archived: false,
        })
        .await?
        .items)
}

async fn resume(test: &TestCodex, thread: &CodexThread) -> Result<Arc<CodexThread>> {
    let thread_id = thread.startup_metadata().thread_id;
    thread.shutdown_and_wait().await?;
    test.thread_manager.remove_thread(&thread_id).await;
    let saved = load_context(test, thread).await?;
    let items: Vec<RolloutItem> = serde_json::from_value(serde_json::to_value(saved)?)?;
    Ok(test
        .thread_manager
        .resume_thread_with_history(
            test.config.clone(),
            InitialHistory::Resumed(ResumedHistory {
                conversation_id: thread_id,
                history: Arc::new(items),
                rollout_path: None,
            }),
            test.thread_manager.auth_manager(),
            /*parent_trace*/ None,
            ClientMcpExtensions::default(),
        )
        .await?
        .thread)
}

#[test_case("question", ModeKind::Default, ""; "server message id")]
#[test_case("", ModeKind::Default, ""; "generated message id")]
#[test_case("question", ModeKind::Plan, ""; "plan with server message id")]
#[test_case("", ModeKind::Plan, ""; "plan with generated message id")]
#[test_case("question", ModeKind::Plan, "Here is the plan:\n"; "text and plan share one order")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streamed_question_precedes_reply_across_resume(
    message_id: &str,
    mode: ModeKind,
    preamble: &str,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    const INITIAL: &str = "Prepare the deployment. Staging only.";
    const QUESTION: &str = "Deploy abc123?";
    const ANSWER: &str = "Yes.";
    let (prefix, suffix) = if mode == ModeKind::Plan {
        (format!("{preamble}<proposed_plan>\n"), "\n</proposed_plan>")
    } else {
        (String::new(), "")
    };
    let question = format!("{prefix}{QUESTION}{suffix}");
    let (release, gate) = oneshot::channel();
    let (server, _completions) = start_streaming_sse_server(vec![
        vec![
            StreamingSseChunk {
                gate: None,
                body: sse(vec![
                    ev_response_created("question-response"),
                    ev_message_item_added(message_id, ""),
                    ev_output_text_delta(&format!("{prefix}Deploy ")),
                    ev_output_text_delta("abc123?"),
                    ev_output_text_delta(suffix),
                ]),
            },
            StreamingSseChunk {
                gate: Some(gate),
                body: sse(vec![
                    ev_assistant_message(message_id, &question),
                    ev_completed("question-response"),
                ]),
            },
        ],
        vec![StreamingSseChunk {
            gate: None,
            body: sse(vec![
                ev_assistant_message("empty-final", ""),
                ev_completed("answer-response"),
            ]),
        }],
    ])
    .await;
    let test = test_codex()
        .with_model("gpt-5.4")
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_config(|config| {
            config.experimental_thread_store = ThreadStoreConfig::Local;
            config
                .features
                .enable(Feature::GuardianThreadContext)
                .expect("enable retained context");
        })
        .build_with_streaming_server(&server)
        .await?;
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: INITIAL.to_owned(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                collaboration_mode: Some(CollaborationMode {
                    mode,
                    settings: Settings {
                        model: test.session_configured.model.clone(),
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            }),
        )
        .await?;
    wait_for_event(&test.codex, |event| match event {
        EventMsg::AgentMessageContentDelta(delta) => delta.delta.contains("abc123?"),
        EventMsg::PlanDelta(delta) => delta.delta.contains("abc123?"),
        _ => false,
    })
    .await;
    // The reply is accepted while the question's completed item is still withheld.
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: ANSWER.to_owned(),
            text_elements: Vec::new(),
        }]))
        .await?;
    release.send(()).expect("release question completion");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let history = test.codex.conversation_history_snapshot().await;
    let retained = history
        .retained_context()
        .context("live retained context")?;
    assert!(!retained.has_omitted_assistant_messages());
    assert_eq!(
        retained
            .ordered_entries()
            .map(|(order, entry)| match entry {
                RetainedContextEntry::UserMessage(message) =>
                    (order, "user", message.text.as_str()),
                RetainedContextEntry::AssistantMessage(message) => {
                    (order, "assistant", message.text.as_str())
                }
                RetainedContextEntry::VerifiedAnswer(_) => panic!("expected plain chat messages"),
            })
            .collect::<Vec<_>>(),
        vec![
            (RetainedContextOrder::Local(0), "user", INITIAL),
            (
                RetainedContextOrder::Local(1),
                "assistant",
                question.as_str()
            ),
            (RetainedContextOrder::Local(2), "user", ANSWER),
        ],
    );
    let resumed = resume(&test, &test.codex).await?;
    assert_eq!(
        resumed
            .conversation_history_snapshot()
            .await
            .retained_context(),
        Some(retained),
    );
    resumed.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}

async fn compact_and_assert_answers(
    test: &TestCodex,
    thread: &CodexThread,
    expected: &[VerifiedAnswer],
) -> Result<RetainedContext> {
    // Inspect live state by persisting a real compaction checkpoint, not a private getter.
    // Repeating this after legacy rollback replay also catches checkpoint resurrection.
    thread.submit(Op::Compact).await?;
    wait_for_event(thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    thread.flush_rollout().await?;
    let history = load_context(test, thread).await?;
    let checkpoint = history
        .iter()
        .rev()
        .find_map(|item| match item {
            RolloutItem::Compacted(checkpoint) => Some(checkpoint),
            _ => None,
        })
        .context("compaction checkpoint")?;
    let actual = checkpoint
        .retained_context
        .as_ref()
        .context("retained context checkpoint")?;
    assert_eq!(
        actual.verified_answers().cloned().collect::<Vec<_>>(),
        expected
    );
    Ok(actual.clone())
}

#[derive(Clone, Copy)]
enum InstructionSize {
    Normal,
    Oversized,
}

#[test_case(InstructionSize::Normal; "enabled paginated resume")]
#[test_case(InstructionSize::Oversized; "oversized instruction resume")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retained_instructions_keep_identity_across_compaction_and_resume(
    instruction_size: InstructionSize,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    const INITIAL: &str = "Never publish publicly.";
    const QUESTION: &str = "Should I also inspect the README?";
    const STEER: &str = "Yes, inspect the README.";
    let initial = match instruction_size {
        InstructionSize::Normal => INITIAL.to_owned(),
        InstructionSize::Oversized => format!(
            "{INITIAL}\n{}\nNever upload logs.",
            "Project detail. ".repeat(2_000)
        ),
    };
    let server = start_mock_server().await;
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_config(move |config| {
            config.experimental_thread_store = ThreadStoreConfig::Local;

            // Exercise local compaction's rebuilt user messages, not an opaque checkpoint.
            config.model_provider.name = "Local compaction test provider".to_owned();
            config
                .features
                .disable(Feature::TokenBudget)
                .expect("use local compaction");
            config
                .features
                .enable(Feature::DefaultModeRequestUserInput)
                .expect("enable request_user_input");
        })
        .build_with_auto_env(&server)
        .await?;
    let questions = [
        ("before-steer", "Publish?", "Only privately."),
        ("after-steer", "Publish the README?", "Do not publish it."),
    ];
    let mut responses = questions
        .iter()
        .map(|(call_id, question, _)| {
            let mut events = vec![
                ev_function_call(
                    call_id,
                    "request_user_input",
                    &json!({"questions": [{
                        "id": "publish", "header": "Publish", "question": question,
                        "options": [
                            {"label": "Yes", "description": "Publish privately."},
                            {"label": "No", "description": "Keep local."}
                        ]
                    }]})
                    .to_string(),
                ),
                ev_completed(call_id),
            ];
            if *call_id == "before-steer" {
                events.insert(
                    /*index*/ 0,
                    ev_assistant_message("ordinary-question", QUESTION),
                );
            }
            sse(events)
        })
        .collect::<Vec<_>>();
    responses.push(sse(vec![ev_completed("done")]));
    responses.push(sse(vec![
        ev_assistant_message("summary-0", "Compacted inspection context."),
        ev_completed("compact-0"),
    ]));
    let response_mock = mount_sse_sequence(&server, responses).await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: initial.clone(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let mut answers = Vec::new();
    for (call_id, question, answer) in questions {
        let request = wait_for_event_match(&test.codex, |event| match event {
            EventMsg::RequestUserInput(request) => Some(request.clone()),
            _ => None,
        })
        .await;
        if answers.is_empty() {
            test.codex
                .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                    text: STEER.to_owned(),
                    text_elements: Vec::new(),
                }]))
                .await?;
        }
        test.codex
            .submit(Op::UserInputAnswer {
                id: request.turn_id.clone(),
                response: RequestUserInputResponse {
                    answers: HashMap::from([(
                        "publish".to_owned(),
                        RequestUserInputAnswer {
                            answers: vec![answer.to_owned()],
                        },
                    )]),
                },
            })
            .await?;
        answers.push(VerifiedAnswer {
            turn_id: request.turn_id,
            call_id: call_id.to_owned(),
            questions: vec![VerifiedQuestionAnswer {
                question: question.to_owned(),
                answer: answer.to_owned(),
            }],
        });
    }
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    // Rebuild from source events before there is a compaction checkpoint. The answer
    // was persisted first, but the accepted steering instruction must still precede it.
    let thread = resume(&test, &test.codex).await?;
    assert_eq!(answers[0].turn_id, answers[1].turn_id);
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 3);
    assert!(requests[0].has_message_with_input_texts("user", |texts| texts == [initial.as_str()]));
    for (index, request) in requests.iter().skip(1).enumerate() {
        assert!(request.has_message_with_input_texts("user", |texts| texts == [STEER]));
        for answer in &answers[..=index] {
            let (output, _) = request
                .function_call_output_content_and_success(&answer.call_id)
                .context("answer tool output")?;
            let output: serde_json::Value = serde_json::from_str(&output.context("answer text")?)?;
            assert_eq!(
                output,
                json!({"answers": {"publish": {"answers": [answer.questions[0].answer]}}})
            );
        }
    }
    let history = thread.conversation_history_snapshot().await;
    let retained = history.retained_context().context("retained originals")?;
    let revisions = retained
        .ordered_entries()
        .filter_map(|(_, entry)| retained.source(entry))
        .map(|source| (source.id.message_id, source.revision))
        .collect::<std::collections::HashMap<_, _>>();
    // Shared order: initial input, ordinary question, first tool call, steer, first
    // answer, second tool call, second answer. Recording the queued steer later must not move it.
    let user_messages = [(0, initial.as_str()), (3, STEER)]
        .into_iter()
        .enumerate()
        .map(|(index, (order, text))| {
            let message_id = history
                .items()
                .find(|item| {
                    matches!(
                        item,
                        ResponseItem::Message { role, content, .. }
                            if role == "user" && matches!(
                                content.as_slice(),
                                [ContentItem::InputText { text: body }] if body == text
                            )
                    )
                })
                .and_then(ResponseItem::id)
                .expect("original user-message identity");
            json!({
                "order": order, "turn_id": answers[index].turn_id,
                "message_id": message_id.as_str(), "revision": revisions[message_id.as_str()],
                "text": codex_guardian_context::truncate_text(text, /*max_tokens*/ 900),
                "complete": index != 0 || matches!(instruction_size, InstructionSize::Normal),
            })
        })
        .collect::<Vec<_>>();
    let ordered_answers = answers
        .iter()
        .zip([4, 6])
        .map(|(answer, order)| {
            let mut value = json!(answer);
            value["order"] = json!(order);
            value
        })
        .collect::<Vec<_>>();
    let next_order = 7_u64;
    let expected = json!({
        "user_messages": user_messages, "user_messages_incomplete": false,
        "assistant_messages": [{"order": 1, "turn_id": answers[0].turn_id,
            "message_id": "ordinary-question", "revision": revisions["ordinary-question"],
            "text": QUESTION, "complete": true}],
        "assistant_messages_incomplete": false,
        "verified_answers": ordered_answers, "incomplete": false, "next_order": next_order,
    });
    assert_eq!(
        serde_json::to_value(history.retained_context())?,
        expected.clone()
    );
    assert_eq!(
        serde_json::to_value(compact_and_assert_answers(&test, &thread, &answers).await?)?,
        expected
    );
    let compacted = thread.conversation_history_snapshot().await;
    assert!(!compacted.items().any(|item| {
        item.id()
            .is_some_and(|id| id.as_str() == "ordinary-question")
    }));
    for message in &user_messages {
        assert_eq!(
            compacted
                .items()
                .any(|item| item.id().map(ResponseItemId::as_str) == message["message_id"].as_str()),
            true,
            "compaction preserves original user-message identity"
        );
    }
    let thread = resume(&test, &thread).await?;
    assert_eq!(
        serde_json::to_value(
            thread
                .conversation_history_snapshot()
                .await
                .retained_context()
        )?,
        expected.clone()
    );
    thread.shutdown_and_wait().await?;
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 4);
    assert!(requests[3].has_message_with_input_texts("user", |texts| texts == [STEER]));
    Ok(())
}

#[derive(Clone, Copy)]
enum LegacyInstructionSource {
    GuardianHistory,
    ModelWindow,
    Missing,
}

#[test_case(ThreadHistoryMode::Legacy, LegacyInstructionSource::GuardianHistory; "legacy backup")]
#[test_case(ThreadHistoryMode::Paginated, LegacyInstructionSource::GuardianHistory; "paginated backup")]
#[test_case(ThreadHistoryMode::Legacy, LegacyInstructionSource::ModelWindow; "legacy window")]
#[test_case(ThreadHistoryMode::Paginated, LegacyInstructionSource::ModelWindow; "paginated window")]
#[test_case(ThreadHistoryMode::Legacy, LegacyInstructionSource::Missing; "legacy missing source")]
#[test_case(ThreadHistoryMode::Paginated, LegacyInstructionSource::Missing; "paginated missing source")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_checkpoint_recovers_root_excerpt_before_discarding_backup(
    history_mode: ThreadHistoryMode,
    source_location: LegacyInstructionSource,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = test_codex()
        .with_history_mode(history_mode)
        .with_config(|config| {
            config.experimental_thread_store = ThreadStoreConfig::Local;

            config
                .features
                .disable(Feature::TokenBudget)
                .expect("use local compaction");
            config.model_provider.name = "Local compaction test provider".to_owned();
        })
        .build_with_auto_env(&server)
        .await?;
    mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_completed("initial-turn")]),
            sse(vec![
                ev_assistant_message("summary", "Compacted context."),
                ev_completed("compact"),
            ]),
            sse(vec![
                ev_assistant_message("summary-again", "Compacted context."),
                ev_completed("compact-again"),
            ]),
        ],
    )
    .await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: format!(
                "Never publish publicly.\n{}\nNever upload logs.",
                "Project detail. ".repeat(2_000)
            ),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let snapshot = test.codex.conversation_history_snapshot().await;
    let source = snapshot
        .items()
        .find(|item| matches!(item, ResponseItem::Message { role, .. } if role == "user"))
        .cloned()
        .context("original instruction")?;
    let mut expected = compact_and_assert_answers(&test, &test.codex, &[]).await?;
    let mut checkpoint = load_context(&test, &test.codex)
        .await?
        .into_iter()
        .rev()
        .find_map(|item| match item {
            RolloutItem::Compacted(checkpoint) => Some(checkpoint),
            _ => None,
        })
        .context("compaction checkpoint")?;
    // Pre-cutover checkpoints cleared retained text over 16 KiB and kept its raw
    // source in the Guardian backup. Persist that old wire format for real replay.
    let mut legacy = serde_json::to_value(&expected)?;
    legacy["user_messages"][0]["text"] = json!("");
    let legacy: RetainedContext = serde_json::from_value(legacy)?;
    checkpoint.retained_context = Some(legacy.clone());
    let window = checkpoint
        .replacement_history
        .as_mut()
        .context("compacted model window")?;
    window.retain(|item| item.id() != source.id());
    match source_location {
        LegacyInstructionSource::GuardianHistory => {
            // Compaction can leave a shorter copy with the same ID in the model window.
            let mut shortened = source.clone();
            let ResponseItem::Message { content, .. } = &mut shortened else {
                unreachable!("original instruction is a user message");
            };
            *content = vec![ContentItem::InputText {
                text: "Never publish publicly.".to_owned(),
            }];
            window.push(shortened.into());
            checkpoint.guardian_history = Some(GuardianHistoryCheckpoint(vec![source]));
        }
        LegacyInstructionSource::ModelWindow => window.push(source.into()),
        LegacyInstructionSource::Missing => expected = legacy,
    }
    test.codex
        .append_rollout_items(&[RolloutItem::Compacted(checkpoint)])
        .await?;
    let resumed = resume(&test, &test.codex).await?;
    assert_eq!(
        resumed
            .conversation_history_snapshot()
            .await
            .retained_context(),
        Some(&expected)
    );
    // The excerpt must survive another compaction and resume after the backup is gone.
    assert_eq!(
        compact_and_assert_answers(&test, &resumed, &[]).await?,
        expected
    );
    let resumed = resume(&test, &resumed).await?;
    assert_eq!(
        resumed
            .conversation_history_snapshot()
            .await
            .retained_context(),
        Some(&expected)
    );
    resumed.shutdown_and_wait().await?;
    Ok(())
}

// Cover live copied history and a truncated referenced checkpoint. Parent-answer
// omission is already exercised by retained_answers_cross_real_session_boundaries.
#[test_case(ThreadHistoryMode::Legacy; "copied worker history")]
#[test_case(ThreadHistoryMode::Paginated; "referenced truncated checkpoint")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn standalone_fork_retains_inherited_user_instructions(
    history_mode: ThreadHistoryMode,
) -> Result<()> {
    let compacted = history_mode == ThreadHistoryMode::Paginated;
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mut test = test_codex()
        .with_history_mode(history_mode)
        .with_config(|config| {
            config.experimental_thread_store = ThreadStoreConfig::Local;
            config
                .features
                .disable(Feature::TokenBudget)
                .expect("use local compaction for worker checkpoint");
            config.model_provider.name = "Local compaction test provider".to_owned();
            for feature in [
                Feature::GuardianApproval,
                Feature::Collab,
                Feature::MultiAgentV2,
                Feature::DefaultModeRequestUserInput,
            ] {
                config
                    .features
                    .enable(feature)
                    .expect("enable test feature");
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let instruction = if compacted {
        format!(
            "You may publish the release. {} Delegate an inspection.",
            "Original project detail. ".repeat(500)
        )
    } else {
        "You may publish the release. Delegate an inspection.".to_owned()
    };
    let mut created = test.thread_manager.subscribe_thread_created();
    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_function_call_with_namespace(
                    "spawn", "collaboration", "spawn_agent",
                    &json!({"task_name": "worker", "message": "Inspect the project.", "fork_turns": "all"}).to_string(),
                ),
                ev_completed("spawn-response"),
            ]),
            sse(vec![ev_completed("root-complete")]),
            sse(vec![ev_completed("worker-complete")]),
        ],
    ).await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: instruction.clone(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let worker = test
        .thread_manager
        .get_thread(created.recv().await?)
        .await?;
    wait_for_event(&worker, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    test.codex.shutdown_and_wait().await?;
    let local_instruction = if compacted {
        // Leave less than Guardian's 900-token budget for the inherited source
        // within local compaction's 20K user-message budget.
        format!("Ask me before publishing. {}", "x".repeat(78_000))
    } else {
        "Ask me before publishing.".to_owned()
    };
    mount_sse_sequence(&server, vec![sse(vec![ev_completed("local-instruction")])]).await;
    worker
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: local_instruction.clone(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&worker, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    if compacted {
        // Local compaction keeps the inherited source message in replacement history.
        mount_sse_sequence(
            &server,
            vec![sse(vec![
                ev_assistant_message("worker-summary", "Inspected the project."),
                ev_completed("worker-compacted"),
            ])],
        )
        .await;
        compact_and_assert_answers(&test, &worker, &[]).await?;
    }
    let worker_context = worker.conversation_history_snapshot().await;
    let inherited_text = worker_context
        .items()
        .find_map(|item| match item {
            ResponseItem::Message { role, content, .. } if role == "user" => {
                let [ContentItem::InputText { text }] = content.as_slice() else {
                    return None;
                };
                text.starts_with("You may publish the release.")
                    .then(|| text.clone())
            }
            _ => None,
        })
        .context("inherited source in worker history")?;
    if compacted {
        assert_ne!(inherited_text, instruction);
        assert!(inherited_text.contains("tokens truncated"));
        assert!(inherited_text.len() < 900 * 4);
    }
    // The adopted root later compacts through the mechanical path, which can drop originals.
    test.config
        .features
        .enable(Feature::TokenBudget)
        .expect("use mechanical root compaction");
    let fork = if history_mode == ThreadHistoryMode::Paginated {
        let prepared = test
            .thread_store
            .prepare_fork(PrepareForkParams {
                thread_id: worker.startup_metadata().thread_id,
                boundary: ForkBoundary::Latest,
            })
            .await?;
        test.thread_manager
            .fork_prepared_thread(
                codex_core::StartThreadOptions::new(test.config.clone()),
                prepared,
            )
            .await?
    } else {
        test.thread_manager
            .fork_thread_from_history(
                ForkSnapshot::Interrupted,
                codex_core::StartThreadOptions::new(test.config.clone()),
                InitialHistory::Resumed(ResumedHistory {
                    conversation_id: worker.startup_metadata().thread_id,
                    history: Arc::new(load_context(&test, &worker).await?),
                    rollout_path: None,
                }),
            )
            .await?
    };
    assert!(fork.thread.guardian_root_snapshot().await.is_none());
    let history = fork.thread.conversation_history_snapshot().await;
    let retained = history.retained_context().context("root context")?;
    let answers = codex_guardian_context::render_verified_answers(retained);
    assert_eq!(
        (answers.fragments, answers.complete),
        (
            vec![codex_guardian_context::GuardianRootMessage::IncompleteVerifiedAnswers.render()],
            false
        ),
    );
    assert_eq!(
        retained
            .ordered_entries()
            .filter_map(|(_, entry)| match entry {
                codex_history::RetainedContextEntry::UserMessage(message) =>
                    Some((message.text.clone(), message.complete)),
                codex_history::RetainedContextEntry::AssistantMessage(_) => None,
                codex_history::RetainedContextEntry::VerifiedAnswer(_) =>
                    panic!("no answers before root adoption"),
            })
            .collect::<Vec<_>>(),
        vec![
            (inherited_text, !compacted),
            (
                codex_guardian_context::truncate_text(&local_instruction, /*max_tokens*/ 900),
                !compacted,
            ),
        ],
    );
    let after = record_answer(&fork.thread, &server, "root-action", "Do not publish.").await?;
    let expected = fork
        .thread
        .conversation_history_snapshot()
        .await
        .retained_context()
        .context("root after local answer")?
        .clone();
    assert!(!expected.verified_answers_complete());
    let root = fork.thread;
    compact_and_assert_answers(&test, &root, &[after]).await?;
    let root = resume(&test, &root).await?;
    assert_eq!(
        root.conversation_history_snapshot()
            .await
            .retained_context(),
        Some(&expected)
    );
    root.shutdown_and_wait().await?;
    worker.shutdown_and_wait().await?;
    Ok(())
}

#[test_case(false; "without checkpoint")]
#[test_case(true; "after checkpoint")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forked_parent_instructions_do_not_become_local_authorization(
    compact_parent: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    const PARENT_GRANT: &str = "You may publish the private release. Delegate its inspection.";
    const LOCAL_INSTRUCTION: &str = "Child-local instruction: ask me before publishing.";
    let server = start_mock_server().await;
    let child_gate = Arc::new(ForkTestLifecycle::default());
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.tool_lifecycle_contributor(child_gate.clone());
    extensions.thread_lifecycle_contributor(child_gate.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_history_mode(ThreadHistoryMode::Legacy)
        .with_config(move |config| {
            config.experimental_thread_store = ThreadStoreConfig::Local;
            config.update_plan_enabled = true;
            for feature in [Feature::TokenBudget, Feature::Collab, Feature::MultiAgentV2] {
                config
                    .features
                    .enable(feature)
                    .expect("enable test feature");
            }
        })
        .build_with_auto_env(&server)
        .await?;
    mount_sse_sequence(&server, vec![sse(vec![ev_completed("initial")])]).await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Inspect the project.".to_owned(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    wait_for_thread_idle(&test.codex).await;
    if compact_parent {
        test.codex.submit(Op::Compact).await?;
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        wait_for_thread_idle(&test.codex).await;
    }

    let mut created = test.thread_manager.subscribe_thread_created();
    let parent_id = test.session_configured.thread_id.to_string();
    mount_sse_once_match(
        &server,
        header("thread-id", parent_id.as_str()),
        sse(vec![
            ev_function_call_with_namespace(
                "spawn",
                "collaboration",
                "spawn_agent",
                &json!({
                    "task_name": "worker",
                    "message": "Inspect without publishing.",
                    "fork_turns": "all",
                })
                .to_string(),
            ),
            ev_completed("spawn-response"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        header("thread-id", parent_id.as_str()),
        sse(vec![ev_completed("parent-fork-completion")]),
    )
    .await;
    // Pause the child while checking inherited authorization. Its completion notification
    // must not add a parent message or consume a parent mock response.
    let child_requests = mount_sse_once_match(
        &server,
        move |request: &wiremock::Request| {
            request
                .headers
                .get("thread-id")
                .and_then(|value| value.to_str().ok())
                .is_some_and(|thread_id| thread_id != parent_id)
        },
        sse(vec![
            ev_function_call(
                "child-pause",
                "update_plan",
                &json!({"plan": [{"step": "Inspect the project", "status": "in_progress"}]})
                    .to_string(),
            ),
            ev_completed("child-paused"),
        ]),
    )
    .await;
    // This grant is either in uncompacted history or in the checkpoint's replay suffix.
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: PARENT_GRANT.to_owned(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let child = test.thread_manager.get_thread(created.try_recv()?).await?;
    tokio::time::timeout(Duration::from_secs(30), child_gate.entered.notified())
        .await
        .context("child did not reach the paused tool call")?;
    let child_id = child.startup_metadata().thread_id.to_string();
    let requests = child_requests.requests();
    let child_request = requests
        .iter()
        .find(|request| request.body_json()["client_metadata"]["thread_id"] == child_id)
        .context("child model request")?;
    assert!(serde_json::to_string(&child_request.input())?.contains(PARENT_GRANT));
    let history = child.conversation_history_snapshot().await;
    assert_eq!(
        history.retained_context().map(|context| (
            context.ordered_entries().count(),
            context.user_messages_complete()
        )),
        Some((0, true)),
        "inherited conversation must not populate child-local authorization",
    );
    let root_snapshot = child.guardian_root_snapshot().await.context("live root")?;
    assert!(
        root_snapshot
            .messages
            .contains(&codex_core::GuardianRootMessage::User(
                PARENT_GRANT.to_owned()
            ))
    );

    let goal = "Inspect only; do not publish.";
    test.codex
        .record_user_goal_update(codex_core::context::UserGoalUpdate::Set {
            objective: Some(goal.to_owned()),
            status: None,
        })
        .await?;
    let snapshot = child
        .guardian_root_snapshot()
        .await
        .context("root after goal")?;
    assert!(snapshot.messages.iter().any(|message| {
        matches!(message, codex_core::GuardianRootMessage::User(text) if text.contains(goal))
    }));

    mount_sse_once_match(
        &server,
        header("thread-id", child_id.as_str()),
        sse(vec![ev_completed("child-fork-completion")]),
    )
    .await;
    child_gate.release.notify_one();
    wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    mount_sse_once_match(
        &server,
        header("thread-id", child_id.as_str()),
        sse(vec![ev_completed("local-instruction")]),
    )
    .await;
    child
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: LOCAL_INSTRUCTION.to_owned(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    let history = child.conversation_history_snapshot().await;
    let expected = history.retained_context().cloned();
    assert_eq!(
        expected.as_ref().map(|context| context
            .ordered_entries()
            .filter_map(|(_, entry)| match entry {
                codex_history::RetainedContextEntry::UserMessage(message) =>
                    Some(message.text.as_str()),
                codex_history::RetainedContextEntry::AssistantMessage(_) => None,
                codex_history::RetainedContextEntry::VerifiedAnswer(_) =>
                    panic!("unexpected answer"),
            })
            .collect::<Vec<_>>()),
        Some(vec![LOCAL_INSTRUCTION]),
        "genuine child-local instructions must still be captured",
    );
    let child = resume(&test, &child).await?;
    assert_eq!(
        child
            .conversation_history_snapshot()
            .await
            .retained_context(),
        expected.as_ref()
    );
    child.submit(Op::Compact).await?;
    wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    let child = resume(&test, &child).await?;
    assert_eq!(
        child
            .conversation_history_snapshot()
            .await
            .retained_context(),
        expected.as_ref()
    );
    child.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[test_case(ThreadHistoryMode::Legacy; "legacy child fork")]
#[test_case(ThreadHistoryMode::Paginated; "paginated child fork")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retained_answers_cross_real_session_boundaries(
    history_mode: ThreadHistoryMode,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = test_codex()
        .with_history_mode(history_mode)
        .with_config(|config| {
            config.experimental_thread_store = ThreadStoreConfig::Local;
            for feature in [
                Feature::TokenBudget,
                Feature::DefaultModeRequestUserInput,
                Feature::Collab,
                Feature::MultiAgentV2,
            ] {
                config
                    .features
                    .enable(feature)
                    .expect("enable test feature");
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let before = record_answer(
        &test.codex,
        &server,
        "before-compact",
        "Only publish privately.",
    )
    .await?;
    let thread = resume(&test, &test.codex).await?;
    compact_and_assert_answers(&test, &thread, std::slice::from_ref(&before)).await?;

    let after = record_answer(
        &thread,
        &server,
        "after-compact",
        "Do not publish after all.",
    )
    .await?;
    thread.flush_rollout().await?;
    let saved = load_context(&test, &thread).await?;
    let checkpoint_index = saved
        .iter()
        .rposition(|item| matches!(item, RolloutItem::Compacted(_)))
        .context("real compaction persisted a checkpoint")?;
    let RolloutItem::Compacted(checkpoint) = &saved[checkpoint_index] else {
        unreachable!()
    };
    assert_eq!(
        checkpoint
            .retained_context
            .as_ref()
            .context("retained checkpoint")?
            .verified_answers()
            .cloned()
            .collect::<Vec<_>>(),
        vec![before.clone()],
    );
    let suffix = &saved[checkpoint_index + 1..];
    let mut events = suffix
        .iter()
        .filter_map(|item| match item {
            RolloutItem::RetainedContext(event) => Some(event.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    events.dedup();
    assert_eq!(
        events,
        vec![RetainedContextEvent::VerifiedAnswer {
            answer: after.clone(),
            acceptance_order: Some(5)
        }]
    );
    let thread = resume(&test, &thread).await?;
    let expected = [before.clone(), after];

    // Full-history delegation must not turn parent-local answers into child-local facts.
    let mut created = test.thread_manager.subscribe_thread_created();
    let arguments = json!({
        "task_name": "worker",
        "message": "Inspect without publishing.",
        "fork_turns": "all",
    });
    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_function_call_with_namespace(
                    "spawn",
                    "collaboration",
                    "spawn_agent",
                    &arguments.to_string(),
                ),
                ev_completed("spawn-response"),
            ]),
            sse(vec![ev_completed("first-fork-completion")]),
            sse(vec![ev_completed("second-fork-completion")]),
        ],
    )
    .await;
    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Delegate an inspection.".to_owned(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    let child = test.thread_manager.get_thread(created.try_recv()?).await?;
    wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    child.flush_rollout().await?;
    let child_history = load_context(&test, &child).await?;
    assert!(
        serde_json::to_string(&child_history)?.contains("Delegate an inspection."),
        "the child must actually inherit parent conversation context"
    );
    compact_and_assert_answers(&test, &child, &[]).await?;
    let child = resume(&test, &child).await?;
    compact_and_assert_answers(&test, &child, &[]).await?;
    child.shutdown_and_wait().await?;
    compact_and_assert_answers(&test, &thread, &expected).await?;

    thread.shutdown_and_wait().await?;
    Ok(())
}
