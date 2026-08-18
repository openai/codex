use std::sync::Arc;

use codex_core::StartThreadOptions;
use codex_core::ThreadManager;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_core::config::ExtraConfig;
use codex_core::config::ThreadStoreConfig;
use codex_core::resolve_installation_id;
use codex_core::thread_store_from_config;
use codex_exec_server::EnvironmentManager;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_login::CodexAuth;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use core_test_support::PathBufExt;
use core_test_support::load_default_config_for_test;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::sync::oneshot;

const PERSISTENT_MODE_MESSAGE: &str = "continue the persistent task";

type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

fn chunk(event: Value) -> StreamingSseChunk {
    StreamingSseChunk {
        gate: None,
        body: responses::sse(vec![event]),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stores_config_and_starts_configured_turn_after_idle() -> TestResult {
    assert_persistent_turn_after_idle("gpt-5.6-sol", &[PERSISTENT_MODE_MESSAGE]).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn nathree_continues_after_final_without_injecting_a_user_message() -> TestResult {
    assert_persistent_turn_after_idle("nathree", &[]).await
}

async fn assert_persistent_turn_after_idle(
    model: &str,
    expected_continuation_messages: &[&str],
) -> TestResult {
    let (release_second_turn_tx, release_second_turn_rx) = oneshot::channel();
    let first_turn = vec![
        chunk(ev_response_created("resp-1")),
        chunk(ev_assistant_message("msg-1", "done")),
        chunk(ev_completed("resp-1")),
    ];
    let second_turn = vec![
        chunk(ev_response_created("resp-2")),
        StreamingSseChunk {
            gate: Some(release_second_turn_rx),
            body: responses::sse(vec![ev_completed("resp-2")]),
        },
    ];
    let (server, _completions) = start_streaming_sse_server(vec![first_turn, second_turn]).await;

    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model = Some(model.to_string());
    config.cwd = cwd.path().to_path_buf().abs();
    config.model_provider.base_url = Some(format!("{}/v1", server.uri()));
    config.model_provider.supports_websockets = false;
    config.extra_config = Some(ExtraConfig {
        persistent_mode_message: Some(PERSISTENT_MODE_MESSAGE.to_string()),
    });
    config.experimental_thread_store = ThreadStoreConfig::InMemory {
        id: uuid::Uuid::new_v4().to_string(),
    };

    let state_db = codex_core::init_state_db(&config).await;
    let thread_store = thread_store_from_config(&config, state_db.clone());
    let installation_id = resolve_installation_id(&config.codex_home).await?;
    let auth_manager =
        codex_core::test_support::auth_manager_from_auth(CodexAuth::from_api_key("dummy"));
    let thread_manager = Arc::new_cyclic(|thread_manager| {
        let extensions = {
            let mut registry = ExtensionRegistryBuilder::<Config>::new();
            codex_internal_persistent_mode::install(&mut registry, thread_manager.clone());
            Arc::new(registry.build())
        };
        ThreadManager::new(
            &config,
            Arc::clone(&auth_manager),
            codex_core::build_models_manager(&config, Arc::clone(&auth_manager)),
            codex_core::CodexAppsToolsCache::default(),
            SessionSource::Exec,
            Arc::new(EnvironmentManager::default_for_tests()),
            extensions,
            Arc::new(codex_core::test_support::EmptyUserInstructionsProvider),
            /*analytics_events_client*/ None,
            Arc::clone(&thread_store),
            codex_core::local_agent_graph_store_from_state_db(state_db.as_ref()),
            installation_id.clone(),
            /*attestation_provider*/ None,
            /*external_time_provider*/ None,
        )
    });
    let codex = thread_manager
        .start_thread(StartThreadOptions::new(config))
        .await?
        .thread;
    let stored_thread = codex
        .read_thread(
            /*include_archived*/ true, /*include_history*/ false,
        )
        .await?;
    assert_eq!(
        stored_thread.extra_config,
        Some(ExtraConfig {
            persistent_mode_message: Some(PERSISTENT_MODE_MESSAGE.to_string()),
        })
    );

    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "start".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;

    server.wait_for_request_count(/*count*/ 2).await;
    let requests = server.requests().await;
    assert_eq!(requests.len(), 2);
    let first_request: Value = serde_json::from_slice(&requests[0])?;
    let second_request: Value = serde_json::from_slice(&requests[1])?;
    assert!(first_request["input"].is_array());
    assert!(second_request["input"].is_array());
    let user_messages = |request: &Value| {
        request["input"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|item| item["role"] == "user")
            .map(|item| item["content"].clone())
            .collect::<Vec<_>>()
    };
    let mut expected_user_messages = user_messages(&first_request);
    expected_user_messages.extend(
        expected_continuation_messages
            .iter()
            .map(|message| json!([{"type": "input_text", "text": message}])),
    );
    assert_eq!(user_messages(&second_request), expected_user_messages);
    assert!(
        second_request["input"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|item| item["role"] == "assistant"
                && item["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|content| content["text"] == "done")),
        "persistent continuation should retain the preceding assistant response"
    );

    codex.submit(Op::Shutdown).await?;
    wait_for_event(&codex, |event| matches!(event, EventMsg::ShutdownComplete)).await;
    drop(release_second_turn_tx);
    server.shutdown().await;
    Ok(())
}
