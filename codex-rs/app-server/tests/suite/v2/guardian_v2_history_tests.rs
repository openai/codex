//! Exercises both reviewers' evidence delivery through real compaction and resume.

use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use app_test_support::write_models_cache_with_models;
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::header;
use axum::routing::get;
use codex_app_server_protocol::ApprovalsReviewer;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::GuardianApprovalReview;
use codex_app_server_protocol::GuardianApprovalReviewStatus;
use codex_app_server_protocol::ItemGuardianApprovalReviewCompletedNotification;
use codex_app_server_protocol::ThreadCompactStartParams;
use codex_app_server_protocol::ThreadCompactStartResponse;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_config::types::AuthCredentialsStoreMode;
use codex_features::Feature;
use codex_rollout::RolloutItem;
use core_test_support::load_default_config_for_test;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use test_case::test_case;
use tokio::net::TcpListener;
use tokio::time::timeout;

use super::MODEL;
use super::MockResponsesState;
use super::TEST_SERVER_NAME;
use super::TEST_TOOL_NAME;
use super::TIMEOUT;
use super::USER_INPUT_RESTRICTION;
use super::luna_response;
use super::luna_websocket;
use super::start_mcp_server;
use super::submit_user_input_response;
use super::user_input_request_events;
use super::wait_for_luna_request;

#[derive(Clone, Copy)]
enum EvidenceSize {
    Normal,
    OversizedAnswer,
    OversizedInstruction,
}

#[derive(Clone, Copy)]
enum ContextPath {
    Legacy,
    ThreadOwned,
}

#[derive(Clone, Copy)]
enum CheckpointReuse {
    Enabled,
    Disabled,
}

#[derive(Clone, Copy)]
enum ReviewCheckpoint {
    Valid,
    EmptyContent,
    IncompatibleReviewer,
    UnknownReviewer,
    EmptyReviewerHash,
}

