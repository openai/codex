use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::create_request_user_input_sse_response;
use app_test_support::write_models_cache_with_models;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadForkResponse;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ThreadItemsListParams;
use codex_app_server_protocol::ThreadItemsListResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadRevertParams;
use codex_app_server_protocol::ThreadRevertResponse;
use codex_app_server_protocol::ThreadRevertedNotification;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadTurnsListParams;
use codex_app_server_protocol::ThreadTurnsListResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::MultiAgentVersion;
use codex_rollout::RolloutItem;
use codex_rollout::read_session_meta_line;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::load_default_config_for_test;
use pretty_assertions::assert_eq;
use serde_json::Value;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[test_case::test_case(false; "live_reload")]
#[test_case::test_case(true; "cold_resume")]
#[tokio::test]
async fn thread_revert_preserves_model_selected_multi_agent_version(restart: bool) -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .disable_feature(Feature::MultiAgentV2)
        .write(codex_home.path())?;
    let config = load_default_config_for_test(&codex_home).await;
    let mut model = codex_core::test_support::construct_model_info_offline("mock-model", &config);
    model.multi_agent_version = Some(MultiAgentVersion::V2);
    write_models_cache_with_models(codex_home.path(), vec![model])?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    initialize_experimental(&mut mcp).await?;
    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            history_mode: Some(ThreadHistoryMode::Paginated),
            ..Default::default()
        })
        .await?;
    let completed = mcp
        .start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "First message".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: ThreadRevertResponse = mcp
        .request(|request_id| ClientRequest::ThreadRevert {
            request_id,
            params: ThreadRevertParams {
                thread_id: thread.id.clone(),
                before_turn_id: completed.turn.id,
            },
        })
        .await?;
    if restart {
        // Restart before another turn can persist a replacement TurnContext.
        mcp.shutdown_gracefully().await?;
        mcp = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .build()
            .await?;
        initialize_experimental(&mut mcp).await?;
        let _: ThreadResumeResponse = mcp
            .request(|request_id| ClientRequest::ThreadResume {
                request_id,
                params: ThreadResumeParams {
                    thread_id: thread.id.clone(),
                    exclude_turns: true,
                    ..Default::default()
                },
            })
            .await?;
    }
    mcp.start_turn_and_wait_for_completion(TurnStartParams {
        thread_id: thread.id,
        input: vec![UserInput::Text {
            text: "Edited first message".to_string(),
            text_elements: Vec::new(),
        }],
        ..Default::default()
    })
    .await?;

    let requests = server.received_requests().await.expect("response requests");
    let mut multi_agent_namespaces = Vec::new();
    for request in requests
        .iter()
        .filter(|request| request.url.path().ends_with("/responses"))
    {
        let body = request.body_json::<Value>()?;
        multi_agent_namespaces.push(
            body["tools"]
                .as_array()
                .expect("tools")
                .iter()
                .filter_map(|tool| tool["name"].as_str())
                .filter(|name| matches!(*name, "collaboration" | "multi_agent_v1"))
                .map(str::to_owned)
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(
        multi_agent_namespaces,
        vec![vec!["collaboration"], vec!["collaboration"]]
    );
    Ok(())
}

#[tokio::test]
async fn thread_revert_preserves_fork_cutoff_after_cold_resume() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    let updated_workspace = TempDir::new()?;
    let saved_cwd = AbsolutePathBuf::from_absolute_path(updated_workspace.path().canonicalize()?)?
        .into_path_buf();
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;
    // This fixture checks host-native cwd restoration across fork and revert.
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    initialize_experimental(&mut mcp).await?;
    let ThreadStartResponse { thread: parent, .. } = mcp
        .request(|request_id| ClientRequest::ThreadStart {
            request_id,
            params: ThreadStartParams {
                history_mode: Some(ThreadHistoryMode::Paginated),
                ..Default::default()
            },
        })
        .await?;
    let mut parent_turns = Vec::new();
    for text in ["parent first", "parent second"] {
        let completed = mcp
            .start_turn_and_wait_for_completion(TurnStartParams {
                thread_id: parent.id.clone(),
                cwd: Some(parent.cwd.as_path().to_path_buf()),
                input: vec![UserInput::Text {
                    text: text.to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            })
            .await?;
        parent_turns.push(completed.turn.id);
    }
    let ThreadForkResponse { thread: child, .. } = mcp
        .request(|request_id| ClientRequest::ThreadFork {
            request_id,
            params: ThreadForkParams {
                thread_id: parent.id.clone(),
                cwd: Some(codex_home.path().to_string_lossy().into_owned()),
                ..Default::default()
            },
        })
        .await?;
    let child_meta = read_session_meta_line(child.path.as_ref().expect("child rollout"))
        .await?
        .meta;
    let fork_cutoff = child_meta
        .history_base
        .expect("fork history base")
        .end_ordinal_exclusive;
    assert_eq!(child_meta.forked_from_ordinal_exclusive, Some(fork_cutoff));
    let inherited_revert_cutoff =
        std::fs::read_to_string(parent.path.as_ref().expect("parent rollout"))?
            .lines()
            .map(codex_rollout::parse_rollout_line)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .find_map(|line| match line.item {
                RolloutItem::EventMsg(EventMsg::TurnStarted(turn))
                    if turn.turn_id == parent_turns[1] =>
                {
                    line.ordinal
                }
                _ => None,
            })
            .expect("inherited turn start ordinal");
    let mut child_turns = Vec::new();
    for text in ["child first", "child second"] {
        let completed = mcp
            .start_turn_and_wait_for_completion(TurnStartParams {
                thread_id: child.id.clone(),
                cwd: Some(saved_cwd.clone()),
                input: vec![UserInput::Text {
                    text: text.to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            })
            .await?;
        child_turns.push(completed.turn.id);
    }

    // First revert within the child, then revert into its inherited parent history.
    for (before_turn_id, expected_cutoff) in [
        (child_turns[1].clone(), fork_cutoff),
        (parent_turns[1].clone(), inherited_revert_cutoff),
    ] {
        let ThreadRevertResponse {
            thread: reverted, ..
        } = mcp
            .request(|request_id| ClientRequest::ThreadRevert {
                request_id,
                params: ThreadRevertParams {
                    thread_id: child.id.clone(),
                    before_turn_id,
                },
            })
            .await?;
        let meta = read_session_meta_line(reverted.path.as_ref().expect("reverted rollout"))
            .await?
            .meta;
        assert_eq!(meta.forked_from_ordinal_exclusive, Some(expected_cutoff));
        if expected_cutoff == fork_cutoff {
            assert!(
                meta.history_base
                    .expect("child revert base")
                    .end_ordinal_exclusive
                    > fork_cutoff
            );
        }

        mcp.shutdown_gracefully().await?;
        mcp = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .without_auto_env()
            .build()
            .await?;
        initialize_experimental(&mut mcp).await?;
        let ThreadResumeResponse { cwd, .. } = mcp
            .request(|request_id| ClientRequest::ThreadResume {
                request_id,
                params: ThreadResumeParams {
                    thread_id: child.id.clone(),
                    ..Default::default()
                },
            })
            .await?;
        if expected_cutoff == fork_cutoff {
            assert_eq!(cwd.as_path(), saved_cwd);
        } else {
            // Only parent-owned snapshots remain after reverting into inherited history.
            assert_eq!(cwd.as_path(), child_meta.cwd);
        }
        mcp.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: child.id.clone(),
            input: vec![UserInput::Text {
                text: "continue after revert and cold resume".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
        let requests = server.received_requests().await.expect("response requests");
        let body = requests
            .iter()
            .rev()
            .find(|request| request.url.path().ends_with("/responses"))
            .expect("resumed model request")
            .body_json::<Value>()?;
        let metadata: Value = serde_json::from_str(
            body["client_metadata"]["x-codex-turn-metadata"]
                .as_str()
                .expect("turn metadata"),
        )?;
        assert_eq!(
            (
                metadata["forked_from_thread_id"].as_str(),
                metadata["forked_from_ordinal_exclusive"].as_u64()
            ),
            (Some(parent.id.as_str()), Some(expected_cutoff))
        );
    }
    Ok(())
}

#[tokio::test]
async fn thread_revert_replaces_paginated_history_before_turn() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    initialize_experimental(&mut mcp).await?;

    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            history_mode: Some(ThreadHistoryMode::Paginated),
            ..Default::default()
        })
        .await?;
    let stale_rollout_path = thread.path.clone().expect("thread rollout path");
    let mut turn_ids = Vec::new();
    for text in ["first", "second"] {
        let completed = mcp
            .start_turn_and_wait_for_completion(TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![UserInput::Text {
                    text: text.to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            })
            .await?;
        turn_ids.push(completed.turn.id);
    }

    let ThreadRevertResponse {
        thread: reverted_thread,
        turns_backwards_cursor,
        items_backwards_cursor,
    } = mcp
        .request(|request_id| ClientRequest::ThreadRevert {
            request_id,
            params: ThreadRevertParams {
                thread_id: thread.id.clone(),
                before_turn_id: turn_ids[1].clone(),
            },
        })
        .await?;
    let reverted: ThreadRevertedNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_notification("thread/reverted"),
    )
    .await??;
    assert_eq!(reverted.thread_id, thread.id);

    assert_eq!(reverted_thread.id, thread.id);
    assert!(reverted_thread.turns.is_empty());
    assert!(items_backwards_cursor.is_some());
    assert_eq!(
        turn_ids_from_cursor(
            &mut mcp,
            &thread.id,
            turns_backwards_cursor,
            /*sort_direction*/ None,
        )
        .await?,
        turn_ids[..1]
    );
    let ThreadItemsListResponse {
        data: reverted_items,
        ..
    } = mcp
        .request(|request_id| ClientRequest::ThreadItemsList {
            request_id,
            params: ThreadItemsListParams {
                thread_id: thread.id.clone(),
                turn_id: None,
                cursor: items_backwards_cursor,
                limit: None,
                sort_direction: None,
            },
        })
        .await?;
    assert!(!reverted_items.is_empty());
    assert!(
        reverted_items
            .iter()
            .all(|item| item.turn_id == turn_ids[0])
    );

    mcp.shutdown_gracefully().await?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    initialize_experimental(&mut mcp).await?;
    let stale_resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            path: Some(stale_rollout_path),
            ..Default::default()
        })
        .await?;
    let stale_resume_error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(stale_resume_id)),
    )
    .await??;
    assert!(
        stale_resume_error.error.message.contains("stale path")
            && stale_resume_error
                .error
                .message
                .contains("omit path and resume by thread id"),
        "unexpected resume error: {}",
        stale_resume_error.error.message,
    );
    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            ..Default::default()
        })
        .await?;
    let _: ThreadResumeResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(resume_id)).await??;
    let invalid_revert_id = mcp
        .send_raw_request(
            "thread/revert",
            Some(serde_json::to_value(ThreadRevertParams {
                thread_id: thread.id.clone(),
                before_turn_id: "missing-turn".to_string(),
            })?),
        )
        .await?;
    let invalid_revert_error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(invalid_revert_id)),
    )
    .await??;
    assert_eq!(
        invalid_revert_error.error.message,
        "turn not found: missing-turn"
    );

    let third_turn = mcp
        .start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "third".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let requests = server.received_requests().await.expect("response requests");
    let model_input = requests
        .iter()
        .rev()
        .find(|request| request.url.path().ends_with("/responses"))
        .expect("third turn response request")
        .body_json::<serde_json::Value>()?["input"]
        .clone();
    let model_input = serde_json::to_string(&model_input)?;
    assert!(model_input.contains("first"));
    assert!(!model_input.contains("second"));
    assert!(model_input.contains("third"));
    assert_eq!(
        turn_ids_from_cursor(
            &mut mcp,
            &thread.id,
            /*cursor*/ None,
            Some(SortDirection::Asc),
        )
        .await?,
        vec![turn_ids[0].clone(), third_turn.turn.id]
    );
    Ok(())
}

