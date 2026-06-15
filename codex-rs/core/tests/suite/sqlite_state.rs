use anyhow::Result;
use codex_config::types::McpServerConfig;
use codex_config::types::McpServerTransportConfig;
use codex_core::config::Config;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::ThreadId;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use codex_protocol::dynamic_tools::DynamicToolNamespaceSpec;
use codex_protocol::dynamic_tools::DynamicToolNamespaceTool;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::UserMessageEvent;
use codex_protocol::protocol::UserSubmission;
use codex_protocol::user_input::UserInput;
use codex_web_search_extension::install as install_web_search_extension;
use core_test_support::responses;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::ev_web_search_call_done;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use tokio::time::Duration;
use tracing_subscriber::prelude::*;
use uuid::Uuid;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn new_thread_is_recorded_in_state_db() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let thread_id = test.session_configured.thread_id;
    let rollout_path = test.codex.rollout_path().expect("rollout path");
    let db_path = codex_state::state_db_path(test.config.sqlite_home.as_path());

    for _ in 0..100 {
        if tokio::fs::try_exists(&db_path).await.unwrap_or(false) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let db = test.codex.state_db().expect("state db enabled");
    assert!(
        !rollout_path.exists(),
        "fresh thread rollout should not be materialized before first user message"
    );

    let initial_metadata = db.get_thread(thread_id).await?;
    assert!(
        initial_metadata.is_none(),
        "fresh thread should not be recorded in state db before first user message"
    );

    test.submit_turn("materialize rollout").await?;

    let mut metadata = None;
    for _ in 0..100 {
        metadata = db.get_thread(thread_id).await?;
        if metadata.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let metadata = metadata.expect("thread should exist in state db");
    assert_eq!(metadata.id, thread_id);
    assert_eq!(metadata.rollout_path, rollout_path.to_path_buf());
    assert!(
        rollout_path.exists(),
        "rollout should be materialized after first user message"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resume_restores_dynamic_tools_from_rollout_with_sqlite_enabled() -> Result<()> {
    let server = start_mock_server().await;
    let mock = mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
            responses::sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;

    let namespace = "resume_tools";
    let namespace_description = "Tools available after resume.";
    let tool_name = "resume_lookup";
    let tool_description = "Look up a value after resume.";
    let input_schema = json!({
        "type": "object",
        "properties": { "query": { "type": "string" } },
        "required": ["query"],
        "additionalProperties": false,
    });
    let dynamic_tool = DynamicToolSpec::Namespace(DynamicToolNamespaceSpec {
        name: namespace.to_string(),
        description: namespace_description.to_string(),
        tools: vec![DynamicToolNamespaceTool::Function(
            DynamicToolFunctionSpec {
                name: tool_name.to_string(),
                description: tool_description.to_string(),
                input_schema: input_schema.clone(),
                defer_loading: false,
            },
        )],
    });
    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
    });
    let base_test = builder.build(&server).await?;
    let started = base_test
        .thread_manager
        .start_thread_with_tools(base_test.config.clone(), vec![dynamic_tool])
        .await?;
    let rollout_path = started
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");

    started
        .thread
        .submit(Op::UserInput {
            submission: UserSubmission {
                items: vec![UserInput::Text {
                    text: "persist this thread".to_string(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
                responsesapi_client_metadata: None,
                additional_context: Default::default(),
            },
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&started.thread, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let mut resume_builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
    });
    let resumed = resume_builder
        .resume(&server, base_test.home.clone(), rollout_path)
        .await?;
    resumed.submit_turn("use the restored tool").await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    let resumed_body = requests[1].body_json();
    let tools = resumed_body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("resumed request tools");
    let restored_namespace = tools
        .iter()
        .find(|tool| tool.get("name") == Some(&json!(namespace)))
        .expect("dynamic tool namespace should be restored from rollout metadata");
    assert_eq!(
        restored_namespace,
        &json!({
            "type": "namespace",
            "name": namespace,
            "description": namespace_description,
            "tools": [{
                "type": "function",
                "name": tool_name,
                "description": tool_description,
                "strict": false,
                "parameters": input_schema,
            }],
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resume_restores_legacy_dynamic_tools_from_rollout_with_sqlite_enabled() -> Result<()> {
    let server = start_mock_server().await;
    let mock = mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
            responses::sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;

    let namespace = "resume_tools";
    let tool_name = "resume_lookup";
    let tool_description = "Look up a value after resume.";
    let input_schema = json!({
        "type": "object",
        "properties": { "query": { "type": "string" } },
        "required": ["query"],
        "additionalProperties": false,
    });
    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
    });
    let base_test = builder.build(&server).await?;
    let started = base_test
        .thread_manager
        .start_thread_with_tools(base_test.config.clone(), Vec::new())
        .await?;
    let rollout_path = started
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");

    started
        .thread
        .submit(Op::UserInput {
            submission: UserSubmission {
                items: vec![UserInput::Text {
                    text: "persist this thread".to_string(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
                responsesapi_client_metadata: None,
                additional_context: Default::default(),
            },
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&started.thread, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    started.thread.submit(Op::Shutdown).await?;
    wait_for_event(&started.thread, |event| {
        matches!(event, EventMsg::ShutdownComplete)
    })
    .await;

    let mut rollout_lines = fs::read_to_string(&rollout_path)?
        .lines()
        .map(serde_json::from_str::<serde_json::Value>)
        .collect::<serde_json::Result<Vec<_>>>()?;
    rollout_lines.first_mut().expect("session metadata line")["payload"]["dynamic_tools"] = json!([{
        "namespace": namespace,
        "name": tool_name,
        "description": tool_description,
        "inputSchema": input_schema,
        "exposeToContext": true,
    }]);
    let rollout = rollout_lines
        .iter()
        .map(serde_json::to_string)
        .collect::<serde_json::Result<Vec<_>>>()?
        .join("\n");
    fs::write(&rollout_path, format!("{rollout}\n"))?;

    let mut resume_builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
    });
    let resumed = resume_builder
        .resume(&server, base_test.home.clone(), rollout_path)
        .await?;
    resumed.submit_turn("use the restored tool").await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    let resumed_body = requests[1].body_json();
    let tools = resumed_body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("resumed request tools");
    let restored_namespace = tools
        .iter()
        .find(|tool| tool.get("name") == Some(&json!(namespace)))
        .expect("dynamic tool namespace should be restored from rollout metadata");
    assert_eq!(
        restored_namespace,
        &json!({
            "type": "namespace",
            "name": namespace,
            "description": "Tools in the resume_tools namespace.",
            "tools": [{
                "type": "function",
                "name": tool_name,
                "description": tool_description,
                "strict": false,
                "parameters": input_schema,
            }],
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn backfill_scans_existing_rollouts() -> Result<()> {
    let server = start_mock_server().await;

    let uuid = Uuid::now_v7();
    let thread_id = ThreadId::from_string(&uuid.to_string())?;
    let rollout_rel_path = format!("sessions/2026/01/27/rollout-2026-01-27T12-00-00-{uuid}.jsonl");
    let rollout_rel_path_for_hook = rollout_rel_path.clone();

    let mut builder = test_codex()
        .with_pre_build_hook(move |codex_home| {
            let rollout_path = codex_home.join(&rollout_rel_path_for_hook);
            let parent = rollout_path
                .parent()
                .expect("rollout path should have parent");
            fs::create_dir_all(parent).expect("should create rollout directory");
            let session_meta_line = SessionMetaLine {
                meta: SessionMeta {
                    id: thread_id,
                    forked_from_id: None,
                    parent_thread_id: None,
                    timestamp: "2026-01-27T12:00:00Z".to_string(),
                    cwd: codex_home.to_path_buf(),
                    originator: "test".to_string(),
                    cli_version: "test".to_string(),
                    source: SessionSource::default(),
                    thread_source: None,
                    agent_path: None,
                    agent_nickname: None,
                    agent_role: None,
                    model_provider: None,
                    base_instructions: None,
                    dynamic_tools: None,
                    memory_mode: None,
                    multi_agent_version: None,
                },
                git: None,
            };

            let lines = [
                RolloutLine {
                    timestamp: "2026-01-27T12:00:00Z".to_string(),
                    item: RolloutItem::SessionMeta(session_meta_line),
                },
                RolloutLine {
                    timestamp: "2026-01-27T12:00:01Z".to_string(),
                    item: RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
                        client_id: None,
                        message: "hello from backfill".to_string(),
                        images: None,
                        local_images: Vec::new(),
                        text_elements: Vec::new(),
                        ..Default::default()
                    })),
                },
            ];

            let jsonl = lines
                .iter()
                .map(|line| serde_json::to_string(line).expect("rollout line should serialize"))
                .collect::<Vec<_>>()
                .join("\n");
            fs::write(&rollout_path, format!("{jsonl}\n")).expect("should write rollout file");
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::Sqlite)
                .expect("test config should allow feature update");
        });

    let test = builder.build(&server).await?;

    let db_path = codex_state::state_db_path(test.config.sqlite_home.as_path());
    let rollout_path = test.config.codex_home.join(&rollout_rel_path);
    let default_provider = test.config.model_provider_id.clone();

    for _ in 0..20 {
        if tokio::fs::try_exists(&db_path).await.unwrap_or(false) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let db = test.codex.state_db().expect("state db enabled");

    let mut metadata = None;
    for _ in 0..40 {
        metadata = db.get_thread(thread_id).await?;
        if metadata.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let metadata = metadata.expect("backfilled thread should exist in state db");
    assert_eq!(metadata.id, thread_id);
    assert_eq!(metadata.rollout_path, rollout_path.to_path_buf());
    assert_eq!(metadata.model_provider, default_provider);
    assert!(metadata.first_user_message.is_some());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_messages_persist_in_state_db() -> Result<()> {
    let server = start_mock_server().await;
    mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
            responses::sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let db_path = codex_state::state_db_path(test.config.sqlite_home.as_path());
    for _ in 0..100 {
        if tokio::fs::try_exists(&db_path).await.unwrap_or(false) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    test.submit_turn("hello from sqlite").await?;
    test.submit_turn("another message").await?;

    let db = test.codex.state_db().expect("state db enabled");
    let thread_id = test.session_configured.thread_id;

    let mut metadata = None;
    for _ in 0..100 {
        metadata = db.get_thread(thread_id).await?;
        if metadata
            .as_ref()
            .map(|entry| entry.first_user_message.is_some())
            .unwrap_or(false)
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let metadata = metadata.expect("thread should exist in state db");
    assert!(metadata.first_user_message.is_some());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_search_marks_thread_memory_mode_polluted_when_configured() -> Result<()> {
    let server = start_mock_server().await;
    mount_sse_sequence(
        &server,
        vec![responses::sse(vec![
            ev_response_created("resp-1"),
            ev_web_search_call_done("ws-1", "completed", "weather seattle"),
            ev_completed("resp-1"),
        ])],
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
        config.memories.disable_on_external_context = true;
    });
    let test = builder.build(&server).await?;
    let db = test.codex.state_db().expect("state db enabled");
    let thread_id = test.session_configured.thread_id;

    test.submit_turn("search the web").await?;

    let mut memory_mode = None;
    for _ in 0..100 {
        memory_mode = db.get_thread_memory_mode(thread_id).await?;
        if memory_mode.as_deref() == Some("polluted") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    assert_eq!(memory_mode.as_deref(), Some("polluted"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn standalone_web_search_marks_thread_memory_mode_polluted_when_configured() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    Mock::given(method("POST"))
        .and(path("/v1/alpha/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "output": "Search result",
        })))
        .expect(1)
        .mount(&server)
        .await;
    mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                ev_response_created("resp-1"),
                responses::ev_function_call_with_namespace(
                    "web-run-1",
                    "web",
                    "run",
                    &json!({
                        "search_query": [{"q": "standalone web search"}],
                    })
                    .to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let auth = CodexAuth::from_api_key("dummy");
    let auth_manager = codex_core::test_support::auth_manager_from_auth(auth.clone());
    let mut extension_builder = ExtensionRegistryBuilder::<Config>::new();
    install_web_search_extension(&mut extension_builder, auth_manager);
    let mut builder = test_codex()
        .with_auth(auth)
        .with_extensions(Arc::new(extension_builder.build()))
        .with_config(|config| {
            config
                .features
                .enable(Feature::Sqlite)
                .expect("test config should allow feature update");
            config
                .features
                .enable(Feature::StandaloneWebSearch)
                .expect("standalone web search should be enabled");
            config.memories.disable_on_external_context = true;
            config
                .web_search_mode
                .set(WebSearchMode::Live)
                .expect("web search mode should be accepted");
        });
    let test = builder.build(&server).await?;
    let db = test.codex.state_db().expect("state db enabled");
    let thread_id = test.session_configured.thread_id;

    test.submit_turn("search the web").await?;

    let mut memory_mode = None;
    for _ in 0..100 {
        memory_mode = db.get_thread_memory_mode(thread_id).await?;
        if memory_mode.as_deref() == Some("polluted") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    assert_eq!(memory_mode.as_deref(), Some("polluted"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_call_marks_thread_memory_mode_polluted_when_configured() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "call-123";
    let server_name = "rmcp";
    let namespace = format!("mcp__{server_name}");
    mount_sse_once(
        &server,
        responses::sse(vec![
            ev_response_created("resp-1"),
            responses::ev_function_call_with_namespace(
                call_id,
                &namespace,
                "echo",
                "{\"message\":\"ping\"}",
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "rmcp echo tool completed."),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    let rmcp_test_server_bin = stdio_server_bin()?;
    let mut builder = test_codex().with_config(move |config| {
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
        config.memories.disable_on_external_context = true;

        let mut servers = config.mcp_servers.get().clone();
        servers.insert(
            server_name.to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::Stdio {
                    command: rmcp_test_server_bin,
                    args: Vec::new(),
                    env: Some(HashMap::from([(
                        "MCP_TEST_VALUE".to_string(),
                        "propagated-env".to_string(),
                    )])),
                    env_vars: Vec::new(),
                    cwd: None,
                },
                environment_id: "local".to_string(),
                enabled: true,
                required: false,
                supports_parallel_tool_calls: false,
                disabled_reason: None,
                startup_timeout_sec: Some(Duration::from_secs(10)),
                tool_timeout_sec: None,
                default_tools_approval_mode: None,
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
                oauth: None,
                oauth_resource: None,
                tools: HashMap::new(),
            },
        );
        config
            .mcp_servers
            .set(servers)
            .expect("test mcp servers should accept any configuration");
    });
    let test = builder.build(&server).await?;
    wait_for_mcp_server(&test.codex, server_name).await?;
    let db = test.codex.state_db().expect("state db enabled");
    let thread_id = test.session_configured.thread_id;
    let cwd = test.config.cwd.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::read_only(), cwd.as_path());

    test.codex
        .submit(Op::UserInput {
            submission: UserSubmission {
                items: vec![UserInput::Text {
                    text: "call the rmcp echo tool".to_string(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
                responsesapi_client_metadata: None,
                additional_context: Default::default(),
            },
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(cwd)),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: test.session_configured.model.clone(),
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::McpToolCallEnd(_))
    })
    .await;
    wait_for_event_match(&test.codex, |event| match event {
        EventMsg::Error(err) => Some(Err(anyhow::anyhow!(err.message.clone()))),
        EventMsg::TurnComplete(_) => Some(Ok(())),
        _ => None,
    })
    .await?;

    let mut memory_mode = None;
    for _ in 0..100 {
        memory_mode = db.get_thread_memory_mode(thread_id).await?;
        if memory_mode.as_deref() == Some("polluted") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    assert_eq!(memory_mode.as_deref(), Some("polluted"));
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn tool_call_logs_include_thread_id() -> Result<()> {
    let server = start_mock_server().await;
    let call_id = "call-1";
    let args = json!({
        "command": "echo hello",
        "timeout_ms": 1_000,
        "login": false,
    });
    let args_json = serde_json::to_string(&args)?;
    mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "shell_command", &args_json),
                ev_completed("resp-1"),
            ]),
            responses::sse(vec![ev_completed("resp-2")]),
        ],
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;
    let db = test.codex.state_db().expect("state db enabled");
    let expected_thread_id = test.session_configured.thread_id.to_string();

    test.submit_turn("run a shell command").await?;

    let log_db_layer = codex_state::log_db::start(db.clone());
    let subscriber = tracing_subscriber::registry().with(log_db_layer.clone());
    let dispatch = tracing::Dispatch::new(subscriber);
    tracing::dispatcher::with_default(&dispatch, || {
        let span = tracing::info_span!("test_log_span", thread_id = %expected_thread_id);
        let _entered = span.enter();
        tracing::info!("ToolCall: shell_command {{\"command\":\"echo hello\"}}");
    });
    log_db_layer.flush().await;

    let mut found = None;
    for _ in 0..80 {
        let query = codex_state::LogQuery {
            descending: true,
            limit: Some(20),
            ..Default::default()
        };
        let rows = db.query_logs(&query).await?;
        if let Some(row) = rows.into_iter().find(|row| {
            row.message
                .as_deref()
                .is_some_and(|m| m.contains("ToolCall:"))
        }) {
            let thread_id = row.thread_id;
            let message = row.message;
            found = Some((thread_id, message));
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let (thread_id, message) = found.expect("expected ToolCall log row");
    assert_eq!(thread_id, Some(expected_thread_id));
    assert!(
        message
            .as_deref()
            .is_some_and(|text| text.contains("ToolCall:")),
        "expected ToolCall message, got {message:?}"
    );

    Ok(())
}