#[test_case(ContextPath::ThreadOwned, CheckpointReuse::Enabled, Some("matching"), Some("different"), 0, EvidenceSize::Normal, ReviewCheckpoint::EmptyContent; "empty checkpoint fails closed")]
#[test_case(ContextPath::ThreadOwned, CheckpointReuse::Enabled, Some("matching"), Some("different"), 0, EvidenceSize::Normal, ReviewCheckpoint::IncompatibleReviewer; "incompatible sync reviewer fails closed")]
#[test_case(ContextPath::ThreadOwned, CheckpointReuse::Enabled, Some("matching"), Some("different"), 0, EvidenceSize::Normal, ReviewCheckpoint::UnknownReviewer; "unknown sync compatibility fails closed")]
#[test_case(ContextPath::ThreadOwned, CheckpointReuse::Enabled, Some("matching"), Some("different"), 0, EvidenceSize::Normal, ReviewCheckpoint::EmptyReviewerHash; "empty sync compatibility fails closed")]
#[test_case(ContextPath::Legacy, CheckpointReuse::Enabled, Some("matching"), Some("different"), 0, EvidenceSize::Normal, ReviewCheckpoint::IncompatibleReviewer; "legacy sync compatibility is unchanged")]
#[test_case(ContextPath::ThreadOwned, CheckpointReuse::Disabled, Some("matching"), Some("matching"), 0, EvidenceSize::Normal, ReviewCheckpoint::Valid; "disabled Luna reuse requires sync")]
#[test_case(ContextPath::Legacy, CheckpointReuse::Disabled, Some("matching"), Some("matching"), 0, EvidenceSize::Normal, ReviewCheckpoint::Valid; "legacy disabled Luna reuse still samples")]
#[test_case(ContextPath::ThreadOwned, CheckpointReuse::Enabled, Some("matching"), Some("matching"), 0, EvidenceSize::OversizedInstruction, ReviewCheckpoint::Valid; "instruction budget preserves fresh low score")]
#[test_case(ContextPath::ThreadOwned, CheckpointReuse::Enabled, Some("matching"), Some("matching"), 0, EvidenceSize::Normal, ReviewCheckpoint::Valid; "compatible checkpoint")]
#[test_case(ContextPath::ThreadOwned, CheckpointReuse::Enabled, Some("matching"), Some("different"), 0, EvidenceSize::Normal, ReviewCheckpoint::Valid; "incompatible checkpoint")]
#[test_case(ContextPath::ThreadOwned, CheckpointReuse::Enabled, Some("matching"), None, 0, EvidenceSize::Normal, ReviewCheckpoint::Valid; "unknown Luna compatibility")]
#[test_case(ContextPath::ThreadOwned, CheckpointReuse::Enabled, Some("matching"), Some(""), 0, EvidenceSize::Normal, ReviewCheckpoint::Valid; "empty Luna compatibility")]
#[test_case(ContextPath::ThreadOwned, CheckpointReuse::Enabled, None, Some("matching"), 0, EvidenceSize::Normal, ReviewCheckpoint::Valid; "unknown producer remains unknown after model switch")]
#[test_case(ContextPath::ThreadOwned, CheckpointReuse::Enabled, Some("matching"), Some("matching"), 140, EvidenceSize::Normal, ReviewCheckpoint::Valid; "source call evicted")]
#[test_case(ContextPath::ThreadOwned, CheckpointReuse::Enabled, Some("matching"), Some("matching"), 0, EvidenceSize::OversizedAnswer, ReviewCheckpoint::Valid; "incomplete answers reject fresh low score")]
#[test_case(ContextPath::Legacy, CheckpointReuse::Enabled, Some("matching"), Some("matching"), 0, EvidenceSize::Normal, ReviewCheckpoint::Valid; "legacy answers remain runtime only")]
#[test_case(ContextPath::Legacy, CheckpointReuse::Enabled, Some("matching"), Some("different"), 0, EvidenceSize::Normal, ReviewCheckpoint::Valid; "legacy incompatible checkpoint still samples")]
#[test_case(ContextPath::Legacy, CheckpointReuse::Enabled, Some("matching"), None, 0, EvidenceSize::Normal, ReviewCheckpoint::Valid; "legacy unknown Luna compatibility still samples")]
#[test_case(ContextPath::Legacy, CheckpointReuse::Enabled, Some("matching"), Some("matching"), 140, EvidenceSize::Normal, ReviewCheckpoint::Valid; "legacy source call evicted")]
#[test_case(ContextPath::Legacy, CheckpointReuse::Enabled, Some("matching"), Some("matching"), 0, EvidenceSize::OversizedAnswer, ReviewCheckpoint::Valid; "legacy answer truncation")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardians_retain_evidence_after_compaction_and_resume(
    context_path: ContextPath,
    checkpoint_reuse: CheckpointReuse,
    parent_hash: Option<&str>,
    luna_hash: Option<&str>,
    tool_traffic: usize,
    evidence_size: EvidenceSize,
    review_checkpoint: ReviewCheckpoint,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    const RESTRICTION: &str = "Only publish to a private repository.";
    const EVIDENCE: &str = "repository is private";
    const SUMMARY: &str = "Repository inspection was summarized.";
    let reuse_parent_compaction = matches!(checkpoint_reuse, CheckpointReuse::Enabled);
    let compatible =
        reuse_parent_compaction && parent_hash == Some("matching") && luna_hash == parent_hash;
    let requires_sync = matches!(context_path, ContextPath::ThreadOwned) && !compatible;
    let oversized_instruction = matches!(evidence_size, EvidenceSize::OversizedInstruction);
    let reviewer_hash = match review_checkpoint {
        ReviewCheckpoint::IncompatibleReviewer => Some("different-reviewer"),
        ReviewCheckpoint::UnknownReviewer => None,
        ReviewCheckpoint::EmptyReviewerHash => Some(""),
        ReviewCheckpoint::Valid | ReviewCheckpoint::EmptyContent => Some("matching"),
    };
    let reject_sync_checkpoint = matches!(context_path, ContextPath::ThreadOwned)
        && (!matches!(review_checkpoint, ReviewCheckpoint::Valid)
            || parent_hash != Some("matching"));
    let answer = match evidence_size {
        EvidenceSize::Normal | EvidenceSize::OversizedInstruction => {
            USER_INPUT_RESTRICTION.to_owned()
        }
        EvidenceSize::OversizedAnswer => "Never publish this repository publicly. ".repeat(200),
    };
    let restriction = if oversized_instruction {
        RESTRICTION.repeat(150)
    } else {
        RESTRICTION.to_owned()
    };
    let expected_output = format!("\"echoed\":\"{EVIDENCE}\"");
    let parent_requests = Arc::new(Mutex::new(Vec::<Value>::new()));
    let review_requests = Arc::new(Mutex::new(Vec::<Value>::new()));
    let compact_requests = Arc::new(Mutex::new(Vec::<Value>::new()));
    let mut checkpoint = json!({
        "type": "compaction",
        "id": "cmp_repository",
        "encrypted_content": "encrypted summary"
    });
    match review_checkpoint {
        ReviewCheckpoint::EmptyContent => checkpoint["encrypted_content"] = json!(""),
        ReviewCheckpoint::Valid
        | ReviewCheckpoint::IncompatibleReviewer
        | ReviewCheckpoint::UnknownReviewer
        | ReviewCheckpoint::EmptyReviewerHash => {}
    }
    let rejects_incomplete_score = matches!(
        (context_path, evidence_size),
        (ContextPath::ThreadOwned, EvidenceSize::OversizedAnswer)
    );
    let classifier = Arc::new(MockResponsesState {
        luna_score: if rejects_incomplete_score || oversized_instruction || requires_sync {
            0.0
        } else {
            1.0
        },
        ..Default::default()
    });
    let router = Router::new()
        .route(
            "/v1/responses",
            get(luna_websocket).post({
                let parent_requests = Arc::clone(&parent_requests);
                let review_requests = Arc::clone(&review_requests);
                let compact_requests = Arc::clone(&compact_requests);
                let checkpoint = checkpoint.clone();
                move |State(classifier): State<Arc<MockResponsesState>>,
                      Json(request): Json<Value>| {
                    let parent_requests = Arc::clone(&parent_requests);
                    let review_requests = Arc::clone(&review_requests);
                    let compact_requests = Arc::clone(&compact_requests);
                    let checkpoint = checkpoint.clone();
                    async move {
                        let events = if request["input"].as_array().is_some_and(|input| {
                            input
                                .iter()
                                .any(|item| item["type"] == "compaction_trigger")
                        }) {
                            compact_requests
                                .lock()
                                .expect("request log lock")
                                .push(request);
                            vec![
                                responses::ev_assistant_message("summary", SUMMARY),
                                json!({
                                    "type": "response.output_item.done",
                                    "item": checkpoint,
                                }),
                                responses::ev_completed("compact"),
                            ]
                        } else if request["model"] == "gpt-5.6-luna" {
                            luna_response(&classifier, request).await
                        } else if request["client_metadata"]["x-openai-subagent"] == "guardian" {
                            review_requests
                                .lock()
                                .expect("request log lock")
                                .push(request);
                            vec![
                                responses::ev_assistant_message("review", r#"{"outcome":"allow"}"#),
                                responses::ev_completed("review"),
                            ]
                        } else {
                            let mut requests = parent_requests.lock().expect("request log lock");
                            let step = requests.len();
                            requests.push(request);
                            if step == 0 {
                                user_input_request_events()
                            } else if (step - 1).is_multiple_of(/*rhs*/ 2) {
                                let message = if step == 1 {
                                    EVIDENCE
                                } else {
                                    "current inspection"
                                };
                                vec![
                                    responses::ev_function_call_with_namespace(
                                        &format!("inspect-{step}"),
                                        &format!("mcp__{TEST_SERVER_NAME}"),
                                        TEST_TOOL_NAME,
                                        &json!({"message": message}).to_string(),
                                    ),
                                    responses::ev_completed(&format!("inspect-{step}")),
                                ]
                            } else {
                                let mut events = Vec::new();
                                if step == 2 {
                                    for index in 0..tool_traffic {
                                        events.push(responses::ev_assistant_message(
                                            &format!("traffic-{index}"),
                                            "Ordinary intermediate evidence.",
                                        ));
                                    }
                                }
                                events.push(responses::ev_completed(&format!("done-{step}")));
                                events
                            }
                        };
                        (
                            [(header::CONTENT_TYPE, "text/event-stream")],
                            responses::sse(events),
                        )
                    }
                }
            }),
        )
        .with_state(Arc::clone(&classifier));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let responses_url = format!("http://{}", listener.local_addr()?);
    let responses_server = tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("serve mock responses");
    });
    let (mcp_url, mcp_server) = start_mcp_server(/*sensitive_action*/ None).await?;
    let codex_home = TempDir::new()?;
    let thread_context_enabled = match context_path {
        ContextPath::Legacy => false,
        ContextPath::ThreadOwned => true,
    };
    let mut mock_config = MockResponsesConfig::new(&responses_url)
        .with_provider_name("OpenAI")
        .with_provider_config("requires_openai_auth = true\nsupports_websockets = false")
        .with_root_config("approvals_reviewer = \"auto_review\"\nmodel_auto_compact_token_limit = 1000000")
        .enable_feature(Feature::DefaultModeRequestUserInput)
        .enable_feature(Feature::GuardianApproval)
        .enable_feature(Feature::GuardianReuseParentCompaction)
        .disable_feature(Feature::EnableRequestCompression)
        .disable_feature(Feature::TokenBudget)
        .with_extra_config(&format!(
            "[mcp_servers.{TEST_SERVER_NAME}]\nurl = \"{mcp_url}/mcp\"\ndefault_tools_approval_mode = \"prompt\"\n\n[features.guardianv2]\nenabled = true\nthread_context = {thread_context_enabled}\npersist_scores = true\nreuse_parent_compaction = {reuse_parent_compaction}\n\n[features.guardianv2.review_scope]\ncomputer_use_only = false"
        ));
    if matches!(context_path, ContextPath::ThreadOwned) {
        mock_config = mock_config.disable_feature(Feature::GuardianReuseParentCompaction);
    }
    mock_config.write(codex_home.path())?;
    let config = load_default_config_for_test(&codex_home).await;
    let models = [
        (MODEL, parent_hash),
        ("gpt-5.6-luna", luna_hash),
        ("codex-auto-review", reviewer_hash),
        ("resumed-parent", Some("matching")),
    ]
    .into_iter()
    .map(|(model, hash)| {
        let mut info = codex_core::test_support::construct_model_info_offline(model, &config);
        info.comp_hash = hash.map(str::to_owned);
        if model == MODEL || model == "resumed-parent" {
            info.auto_review_model_override = Some("codex-auto-review".to_owned());
        }
        info
    })
    .collect();
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("access-chatgpt").plan_type("pro"),
        AuthCredentialsStoreMode::File,
    )?;
    write_models_cache_with_models(codex_home.path(), models).await?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized_with_timeout(TIMEOUT)
        .await?;
    let thread = app_server
        .start_thread(ThreadStartParams {
            approval_policy: Some(AskForApproval::OnRequest),
            approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
            history_mode: Some(ThreadHistoryMode::Legacy),
            ..Default::default()
        })
        .await?
        .thread;
    let thread_id = thread.id.clone();

    for (index, prompt) in [
        restriction.as_str(),
        "Recheck the repository.",
        "Inspect after resume.",
    ]
    .into_iter()
    .enumerate()
    {
        if index == 2 || (index == 1 && !requires_sync) {
            app_server.shutdown_gracefully().await?;
            app_server = TestAppServer::builder()
                .with_codex_home(codex_home.path())
                .with_env_overrides(&[("OPENAI_API_KEY", None)])
                .build_initialized_with_timeout(TIMEOUT)
                .await?;
            let id = app_server
                .send_thread_resume_request(ThreadResumeParams {
                    thread_id: thread_id.clone(),
                    model: parent_hash.is_none().then(|| "resumed-parent".to_owned()),
                    ..Default::default()
                })
                .await?;
            let _: ThreadResumeResponse = timeout(TIMEOUT, app_server.read_response(id)).await??;
        }
        if index == 1 {
            let id = app_server
                .send_thread_compact_start_request(ThreadCompactStartParams {
                    thread_id: thread_id.clone(),
                })
                .await?;
            let _: ThreadCompactStartResponse =
                timeout(TIMEOUT, app_server.read_response(id)).await??;
            let completed: TurnCompletedNotification =
                timeout(TIMEOUT, app_server.read_notification("turn/completed")).await??;
            assert_eq!(completed.turn.status, TurnStatus::Completed);
            assert_eq!(compact_requests.lock().expect("request log lock").len(), 1);
        }

        let id = app_server
            .send_turn_start_request(TurnStartParams {
                thread_id: thread_id.clone(),
                input: if index == 1 && requires_sync {
                    Vec::new()
                } else {
                    vec![UserInput::Text {
                        text: prompt.to_owned(),
                        text_elements: Vec::new(),
                    }]
                },
                ..Default::default()
            })
            .await?;
        let started: TurnStartResponse = timeout(TIMEOUT, app_server.read_response(id)).await??;
        if index == 0 {
            // Wait for the pre-answer sample before replying; scoring runs asynchronously.
            let before_answer = wait_for_luna_request(&classifier, /*index*/ 0).await?;
            assert!(
                !before_answer
                    .to_string()
                    .contains(">>> TRUSTED USER ANSWERS START")
            );
            submit_user_input_response(
                &mut app_server,
                json!({
                    "browser_authorization": {"answers": [&answer]}
                }),
            )
            .await?;
            classifier.allow_luna.notify_one();
        }
        if reject_sync_checkpoint && (1..=2).contains(&index) {
            let notification = timeout(
                TIMEOUT,
                app_server.read_stream_until_matching_notification(
                    "checkpoint review failure",
                    |notification| {
                        notification.method == "item/autoApprovalReview/completed"
                            && notification
                                .params
                                .as_ref()
                                .is_some_and(|params| params["turnId"] == started.turn.id)
                    },
                ),
            )
            .await??;
            let assessment: ItemGuardianApprovalReviewCompletedNotification =
                serde_json::from_value(notification.params.expect("review completion"))?;
            let reason = match review_checkpoint {
                ReviewCheckpoint::EmptyContent => {
                    "parent compaction checkpoint is unusable for Guardian review"
                }
                ReviewCheckpoint::Valid
                | ReviewCheckpoint::IncompatibleReviewer
                | ReviewCheckpoint::UnknownReviewer
                | ReviewCheckpoint::EmptyReviewerHash => {
                    "parent compaction checkpoint is incompatible with the Guardian review model or its compatibility is unknown"
                }
            };
            assert_eq!(
                assessment.review,
                GuardianApprovalReview {
                    status: GuardianApprovalReviewStatus::Denied,
                    risk_level: None,
                    user_authorization: None,
                    rationale: Some(format!("Automatic approval review failed: {reason}")),
                }
            );
        }
        let completed: TurnCompletedNotification =
            timeout(TIMEOUT, app_server.read_notification("turn/completed")).await??;
        assert_eq!(completed.turn.status, TurnStatus::Completed);
        if reject_sync_checkpoint && (1..=2).contains(&index) {
            // The first review established a cached session before compaction. Neither
            // that session nor a new one may review unusable evidence, including after resume.
            let reviews = review_requests.lock().expect("request log lock");
            assert_eq!(
                reviews.len(),
                1,
                "no sync inference with an unusable checkpoint"
            );
            assert_eq!(reviews[0]["model"], "codex-auto-review");
            let requests = parent_requests.lock().expect("request log lock");
            let output = requests.last().expect("parent tool result")["input"]
                .as_array()
                .expect("parent input")
                .iter()
                .find(|item| {
                    item["type"] == "function_call_output"
                        && item["call_id"] == format!("inspect-{}", 2 * index + 1)
                })
                .expect("declined tool result");
            assert!(
                output.to_string().contains(
                    "This is a review failure, not a determination that the action is unsafe."
                ),
                "tool must not execute: {output}",
            );
            assert_eq!(
                classifier
                    .luna_requests
                    .lock()
                    .expect("classifier request lock")
                    .len(),
                2,
                "no checkpoint-less async fallback",
            );
            if index == 2 {
                break;
            }
            continue;
        }
        if requires_sync && index > 0 {
            let reviews = review_requests.lock().expect("request log lock");
            assert_eq!(
                reviews.len(),
                index + 1,
                "incompatible checkpoints require sync review"
            );
            let review = &reviews[index];
            assert!(
                review["input"]
                    .as_array()
                    .expect("request input array")
                    .contains(&checkpoint)
            );
            let text = review.to_string();
            if index >= 2 {
                assert!(
                    text.contains(prompt),
                    "current user input missing from sync review: {text}"
                );
            }
            assert!(text.contains(USER_INPUT_RESTRICTION));
            assert!(text.contains("TRUSTED USER ANSWERS START"));
            assert!(text.contains("TRANSCRIPT START"));
            assert!(!text.contains("TRANSCRIPT DELTA START"));
            assert_eq!(
                classifier
                    .luna_requests
                    .lock()
                    .expect("classifier request lock")
                    .len(),
                2,
                "no checkpoint-less async request may run after compaction"
            );
            continue;
        }
        let sample_index = if !requires_sync {
            index + 1
        } else if index == 0 {
            1
        } else {
            2
        };
        let request = wait_for_luna_request(&classifier, sample_index).await?;
        let content = request["input"]
            .as_array()
            .expect("request input array")
            .iter()
            .filter(|item| item["role"] == "user")
            .filter_map(|item| item["content"].as_array())
            .flatten()
            .filter_map(|part| part["text"].as_str())
            .collect::<String>();
        let transcript = content
            .split_once(">>> TRANSCRIPT START\n")
            .expect("transcript start")
            .1
            .split_once(">>> TRANSCRIPT END")
            .expect("transcript end")
            .0;
        if index == 0 && oversized_instruction {
            assert!(content.contains("some retained user instructions are unavailable"));
        } else {
            assert!(
                transcript.contains(prompt),
                "current user input missing: {transcript}"
            );
        }

        {
            let parent = parent_requests.lock().expect("request log lock");
            assert_eq!(parent.len(), (index + 1) * 2 + 1);
            let reviews = review_requests.lock().expect("request log lock");
            assert_eq!(reviews.len(), index + 1);
            let review = &reviews[index];
            let sync_input = review["input"].as_array().expect("request input array");
            let async_input = request["input"].as_array().expect("request input array");
            assert_eq!(sync_input.contains(&checkpoint), index > 0);
            assert_eq!(async_input.contains(&checkpoint), index > 0 && compatible);
            let sync_text = sync_input
                .iter()
                .filter(|item| item["role"] == "user")
                .filter_map(|item| item["content"].as_array())
                .flatten()
                .filter_map(|part| part["text"].as_str())
                .collect::<String>();
            assert!(
                sync_text.contains(prompt),
                "current user input missing from sync review: {sync_text}"
            );
            if index == 0 {
                let input = parent[2]["input"].as_array().expect("request input array");
                let output = input
                    .iter()
                    .find(|item| {
                        item["type"] == "function_call_output" && item["call_id"] == "inspect-1"
                    })
                    .expect("the MCP tool must actually execute before compaction");
                let output = output["output"].as_str().expect("tool output text");
                assert!(output.contains(&expected_output), "{output}");
            } else {
                let parent_items = parent[index * 2 + 1]["input"]
                    .as_array()
                    .expect("request input array");
                let parent_input = serde_json::to_string(parent_items)?;
                // V2 keeps bounded user history while replacing old tool output with a checkpoint.
                assert!(parent_input.contains(RESTRICTION));
                assert!(!parent_input.contains(EVIDENCE));
                assert!(!parent_items.iter().any(|item| {
                    item["type"] == "function_call_output" && item["call_id"] == "inspect-1"
                }));
                {
                    assert!(sync_text.contains(RESTRICTION));
                    if matches!(context_path, ContextPath::ThreadOwned) {
                        assert!(
                            !sync_text.contains(&expected_output),
                            "raw tool result must not survive the parent checkpoint"
                        );
                    } else if tool_traffic == 0 {
                        assert!(sync_text.contains(EVIDENCE));
                        assert!(sync_text.contains(&format!("tool {TEST_TOOL_NAME} result:")));
                        assert!(sync_text.contains(&expected_output));
                    }
                    assert!(sync_text.contains(">>> TRANSCRIPT START"));
                    assert!(!sync_text.contains(">>> TRANSCRIPT DELTA START"));
                    // The endpoint receives the original evidence. The returned checkpoint is
                    // opaque: these tests prove delivery, not what a model can recover from it.
                    let compact_requests = compact_requests.lock().expect("request log lock");
                    let compact_input = serde_json::to_string(&compact_requests[0])?;
                    assert!(compact_input.contains(RESTRICTION));
                    let compact_output = compact_requests[0]["input"]
                        .as_array()
                        .expect("compaction input array")
                        .iter()
                        .find(|item| {
                            item["type"] == "function_call_output" && item["call_id"] == "inspect-1"
                        })
                        .expect("compaction must receive the original MCP result");
                    let compact_output =
                        compact_output["output"].as_str().expect("tool output text");
                    assert!(
                        compact_output.contains(&expected_output),
                        "{compact_output}"
                    );
                    assert!(parent_items.contains(&checkpoint));
                    if matches!(context_path, ContextPath::ThreadOwned) {
                        assert!(
                            content.contains(RESTRICTION),
                            "retained instruction must survive"
                        );
                        assert!(
                            !transcript.contains(&expected_output),
                            "raw tool result must not survive the parent checkpoint"
                        );
                        assert!(transcript.contains(RESTRICTION));
                        assert!(!transcript.contains(SUMMARY));
                    } else {
                        assert!(
                            transcript.contains(RESTRICTION),
                            "retained user restriction missing: {transcript}"
                        );
                        if tool_traffic == 0 {
                            assert!(transcript.contains(&format!("tool {TEST_TOOL_NAME} call:")));
                            assert!(transcript.contains(&format!("tool {TEST_TOOL_NAME} result:")));
                            assert!(transcript.contains(&expected_output));
                        } else {
                            assert!(!transcript.contains("tool request_user_input call:"));
                        }
                    }
                }
            }
            for (consumer, text) in [("async", &content), ("sync", &sync_text)] {
                if matches!(context_path, ContextPath::ThreadOwned) || index == 0 {
                    let answers = text
                        .split_once(">>> TRUSTED USER ANSWERS START")
                        .unwrap_or_else(|| {
                            panic!("missing {consumer} answers at step {index}: {text}")
                        })
                        .1;
                    match evidence_size {
                        EvidenceSize::Normal | EvidenceSize::OversizedInstruction => {
                            assert!(answers.contains("assistant: Can I keep using the browser?"));
                            assert!(answers.contains(&format!("user: {USER_INPUT_RESTRICTION}")));
                        }
                        EvidenceSize::OversizedAnswer => match context_path {
                            ContextPath::ThreadOwned => {
                                assert!(
                                    answers.contains("some verified user answers are unavailable")
                                );
                            }
                            ContextPath::Legacy => {
                                assert!(answers.contains("<truncated omitted_approx_tokens="));
                                assert!(
                                    !answers.contains("some verified user answers are unavailable")
                                );
                            }
                        },
                    }
                } else {
                    assert!(!text.contains(">>> TRUSTED USER ANSWERS START"));
                }
            }
        }
        classifier.allow_luna.notify_one();
        if rejects_incomplete_score || oversized_instruction || (index == 0 && requires_sync) {
            // Wait for host publication, not merely a mock response. The next action must
            // see a fresh low score with unchanged authorization, rather than a stale score.
            let path = thread.path.as_ref().expect("legacy rollout path");
            timeout(TIMEOUT, async {
                loop {
                    let persisted = std::fs::read_to_string(path).unwrap_or_default();
                    if persisted
                        .lines()
                        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                        .any(|line| {
                            line["type"] == "security_risk_score"
                                && line["payload"]["call_id"] == "inspect-1"
                                && line["payload"]["scores"]["action_risk"] == 0.0
                        })
                    {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await?;
        }
        if rejects_incomplete_score || oversized_instruction {
            let notice = match evidence_size {
                EvidenceSize::OversizedInstruction => {
                    "some retained user instructions are unavailable"
                }
                EvidenceSize::OversizedAnswer => "some verified user answers are unavailable",
                EvidenceSize::Normal => unreachable!("incomplete evidence fixture"),
            };
            let completed = app_server
                .start_turn_and_wait_for_completion(TurnStartParams {
                    thread_id: thread_id.clone(),
                    input: Vec::new(),
                    ..Default::default()
                })
                .await?;
            assert_eq!(completed.turn.status, TurnStatus::Completed);
            let fresh_sample = wait_for_luna_request(&classifier, /*index*/ 2).await?;
            assert!(fresh_sample.to_string().contains(notice));
            let reviews = review_requests.lock().expect("request log lock");
            assert_eq!(
                reviews.len(),
                1 + usize::from(rejects_incomplete_score),
                "only incomplete verified answers require another sync review"
            );
            assert!(
                reviews
                    .last()
                    .expect("initial sync review")
                    .to_string()
                    .contains(notice)
            );
            classifier.allow_luna.notify_one();
            break;
        }
    }

    app_server.shutdown_gracefully().await?;
    let rollout = std::fs::read_to_string(thread.path.as_ref().expect("saved rollout path"))?;
    if matches!(context_path, ContextPath::ThreadOwned) {
        for line in rollout
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        {
            if line["type"] == "compacted" {
                assert!(line["payload"]["guardian_history"].is_null());
            }
        }
    }
    let items = rollout
        .lines()
        .map(codex_rollout::parse_rollout_line)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    assert_eq!(
        items
            .iter()
            .filter(|line| matches!(line.item, RolloutItem::RetainedContext(_)))
            .count(),
        usize::from(matches!(context_path, ContextPath::ThreadOwned)),
        "only the enabled path may persist a retained-answer event",
    );
    for line in &items {
        if let RolloutItem::Compacted(checkpoint) = &line.item {
            for envelope in checkpoint.replacement_history.iter().flatten() {
                if matches!(
                    envelope.item,
                    codex_protocol::models::ResponseItem::Compaction { .. }
                ) {
                    assert_eq!(
                        envelope
                            .metadata
                            .as_ref()
                            .and_then(|metadata| metadata.compaction_model_hash.as_deref()),
                        matches!(context_path, ContextPath::ThreadOwned)
                            .then_some(parent_hash)
                            .flatten(),
                        "only the enabled path records checkpoint producer provenance",
                    );
                }
            }
        }
    }
    if matches!(context_path, ContextPath::Legacy) {
        for line in &items {
            if let RolloutItem::Compacted(checkpoint) = &line.item {
                assert_eq!(
                    checkpoint
                        .retained_context
                        .as_ref()
                        .expect("retained context checkpoint")
                        .verified_answers()
                        .count(),
                    0,
                    "flag-off compaction must not populate retained answers",
                );
            }
        }
    }
    mcp_server.abort();
    responses_server.abort();
    Ok(())
}
