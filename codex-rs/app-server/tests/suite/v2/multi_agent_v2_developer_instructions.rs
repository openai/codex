use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::write_models_cache;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxMode;
use codex_app_server_protocol::SandboxPolicy;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::ThreadTokenUsageUpdatedNotification;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::ThreadUnsubscribeStatus;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_protocol::ThreadId;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_state::StateRuntime;
use codex_utils_absolute_path::test_support::PathExt;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
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
#[test_case("full history configured role"; "full history configured role")]
#[test_case("implicit configured default"; "implicit configured default")]
#[test_case("bounded implicit configured default"; "bounded implicit configured default")]
#[test_case("full fork skips default role"; "full fork skips default role")]
#[tokio::test]
async fn spawned_subagents_apply_configured_developer_instruction_precedence(
    case: &str,
) -> Result<()> {
    let fork_turns = match case {
        "bounded history" | "bounded implicit configured default" => Some("1"),
        "no history" | "explicit configured role" | "implicit configured default" => Some("none"),
        _ => None,
    };
    let agent_type = match case {
        "configured role without instructions"
        | "explicit configured role"
        | "full history configured role" => Some("custom"),
        _ => None,
    };
    let configured_override = match case {
        "unset override"
        | "full history configured role"
        | "configured role without instructions"
        | "bounded implicit configured default" => None,
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
            | "full history configured role"
            | "implicit configured default"
            | "bounded implicit configured default"
            | "full fork skips default role"
    );
    let role_has_instructions = matches!(
        case,
        "explicit configured role"
            | "full history configured role"
            | "implicit configured default"
            | "bounded implicit configured default"
            | "full fork skips default role"
    );
    let expected = match case {
        "unset override" | "configured role without instructions" => Some(PARENT_INSTRUCTIONS),
        "blank override" => None,
        "explicit configured role"
        | "full history configured role"
        | "implicit configured default"
        | "bounded implicit configured default" => Some(ROLE_INSTRUCTIONS),
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
    let configured_model = if case == "full history configured role" {
        "gpt-5.5"
    } else {
        "gpt-5.4"
    };
    let mut config = MockResponsesConfig::new(&server.uri()).with_model(configured_model);
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
    if case == "full history configured role" {
        assert_eq!(child_request.body_json()["model"], json!("gpt-5.5"));
        assert!(
            child_request.body_contains_text(PARENT_PROMPT),
            "the child should inherit the parent's conversation history"
        );
        assert!(
            child_texts
                .iter()
                .any(|text| text.contains("<model_switch>")),
            "the child should preserve the parent's model context"
        );
    }
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

/// Child attachment and owner-mediated cold resume preserve recorded instructions and parent authority.
#[test_case(
    None,
    None,
    PARENT_INSTRUCTIONS,
    ThreadHistoryMode::Legacy;
    "inherits parent developer instructions without an override"
)]
#[test_case(
    Some(CHILD_INSTRUCTIONS),
    None,
    CHILD_INSTRUCTIONS,
    ThreadHistoryMode::Legacy;
    "reapplies configured subagent developer instructions"
)]
#[test_case(
    Some(CHILD_INSTRUCTIONS),
    Some("custom"),
    ROLE_INSTRUCTIONS,
    ThreadHistoryMode::Paginated;
    "reapplies updated configured role settings and restores paginated usage"
)]
#[test_case(
    Some(CHILD_INSTRUCTIONS),
    Some("default"),
    ROLE_INSTRUCTIONS,
    ThreadHistoryMode::Legacy;
    "reapplies an implicitly selected configured default role"
)]
#[tokio::test]
async fn cold_resume_preserves_effective_developer_instructions_for_worker(
    configured_subagent_developer_instructions: Option<&str>,
    agent_type: Option<&str>,
    expected_developer_instructions: &str,
    history_mode: ThreadHistoryMode,
) -> Result<()> {
    const INITIAL_PROMPT: &str = "spawn a durable instruction worker";
    const INITIAL_TASK: &str = "perform the initial durable instruction task";
    const FOLLOWUP_PROMPT: &str = "continue the durable instruction worker";
    const FOLLOWUP_TASK: &str = "perform the resumed durable instruction task";
    const SPAWN_CALL_ID: &str = "spawn-durable-instruction-worker";
    const WAIT_CALL_ID: &str = "wait-for-durable-instruction-worker";
    const FOLLOWUP_CALL_ID: &str = "followup-durable-instruction-worker";
    const DIRECT_RESUME_INSTRUCTIONS: &str = "direct resume must not replace worker instructions";
    const COLD_RESUME_ERROR: &str = "cannot resume an unloaded multi-agent v2 sub-agent through its parent; resume the parent first, or use thread/read to inspect it";

    let instruction_markers = [
        PARENT_INSTRUCTIONS,
        CHILD_INSTRUCTIONS,
        ROLE_INSTRUCTIONS,
        DIRECT_RESUME_INSTRUCTIONS,
    ];
    let assert_developer_instructions = |request: &responses::ResponsesRequest, phase: &str| {
        let instruction_texts = request
            .message_input_texts("developer")
            .into_iter()
            .filter(|text| instruction_markers.contains(&text.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            instruction_texts,
            vec![expected_developer_instructions.to_string()],
            "{phase} worker received unexpected developer instructions"
        );
    };
    let has_shell_tool = |request: &responses::ResponsesRequest| {
        let body = request.body_json();
        let tools = body["tools"].as_array().expect("model-visible tools");
        ["exec_command", "shell_command"].into_iter().any(|name| {
            tools.iter().any(|tool| tool["name"] == name)
                || responses::namespace_child_tool(&body, "functions", name).is_some()
        })
    };

    let server = responses::start_mock_server().await;
    let redirect_server = responses::start_mock_server().await;
    let redirect_base_url = format!("{}/v1", redirect_server.uri());
    let mut spawn_args = json!({
        "message": INITIAL_TASK,
        "task_name": "worker",
        "fork_turns": "none",
        "reasoning_effort": "low",
    });
    if let Some(agent_type) = agent_type.filter(|role| *role != "default") {
        spawn_args["agent_type"] = json!(agent_type);
    }
    let initial_parent_request = responses::mount_sse_once_match(
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
                &serde_json::to_string(&spawn_args)?,
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
            responses::ev_completed_with_tokens("initial-child-work", /*total_tokens*/ 150),
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

    let mut feature_config = match configured_subagent_developer_instructions {
        Some(instructions) => format!(
            "[features.multi_agent_v2]\nenabled = true\nsubagent_developer_instructions = {instructions:?}"
        ),
        None => "[features.multi_agent_v2]\nenabled = true".to_string(),
    };
    let codex_home = TempDir::new()?;
    let role_path = codex_home.path().join("worker.toml");
    if let Some(agent_type) = agent_type {
        std::fs::write(
            &role_path,
            format!(
                "developer_instructions = {ROLE_INSTRUCTIONS:?}\nmodel_reasoning_effort = \"low\"\nfeatures.shell_tool = false\n"
            ),
        )?;
        feature_config.push_str(&format!(
            "\n\n[agents.{agent_type}]\ndescription = \"configured role\"\nconfig_file = \"./worker.toml\""
        ));
    }
    MockResponsesConfig::new(&server.uri())
        .with_model("gpt-5.4")
        .with_root_config(&format!(
            "developer_instructions = {PARENT_INSTRUCTIONS:?}\nmodel_reasoning_effort = \"high\""
        ))
        .with_extra_config(&feature_config)
        .write(codex_home.path())?;
    write_models_cache(codex_home.path())?;

    let (thread_id, child_resume_params, baseline) = {
        let mut app_server = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .build_initialized()
            .await?;
        let ThreadStartResponse { thread, .. } = app_server
            .start_thread(ThreadStartParams {
                model: Some("gpt-5.4".to_string()),
                history_mode: Some(history_mode),
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
        assert_developer_instructions(&initial_child_request, "initial");
        assert_eq!(
            initial_child_request.body_json()["reasoning"]["effort"],
            json!("low")
        );
        if agent_type.is_some() {
            assert!(has_shell_tool(&initial_parent_request.single_request()));
            assert!(!has_shell_tool(&initial_child_request));
        }
        let child_thread_id = initial_child_request
            .header("thread-id")
            .expect("initial worker thread ID");
        let baseline: ThreadResumeResponse = app_server
            .request(|request_id| ClientRequest::ThreadResume {
                request_id,
                params: ThreadResumeParams {
                    thread_id: child_thread_id.clone(),
                    exclude_turns: true,
                    ..Default::default()
                },
            })
            .await?;
        assert_eq!(baseline.thread.id, child_thread_id);
        assert_eq!(baseline.thread.status, ThreadStatus::Idle);
        assert_eq!(baseline.thread.can_accept_direct_input, Some(false));
        assert_eq!(baseline.thread.agent_role.as_deref(), agent_type);
        assert_eq!(baseline.model_provider, "mock_provider");
        assert!(matches!(&baseline.sandbox, SandboxPolicy::ReadOnly { .. }));
        assert_eq!(baseline.reasoning_effort, Some(ReasoningEffort::Low));

        let unsubscribed: ThreadUnsubscribeResponse = app_server
            .request(|request_id| ClientRequest::ThreadUnsubscribe {
                request_id,
                params: ThreadUnsubscribeParams {
                    thread_id: child_thread_id.clone(),
                },
            })
            .await?;
        assert_eq!(unsubscribed.status, ThreadUnsubscribeStatus::Unsubscribed);
        let loaded: ThreadLoadedListResponse = app_server
            .request(|request_id| ClientRequest::ThreadLoadedList {
                request_id,
                params: ThreadLoadedListParams::default(),
            })
            .await?;
        assert!(loaded.data.contains(&child_thread_id));

        let child_resume_params = ThreadResumeParams {
            thread_id: child_thread_id,
            model_provider: Some("mock_provider".to_string()),
            sandbox: Some(SandboxMode::DangerFullAccess),
            config: Some(HashMap::from([(
                "model_providers.mock_provider.base_url".to_string(),
                json!(redirect_base_url),
            )])),
            developer_instructions: Some(DIRECT_RESUME_INSTRUCTIONS.to_string()),
            exclude_turns: true,
            ..Default::default()
        };
        let reattached: ThreadResumeResponse = app_server
            .request(|request_id| ClientRequest::ThreadResume {
                request_id,
                params: child_resume_params.clone(),
            })
            .await?;
        assert_eq!(reattached, baseline);
        let shutdown = timeout(READ_TIMEOUT, app_server.shutdown_gracefully()).await??;
        assert!(
            shutdown.success(),
            "initial app-server shutdown failed: {shutdown}"
        );
        (thread.id, child_resume_params, baseline)
    };

    if agent_type.is_some() {
        std::fs::write(
            &role_path,
            format!(
                r#"developer_instructions = {ROLE_INSTRUCTIONS:?}
model_reasoning_effort = "high"
sandbox_mode = "danger-full-access"
model_providers.mock_provider.name = "Untrusted role provider"
model_providers.mock_provider.base_url = {redirect_base_url:?}
features.shell_tool = false
"#
            ),
        )?;
    }

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
    let child_thread_id = child_resume_params.thread_id.clone();
    let child_by_path = ThreadResumeParams {
        thread_id: ThreadId::new().to_string(),
        path: Some(baseline.thread.path.clone().expect("worker rollout path")),
        ..child_resume_params.clone()
    };
    for params in [child_resume_params.clone(), child_by_path.clone()] {
        let request_id = app_server.send_thread_resume_request(params).await?;
        let error = timeout(
            READ_TIMEOUT,
            app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
        )
        .await??;
        assert_eq!(
            serde_json::to_value(error.error)?,
            json!({"code": -32600, "message": COLD_RESUME_ERROR})
        );
    }
    let stored_child: ThreadReadResponse = app_server
        .request(|request_id| ClientRequest::ThreadRead {
            request_id,
            params: ThreadReadParams {
                thread_id: child_thread_id.clone(),
                include_turns: true,
            },
        })
        .await?;
    assert_eq!(stored_child.thread.id, child_thread_id);
    assert_eq!(stored_child.thread.status, ThreadStatus::NotLoaded);
    assert_eq!(stored_child.thread.agent_role.as_deref(), agent_type);
    assert!(!stored_child.thread.turns.is_empty());

    let resumed_parent: ThreadResumeResponse = app_server
        .request(|request_id| ClientRequest::ThreadResume {
            request_id,
            params: ThreadResumeParams {
                thread_id: thread_id.clone(),
                ..Default::default()
            },
        })
        .await?;
    assert_eq!(resumed_parent.reasoning_effort, Some(ReasoningEffort::High));
    let loaded: ThreadLoadedListResponse = app_server
        .request(|request_id| ClientRequest::ThreadLoadedList {
            request_id,
            params: ThreadLoadedListParams::default(),
        })
        .await?;
    assert!(loaded.data.contains(&thread_id));
    assert!(!loaded.data.contains(&child_thread_id));

    let mut expected = baseline;
    if history_mode == ThreadHistoryMode::Paginated {
        let state_db = StateRuntime::init(
            codex_state::SqliteConfig::new_for_testing(codex_home.path().abs()),
            "mock_provider".into(),
        )
        .await?;
        let mut metadata = state_db
            .get_thread(ThreadId::from_string(&child_thread_id)?)
            .await?
            .expect("worker metadata");
        let mut source: SessionSource = serde_json::from_str(&metadata.source)?;
        let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id, ..
        }) = &mut source
        else {
            anyhow::bail!("expected worker thread-spawn source");
        };
        // Stale display metadata must not choose which parent controls the resume.
        *parent_thread_id = ThreadId::new();
        metadata.source = serde_json::to_string(&source)?;
        state_db.upsert_thread(&metadata).await?;
        expected.thread.source = source.into();
    }

    let is_child_usage = |notification: &JSONRPCNotification| {
        notification.method == "thread/tokenUsage/updated"
            && notification
                .params
                .as_ref()
                .is_some_and(|params| params["threadId"].as_str() == Some(child_thread_id.as_str()))
    };
    let resume_id = app_server.send_thread_resume_request(child_by_path).await?;
    let resumed: ThreadResumeResponse = timeout(READ_TIMEOUT, async {
        loop {
            match app_server.read_next_message().await? {
                JSONRPCMessage::Response(response)
                    if response.id == RequestId::Integer(resume_id) =>
                {
                    break app_test_support::to_response(response);
                }
                JSONRPCMessage::Notification(notification) => assert!(
                    !is_child_usage(&notification),
                    "child usage replay must follow the resume response"
                ),
                JSONRPCMessage::Error(error) => anyhow::bail!("unexpected error: {error:?}"),
                JSONRPCMessage::Request(_) | JSONRPCMessage::Response(_) => {}
            }
        }
    })
    .await??;
    if history_mode == ThreadHistoryMode::Paginated {
        assert_eq!(resumed.thread.history_mode, history_mode);
        assert!(resumed.thread.turns.is_empty());
        let replay = timeout(
            READ_TIMEOUT,
            app_server
                .read_stream_until_matching_notification("child usage replay", &is_child_usage),
        )
        .await??;
        let replay: ThreadTokenUsageUpdatedNotification =
            serde_json::from_value(replay.params.expect("usage parameters"))?;
        let saved_turn = stored_child.thread.turns.last().expect("saved child turn");
        assert_eq!(saved_turn.status, TurnStatus::Completed);
        assert_eq!(
            (replay.turn_id, replay.token_usage.total.total_tokens),
            (saved_turn.id.clone(), 150),
        );
        let warm: ThreadResumeResponse = app_server
            .request(|request_id| ClientRequest::ThreadResume {
                request_id,
                params: child_resume_params,
            })
            .await?;
        assert!(warm.thread.turns.is_empty());
        assert!(
            timeout(
                Duration::from_millis(100),
                app_server
                    .read_stream_until_matching_notification("child usage replay", &is_child_usage),
            )
            .await
            .is_err(),
            "warm metadata-only child resume should not replay token usage"
        );
    }
    let expected_reasoning_effort = if agent_type.is_some() {
        ReasoningEffort::High
    } else {
        ReasoningEffort::Low
    };
    assert_eq!(
        resumed.reasoning_effort,
        Some(expected_reasoning_effort.clone())
    );
    let authority = |response: ThreadResumeResponse| {
        (
            response.thread.id,
            response.thread.parent_thread_id,
            response.thread.source,
            response.thread.status,
            response.thread.can_accept_direct_input,
            response.model_provider,
            response.sandbox,
            response.approval_policy,
            response.approvals_reviewer,
            response.active_permission_profile,
        )
    };
    assert_eq!(authority(resumed), authority(expected));
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
                    request.header("thread-id").as_deref() == Some(child_thread_id.as_str())
                        && request.body_contains_text(FOLLOWUP_TASK)
                })
            {
                break request;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;

    assert_eq!(
        resumed_child_request.body_json()["reasoning"]["effort"],
        json!(expected_reasoning_effort)
    );
    if agent_type.is_some() {
        assert!(!has_shell_tool(&resumed_child_request));
    }
    assert_developer_instructions(&resumed_child_request, "resumed");
    assert!(
        redirect_server
            .received_requests()
            .await
            .expect("redirect mock requests")
            .is_empty(),
        "caller or role config redirected the owner-restored child"
    );

    Ok(())
}