#[tokio::test]
async fn thread_revert_interrupts_active_turn_and_keeps_thread_loaded() -> Result<()> {
    let home = TempDir::new()?;
    let server = create_mock_responses_server_sequence(vec![
        create_final_assistant_message_sse_response("first")?,
        create_request_user_input_sse_response("call_blocked")?,
        create_final_assistant_message_sse_response("third")?,
    ])
    .await;
    MockResponsesConfig::new(&server.uri()).write(home.path())?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(home.path())
        .build()
        .await?;
    initialize_experimental(&mut mcp).await?;

    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            history_mode: Some(ThreadHistoryMode::Paginated),
            ..Default::default()
        })
        .await?;
    let first_turn = mcp
        .start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "first".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;

    let TurnStartResponse { turn: active_turn } = mcp
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![UserInput::Text {
                    text: "sleep".to_string(),
                    text_elements: Vec::new(),
                }],
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Plan,
                    settings: Settings {
                        model: "mock-model".to_string(),
                        reasoning_effort: Some(ReasoningEffort::Medium),
                        developer_instructions: None,
                    },
                }),
                approval_policy: Some(AskForApproval::Never),
                ..Default::default()
            },
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;

    let ThreadRevertResponse {
        thread: reverted_thread,
        turns_backwards_cursor,
        items_backwards_cursor,
    } = mcp
        .request(|request_id| ClientRequest::ThreadRevert {
            request_id,
            params: ThreadRevertParams {
                thread_id: thread.id.clone(),
                before_turn_id: active_turn.id.clone(),
            },
        })
        .await?;
    let completed: TurnCompletedNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_notification("turn/completed"),
    )
    .await??;
    assert_eq!(completed.thread_id, thread.id);
    assert_eq!(completed.turn.status, TurnStatus::Interrupted);
    assert!(reverted_thread.turns.is_empty());
    assert!(items_backwards_cursor.is_some());
    assert_eq!(
        turn_ids_from_cursor(
            &mut mcp,
            &thread.id,
            turns_backwards_cursor,
            /*sort_direction*/ None,
        )
        .await?,
        vec![first_turn.turn.id]
    );

    let resumed: ThreadResumeResponse = mcp
        .request(|request_id| ClientRequest::ThreadResume {
            request_id,
            params: ThreadResumeParams {
                thread_id: thread.id.clone(),
                ..Default::default()
            },
        })
        .await?;
    assert_eq!(resumed.approval_policy, AskForApproval::Never);

    mcp.start_turn_and_wait_for_completion(TurnStartParams {
        thread_id: thread.id,
        input: vec![UserInput::Text {
            text: "third".to_string(),
            text_elements: Vec::new(),
        }],
        ..Default::default()
    })
    .await?;
    Ok(())
}

async fn turn_ids_from_cursor(
    mcp: &mut TestAppServer,
    thread_id: &str,
    cursor: Option<String>,
    sort_direction: Option<SortDirection>,
) -> Result<Vec<String>> {
    let ThreadTurnsListResponse { data, .. } = mcp
        .request(|request_id| ClientRequest::ThreadTurnsList {
            request_id,
            params: ThreadTurnsListParams {
                thread_id: thread_id.to_string(),
                cursor,
                limit: None,
                sort_direction,
                items_view: None,
            },
        })
        .await?;
    Ok(data.into_iter().map(|turn| turn.id).collect())
}

async fn initialize_experimental(mcp: &mut TestAppServer) -> Result<()> {
    let initialized = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.initialize_with_capabilities(
            ClientInfo {
                name: "test-client".to_string(),
                title: None,
                version: "0.1.0".to_string(),
            },
            Some(InitializeCapabilities {
                experimental_api: true,
                request_attestation: false,
                opt_out_notification_methods: None,
                mcp_server_openai_form_elicitation: false,
                extensions: None,
            }),
        ),
    )
    .await??;
    assert!(matches!(initialized, JSONRPCMessage::Response(_)));
    Ok(())
}
