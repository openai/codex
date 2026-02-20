use anyhow::Result;
use codex_core::features::Feature;
use codex_protocol::ThreadId;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::UserMessageEvent;
use core_test_support::responses;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::fs;
use tokio::time::Duration;
use tracing_subscriber::prelude::*;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn new_thread_is_recorded_in_state_db() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::Sqlite);
    });
    let test = builder.build(&server).await?;

    let thread_id = test.session_configured.session_id;
    let rollout_path = test.codex.rollout_path().expect("rollout path");
    let db_path = codex_state::state_db_path(test.config.codex_home.as_path());

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
    assert_eq!(metadata.rollout_path, rollout_path);
    assert!(
        rollout_path.exists(),
        "rollout should be materialized after first user message"
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

    let dynamic_tools = vec![
        DynamicToolSpec {
            name: "geo_lookup".to_string(),
            description: "lookup a city".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["city"],
                "properties": { "city": { "type": "string" } }
            }),
        },
        DynamicToolSpec {
            name: "weather_lookup".to_string(),
            description: "lookup weather".to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["zip"],
                "properties": { "zip": { "type": "string" } }
            }),
        },
    ];
    let dynamic_tools_for_hook = dynamic_tools.clone();

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
                    timestamp: "2026-01-27T12:00:00Z".to_string(),
                    cwd: codex_home.to_path_buf(),
                    originator: "test".to_string(),
                    cli_version: "test".to_string(),
                    source: SessionSource::default(),
                    agent_nickname: None,
                    agent_role: None,
                    model_provider: None,
                    base_instructions: None,
                    dynamic_tools: Some(dynamic_tools_for_hook),
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
                        message: "hello from backfill".to_string(),
                        images: None,
                        local_images: Vec::new(),
                        text_elements: Vec::new(),
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
            config.features.enable(Feature::Sqlite);
        });

    let test = builder.build(&server).await?;

    let db_path = codex_state::state_db_path(test.config.codex_home.as_path());
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
    assert_eq!(metadata.rollout_path, rollout_path);
    assert_eq!(metadata.model_provider, default_provider);
    assert!(metadata.first_user_message.is_some());

    let mut stored_tools = None;
    for _ in 0..40 {
        stored_tools = db.get_dynamic_tools(thread_id).await?;
        if stored_tools.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let stored_tools = stored_tools.expect("dynamic tools should be stored");
    assert_eq!(stored_tools, dynamic_tools);

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
        config.features.enable(Feature::Sqlite);
    });
    let test = builder.build(&server).await?;

    let db_path = codex_state::state_db_path(test.config.codex_home.as_path());
    for _ in 0..100 {
        if tokio::fs::try_exists(&db_path).await.unwrap_or(false) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    test.submit_turn("hello from sqlite").await?;
    test.submit_turn("another message").await?;

    let db = test.codex.state_db().expect("state db enabled");
    let thread_id = test.session_configured.session_id;

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
        config.features.enable(Feature::Sqlite);
    });
    let test = builder.build(&server).await?;
    let db = test.codex.state_db().expect("state db enabled");
    let expected_thread_id = test.session_configured.session_id.to_string();

    let subscriber = tracing_subscriber::registry().with(codex_state::log_db::start(db.clone()));
    let dispatch = tracing::Dispatch::new(subscriber);
    let _guard = tracing::dispatcher::set_default(&dispatch);

    test.submit_turn("run a shell command").await?;
    {
        let span = tracing::info_span!("test_log_span", thread_id = %expected_thread_id);
        let _entered = span.enter();
        tracing::info!("ToolCall: shell_command {{\"command\":\"echo hello\"}}");
    }

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
                .is_some_and(|m| m.starts_with("ToolCall:"))
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
            .is_some_and(|text| text.starts_with("ToolCall:")),
        "expected ToolCall message, got {message:?}"
    );

    Ok(())
}
