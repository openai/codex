use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::write_models_cache;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;
use test_case::test_case;
use tokio::time::timeout;

#[cfg(windows)]
const READ_TIMEOUT: Duration = Duration::from_secs(25);
#[cfg(not(windows))]
const READ_TIMEOUT: Duration = Duration::from_secs(10);
const NAMESPACE: &str = "collaboration";
const PARENT_INSTRUCTIONS: &str = "parent-only developer instructions";
const CHILD_INSTRUCTIONS: &str = "child-only developer instructions";
const ROLE_INSTRUCTIONS: &str = "configured role developer instructions";

/// V2 fork modes, roles, and unset/blank overrides expose their agreed instruction precedence.
#[test_case("no history"; "no history")]
#[test_case("full history"; "full history")]
#[test_case("bounded history"; "bounded history")]
#[test_case("configured role without instructions"; "configured role without instructions")]
#[test_case("unset override"; "unset override")]
#[test_case("blank override"; "blank override")]
#[test_case("parent has no instructions"; "parent has no instructions")]
#[test_case("explicit configured role"; "explicit configured role")]
#[test_case("implicit configured default"; "implicit configured default")]
#[test_case("full fork skips default role"; "full fork skips default role")]
#[tokio::test]
async fn spawned_subagents_apply_configured_developer_instruction_precedence(
    case: &str,
) -> Result<()> {
    let fork_turns = match case {
        "bounded history" => Some("1"),
        "no history"
        | "configured role without instructions"
        | "explicit configured role"
        | "implicit configured default" => Some("none"),
        _ => None,
    };
    let agent_type = match case {
        "configured role without instructions" | "explicit configured role" => Some("custom"),
        _ => None,
    };
    let configured_override = match case {
        "unset override" => None,
        "blank override" => Some("   "),
        "full history" => Some("  child-only developer instructions  "),
        _ => Some(CHILD_INSTRUCTIONS),
    };
    let parent = if case == "parent has no instructions" {
        None
    } else {
        Some(PARENT_INSTRUCTIONS)
    };
    let configured_roles = matches!(
        case,
        "configured role without instructions"
            | "explicit configured role"
            | "implicit configured default"
            | "full fork skips default role"
    );
    let role_has_instructions = matches!(
        case,
        "explicit configured role" | "implicit configured default" | "full fork skips default role"
    );
    let expected = match case {
        "unset override" => Some(PARENT_INSTRUCTIONS),
        "blank override" => None,
        "explicit configured role" | "implicit configured default" => Some(ROLE_INSTRUCTIONS),
        _ => Some(CHILD_INSTRUCTIONS),
    };
    const PARENT_PROMPT: &str = "spawn the instruction override worker";
    const CHILD_PROMPT: &str = "perform the instruction override task";
    const SPAWN_CALL_ID: &str = "spawn-instruction-override-worker";

    let server = responses::start_mock_server().await;
    let mut spawn_args = json!({"message": CHILD_PROMPT, "task_name": "worker"});
    if let Some(fork_turns) = fork_turns {
        spawn_args["fork_turns"] = json!(fork_turns);
    }
    if let Some(agent_type) = agent_type {
        spawn_args["agent_type"] = json!(agent_type);
    }
    let parent_request = responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            String::from_utf8_lossy(&request.body).contains(PARENT_PROMPT)
        },
        responses::sse(vec![
            responses::ev_response_created("parent-spawn"),
            responses::ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                NAMESPACE,
                "spawn_agent",
                &serde_json::to_string(&spawn_args)?,
            ),
            responses::ev_completed("parent-spawn"),
        ]),
    )
    .await;
    let child_request = responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            let body = String::from_utf8_lossy(&request.body);
            body.contains(CHILD_PROMPT) && !body.contains(SPAWN_CALL_ID)
        },
        responses::sse(vec![
            responses::ev_response_created("child-work"),
            responses::ev_assistant_message("child-message", "child complete"),
            responses::ev_completed("child-work"),
        ]),
    )
    .await;
    responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            String::from_utf8_lossy(&request.body).contains(SPAWN_CALL_ID)
        },
        responses::sse(vec![
            responses::ev_response_created("parent-complete"),
            responses::ev_assistant_message("parent-message", "parent complete"),
            responses::ev_completed("parent-complete"),
        ]),
    )
    .await;

    let mut feature_config = "[features.multi_agent_v2]\nenabled = true".to_string();
    if let Some(configured_override) = configured_override {
        feature_config.push_str(&format!(
            "\nsubagent_developer_instructions = {configured_override:?}"
        ));
    }
    if configured_roles {
        feature_config.push_str(
                "\n\n[agents.custom]\ndescription = \"configured role\"\nconfig_file = \"./config.toml\"\n\n[agents.default]\ndescription = \"configured default role\"\nconfig_file = \"./config.toml\"",
            );
    }
    let codex_home = TempDir::new()?;
    let mut config = MockResponsesConfig::new(&server.uri()).with_model("gpt-5.4");
    if role_has_instructions {
        config =
            config.with_root_config(&format!("developer_instructions = {ROLE_INSTRUCTIONS:?}"));
    }
    config
        .with_extra_config(&feature_config)
        .write(codex_home.path())?;
    write_models_cache(codex_home.path())?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread, .. } = app_server
        .start_thread(ThreadStartParams {
            model: Some("gpt-5.4".to_string()),
            developer_instructions: parent.map(str::to_string),
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = app_server
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![UserInput::Text {
                    text: PARENT_PROMPT.to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;
    let child_request = timeout(READ_TIMEOUT, async {
        loop {
            if let Some(request) = child_request
                .requests()
                .into_iter()
                .find(|request| !request.inputs_of_type("agent_message").is_empty())
            {
                break request;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;

    let parent_texts = parent_request
        .single_request()
        .message_input_texts("developer");
    if let Some(parent) = parent {
        assert!(
            parent_texts.iter().any(|text| text == parent),
            "{case}: parent developer instructions unexpectedly changed: {parent_texts:?}"
        );
    }
    let child_texts = child_request.message_input_texts("developer");
    let instruction_texts = child_texts
        .iter()
        .map(String::as_str)
        .filter(|text| {
            matches!(
                *text,
                PARENT_INSTRUCTIONS | CHILD_INSTRUCTIONS | ROLE_INSTRUCTIONS
            )
        })
        .collect::<Vec<_>>();
    let expected_instruction_texts = match expected {
        Some(instructions) => vec![instructions],
        None => Vec::new(),
    };
    assert_eq!(
        instruction_texts, expected_instruction_texts,
        "{case}: child received unexpected developer instructions"
    );
    assert!(
        child_texts.iter().all(|text| !text.is_empty()),
        "{case}: an empty developer fragment reached the model: {child_texts:?}"
    );

    Ok(())
}

/// A full-history worker fork replaces parent instructions inside persisted compacted history.
#[tokio::test]
async fn compacted_full_history_fork_replaces_parent_developer_instructions() -> Result<()> {
    const COMPACT_SETUP_PROMPT: &str = "prepare the parent for compaction";
    const COMPACT_PROMPT: &str = "summarize the compacted parent";
    const COMPACTED_SUMMARY: &str = "preserved compacted parent summary";
    const SPAWN_PROMPT: &str = "spawn the compacted-history worker";
    const CHILD_PROMPT: &str = "inspect the compacted parent history";
    const SETUP_CALL_ID: &str = "trigger-parent-compaction";
    const SPAWN_CALL_ID: &str = "spawn-compacted-history-worker";

    let server = responses::start_mock_server().await;
    let compaction_requests = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("parent-before-compaction"),
                responses::ev_function_call(SETUP_CALL_ID, "unsupported_tool", "{}"),
                responses::ev_completed_with_tokens(
                    "parent-before-compaction",
                    /*total_tokens*/ 96,
                ),
            ]),
            responses::sse(vec![
                responses::ev_response_created("parent-compaction"),
                responses::ev_assistant_message("parent-summary", COMPACTED_SUMMARY),
                responses::ev_completed_with_tokens("parent-compaction", /*total_tokens*/ 10),
            ]),
            responses::sse(vec![
                responses::ev_response_created("parent-after-compaction"),
                responses::ev_assistant_message("parent-ready", "parent history compacted"),
                responses::ev_completed_with_tokens(
                    "parent-after-compaction",
                    /*total_tokens*/ 10,
                ),
            ]),
        ],
    )
    .await;
    let parent_request = responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| String::from_utf8_lossy(&request.body).contains(SPAWN_PROMPT),
        responses::sse(vec![
            responses::ev_response_created("parent-spawn-after-compaction"),
            responses::ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                NAMESPACE,
                "spawn_agent",
                &serde_json::to_string(&json!({
                    "message": CHILD_PROMPT,
                    "task_name": "compacted_worker",
                }))?,
            ),
            responses::ev_completed("parent-spawn-after-compaction"),
        ]),
    )
    .await;
    let child_request = responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            let body = String::from_utf8_lossy(&request.body);
            body.contains(CHILD_PROMPT) && !body.contains(SPAWN_CALL_ID)
        },
        responses::sse(vec![
            responses::ev_response_created("compacted-child-work"),
            responses::ev_assistant_message("compacted-child-message", "child complete"),
            responses::ev_completed("compacted-child-work"),
        ]),
    )
    .await;
    responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            String::from_utf8_lossy(&request.body).contains(SPAWN_CALL_ID)
        },
        responses::sse(vec![
            responses::ev_response_created("compacted-parent-complete"),
            responses::ev_assistant_message("compacted-parent-message", "parent complete"),
            responses::ev_completed("compacted-parent-complete"),
        ]),
    )
    .await;

    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_model("gpt-5.4")
        .with_root_config(&format!(
            "developer_instructions = {PARENT_INSTRUCTIONS:?}\nmodel_context_window = 100\nmodel_auto_compact_token_limit = 90\ncompact_prompt = {COMPACT_PROMPT:?}"
        ))
        .with_extra_config(&format!(
            "[features.multi_agent_v2]\nenabled = true\nsubagent_developer_instructions = {CHILD_INSTRUCTIONS:?}"
        ))
        .write(codex_home.path())?;
    write_models_cache(codex_home.path())?;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread, .. } = app_server
        .start_thread(ThreadStartParams {
            model: Some("gpt-5.4".to_string()),
            ..Default::default()
        })
        .await?;
    timeout(
        READ_TIMEOUT,
        app_server.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: COMPACT_SETUP_PROMPT.to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        }),
    )
    .await??;

    let compaction_requests = compaction_requests.requests();
    assert_eq!(compaction_requests.len(), 3);
    assert!(
        compaction_requests[1].body_contains_text(COMPACT_PROMPT),
        "the setup turn should perform actual mid-turn compaction"
    );
    assert!(
        compaction_requests[2]
            .message_input_texts("developer")
            .iter()
            .any(|text| text == PARENT_INSTRUCTIONS),
        "mid-turn compaction should retain parent instructions in its replacement history"
    );

    let _: TurnStartResponse = app_server
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id,
                input: vec![UserInput::Text {
                    text: SPAWN_PROMPT.to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;
    let child_request = timeout(READ_TIMEOUT, async {
        loop {
            if let Some(request) = child_request
                .requests()
                .into_iter()
                .find(|request| !request.inputs_of_type("agent_message").is_empty())
            {
                break request;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;

    assert!(
        parent_request
            .single_request()
            .message_input_texts("developer")
            .iter()
            .any(|text| text == PARENT_INSTRUCTIONS),
        "the parent should retain its own developer instructions after compaction"
    );
    assert!(
        child_request.body_contains_text(COMPACTED_SUMMARY),
        "the full-history child should inherit the compacted parent summary"
    );
    let child_developer_texts = child_request.message_input_texts("developer");
    assert_eq!(
        child_developer_texts
            .iter()
            .filter(|text| text.as_str() == CHILD_INSTRUCTIONS)
            .count(),
        1,
        "the child should receive its configured developer instructions exactly once"
    );
    assert!(
        child_developer_texts
            .iter()
            .all(|text| text != PARENT_INSTRUCTIONS),
        "the child should not inherit parent instructions from compacted history"
    );

    Ok(())
}

/// Cold root resume preserves inherited instructions or reapplies the configured v2 override.
#[test_case(
    None,
    PARENT_INSTRUCTIONS,
    CHILD_INSTRUCTIONS;
    "inherits parent developer instructions without an override"
)]
#[test_case(
    Some(CHILD_INSTRUCTIONS),
    CHILD_INSTRUCTIONS,
    PARENT_INSTRUCTIONS;
    "reapplies configured subagent developer instructions"
)]
#[tokio::test]
async fn cold_resume_preserves_effective_developer_instructions_for_roleless_worker(
    configured_subagent_developer_instructions: Option<&str>,
    expected_developer_instructions: &str,
    unexpected_developer_instructions: &str,
) -> Result<()> {
    const INITIAL_PROMPT: &str = "spawn a durable instruction worker";
    const INITIAL_TASK: &str = "perform the initial durable instruction task";
    const FOLLOWUP_PROMPT: &str = "continue the durable instruction worker";
    const FOLLOWUP_TASK: &str = "perform the resumed durable instruction task";
    const SPAWN_CALL_ID: &str = "spawn-durable-instruction-worker";
    const WAIT_CALL_ID: &str = "wait-for-durable-instruction-worker";
    const FOLLOWUP_CALL_ID: &str = "followup-durable-instruction-worker";

    let server = responses::start_mock_server().await;
    responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            String::from_utf8_lossy(&request.body).contains(INITIAL_PROMPT)
        },
        responses::sse(vec![
            responses::ev_response_created("initial-parent-spawn"),
            responses::ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                NAMESPACE,
                "spawn_agent",
                &serde_json::to_string(&json!({
                    "message": INITIAL_TASK,
                    "task_name": "worker",
                    "fork_turns": "none",
                }))?,
            ),
            responses::ev_completed("initial-parent-spawn"),
        ]),
    )
    .await;
    let initial_child_request = responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            let body = String::from_utf8_lossy(&request.body);
            body.contains(INITIAL_TASK) && !body.contains(SPAWN_CALL_ID)
        },
        responses::sse(vec![
            responses::ev_response_created("initial-child-work"),
            responses::ev_assistant_message("initial-child-message", "initial child complete"),
            responses::ev_completed("initial-child-work"),
        ]),
    )
    .await;
    responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            let body = String::from_utf8_lossy(&request.body);
            body.contains(SPAWN_CALL_ID) && !body.contains(WAIT_CALL_ID)
        },
        responses::sse(vec![
            responses::ev_response_created("initial-parent-wait"),
            responses::ev_function_call_with_namespace(WAIT_CALL_ID, NAMESPACE, "wait_agent", "{}"),
            responses::ev_completed("initial-parent-wait"),
        ]),
    )
    .await;
    responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| String::from_utf8_lossy(&request.body).contains(WAIT_CALL_ID),
        responses::sse(vec![
            responses::ev_response_created("initial-parent-complete"),
            responses::ev_assistant_message("initial-parent-message", "initial parent complete"),
            responses::ev_completed("initial-parent-complete"),
        ]),
    )
    .await;

    let feature_config = match configured_subagent_developer_instructions {
        Some(instructions) => format!(
            "[features.multi_agent_v2]\nenabled = true\nsubagent_developer_instructions = {instructions:?}"
        ),
        None => "[features.multi_agent_v2]\nenabled = true".to_string(),
    };
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_model("gpt-5.4")
        .with_root_config(&format!("developer_instructions = {PARENT_INSTRUCTIONS:?}"))
        .with_extra_config(&feature_config)
        .write(codex_home.path())?;
    write_models_cache(codex_home.path())?;

    let thread_id = {
        let mut app_server = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .build_initialized()
            .await?;
        let ThreadStartResponse { thread, .. } = app_server
            .start_thread(ThreadStartParams {
                model: Some("gpt-5.4".to_string()),
                ..Default::default()
            })
            .await?;
        timeout(
            READ_TIMEOUT,
            app_server.start_turn_and_wait_for_completion(TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![UserInput::Text {
                    text: INITIAL_PROMPT.to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            }),
        )
        .await??;
        let initial_child_request = initial_child_request
            .requests()
            .into_iter()
            .find(|request| !request.inputs_of_type("agent_message").is_empty())
            .expect("initial worker model request");
        let developer_texts = initial_child_request.message_input_texts("developer");
        assert!(
            developer_texts
                .iter()
                .any(|text| text == expected_developer_instructions),
            "initial worker lost its effective developer instructions: {developer_texts:?}"
        );
        assert!(
            developer_texts
                .iter()
                .all(|text| text != unexpected_developer_instructions),
            "initial worker received the wrong developer instructions: {developer_texts:?}"
        );
        let shutdown = timeout(READ_TIMEOUT, app_server.shutdown_gracefully()).await??;
        assert!(
            shutdown.success(),
            "initial app-server shutdown failed: {shutdown}"
        );
        thread.id
    };

    responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            let body = String::from_utf8_lossy(&request.body);
            body.contains(FOLLOWUP_PROMPT) && !body.contains(FOLLOWUP_CALL_ID)
        },
        responses::sse(vec![
            responses::ev_response_created("resumed-parent-followup"),
            responses::ev_function_call_with_namespace(
                FOLLOWUP_CALL_ID,
                NAMESPACE,
                "followup_task",
                &serde_json::to_string(&json!({
                    "target": "worker",
                    "message": FOLLOWUP_TASK,
                }))?,
            ),
            responses::ev_completed("resumed-parent-followup"),
        ]),
    )
    .await;
    let resumed_child_request = responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            let body = String::from_utf8_lossy(&request.body);
            body.contains(FOLLOWUP_TASK) && !body.contains(FOLLOWUP_CALL_ID)
        },
        responses::sse(vec![
            responses::ev_response_created("resumed-child-work"),
            responses::ev_assistant_message("resumed-child-message", "resumed child complete"),
            responses::ev_completed("resumed-child-work"),
        ]),
    )
    .await;
    responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            String::from_utf8_lossy(&request.body).contains(FOLLOWUP_CALL_ID)
        },
        responses::sse(vec![
            responses::ev_response_created("resumed-parent-complete"),
            responses::ev_assistant_message("resumed-parent-message", "resumed parent complete"),
            responses::ev_completed("resumed-parent-complete"),
        ]),
    )
    .await;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let _: ThreadResumeResponse = app_server
        .request(|request_id| ClientRequest::ThreadResume {
            request_id,
            params: ThreadResumeParams {
                thread_id: thread_id.clone(),
                ..Default::default()
            },
        })
        .await?;
    let _: TurnStartResponse = app_server
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id,
                input: vec![UserInput::Text {
                    text: FOLLOWUP_PROMPT.to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;
    let resumed_child_request = timeout(READ_TIMEOUT, async {
        loop {
            if let Some(request) = resumed_child_request
                .requests()
                .into_iter()
                .find(|request| {
                    request
                        .inputs_of_type("agent_message")
                        .iter()
                        .any(|message| {
                            message.get("recipient").and_then(serde_json::Value::as_str)
                                == Some("/root/worker")
                        })
                        && request.body_contains_text(FOLLOWUP_TASK)
                })
            {
                break request;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;

    let developer_texts = resumed_child_request.message_input_texts("developer");
    assert!(
        developer_texts
            .iter()
            .any(|text| text == expected_developer_instructions),
        "resumed worker lost its effective developer instructions: {developer_texts:?}"
    );
    assert!(
        developer_texts
            .iter()
            .all(|text| text != unexpected_developer_instructions),
        "resumed worker received the wrong developer instructions: {developer_texts:?}"
    );

    Ok(())
}
