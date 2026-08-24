use std::time::Duration;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use codex_config::types::AuthCredentialsStoreMode;
use core_test_support::load_default_config_for_test;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::analytics::mount_analytics_capture;
use super::analytics::wait_for_matching_analytics_event;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::test]
async fn app_server_registers_history_and_notes_tools_for_token_budget_threads() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "done"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_model_provider("openai-custom")
        .with_provider_name("OpenAI")
        .with_provider_base_url(&format!("{}/backend-api/codex", server.uri()))
        .with_provider_config("supports_websockets = false\nrequires_openai_auth = true")
        .with_extra_config(
            "[features.token_budget]\nenabled = true\nuse_history_notes_extension = true",
        )
        .write(codex_home.path())?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("access-chatgpt"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized()
        .await?;
    let thread = app_server
        .start_thread(ThreadStartParams::default())
        .await?
        .thread;
    timeout(
        Duration::from_secs(10),
        app_server.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread.id,
            input: vec![UserInput::Text {
                text: "inspect history and notes".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        }),
    )
    .await??;

    let request = response_mock.single_request();
    for (namespace, tool_name) in [
        ("history", "list_windows"),
        ("history", "list_items"),
        ("history", "read_item"),
        ("history", "search_contents"),
        ("notes", "list_files_by_prefix"),
        ("notes", "read_file"),
        ("notes", "search_contents"),
        ("notes", "append_to_file"),
        ("notes", "write_file"),
    ] {
        assert!(
            request.tool_by_name(namespace, tool_name).is_some(),
            "app-server should expose {namespace}.{tool_name} to the model"
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn history_notes_and_async_message_emit_control_tool_analytics() -> Result<()> {
    let calls = [
        ("history", "list_windows", json!({})),
        ("history", "list_items", json!({})),
        (
            "history",
            "read_item",
            json!({"window_id": "PRIVATE_WINDOW", "item_id": "PRIVATE_ITEM"}),
        ),
        (
            "history",
            "search_contents",
            json!({"query": "PRIVATE_QUERY"}),
        ),
        (
            "notes",
            "list_files_by_prefix",
            json!({"prefix": "PRIVATE_PATH"}),
        ),
        ("notes", "read_file", json!({"path": "PRIVATE_PATH"})),
        (
            "notes",
            "search_contents",
            json!({"query": "PRIVATE_QUERY"}),
        ),
        (
            "notes",
            "append_to_file",
            json!({"path": "PRIVATE_PATH", "text": "PRIVATE_TEXT"}),
        ),
        (
            "notes",
            "write_file",
            json!({"path": "PRIVATE_PATH", "text": "PRIVATE_TEXT"}),
        ),
        (
            "functions",
            "send_user_message_async",
            json!({"message": "PRIVATE_MESSAGE"}),
        ),
        ("notes", "list_files_by_prefix", json!({"max_results": 101})),
        (
            "functions",
            "send_user_message_async",
            json!({"message": " "}),
        ),
    ];
    let server = responses::start_mock_server().await;
    for (namespace, tool, _) in &calls[..9] {
        Mock::given(method("POST"))
            .and(path(format!(
                "/backend-api/codex/alpha/{namespace}/v2/{tool}"
            )))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"text": "PRIVATE_RESULT"})),
            )
            .expect(1)
            .mount(&server)
            .await;
    }
    let mut sequence = calls
        .iter()
        .enumerate()
        .map(|(index, (namespace, tool, args))| {
            let response_id = format!("resp-{index}");
            responses::sse(vec![
                responses::ev_response_created(&response_id),
                responses::ev_function_call_with_namespace(
                    &format!("call-{index}"),
                    namespace,
                    tool,
                    &args.to_string(),
                ),
                responses::ev_completed(&response_id),
            ])
        })
        .collect::<Vec<_>>();
    sequence.push(responses::sse(vec![responses::ev_completed("resp-final")]));
    let response_mock = responses::mount_sse_sequence(&server, sequence).await;

    let codex_home = TempDir::new()?;
    let config = load_default_config_for_test(&codex_home).await;
    let mut model = codex_core::test_support::construct_model_info_offline("mock-model", &config);
    model
        .experimental_supported_tools
        .push("send_user_message_async".to_string());
    let catalog_path = codex_home.path().join("models.json");
    std::fs::write(
        &catalog_path,
        serde_json::to_vec(&json!({"models": [model]}))?,
    )?;
    MockResponsesConfig::new(&server.uri())
        .with_model_provider("openai-custom")
        .with_provider_name("OpenAI")
        .with_provider_base_url(&format!("{}/backend-api/codex", server.uri()))
        .with_provider_config("supports_websockets = false\nrequires_openai_auth = true")
        .with_root_config(&format!("chatgpt_base_url = \"{}\"", server.uri()))
        .with_root_config(&format!(
            "model_catalog_json = {}",
            serde_json::to_string(&catalog_path)?
        ))
        .with_extra_config(
            "[features.token_budget]\nenabled = true\nuse_history_notes_extension = true",
        )
        .write(codex_home.path())?;
    mount_analytics_capture(&server, codex_home.path()).await?;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .without_managed_config()
        .build_initialized()
        .await?;
    let thread = app_server
        .start_thread(ThreadStartParams::default())
        .await?
        .thread;
    let completed = app_server
        .start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "exercise control tools".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;

    for (index, (namespace, tool, _)) in calls.iter().enumerate() {
        let event = wait_for_matching_analytics_event(&server, DEFAULT_READ_TIMEOUT, |event| {
            event["event_type"] == "codex_control_tool_call_event"
                && event["event_params"]["item_id"] == format!("call-{index}")
        })
        .await?;
        let params = &event["event_params"];
        assert_eq!(
            json!({
                "tool": params["tool_name"],
                "thread": params["thread_id"],
                "turn": params["turn_id"],
                "status": params["terminal_status"],
                "origin": params["originating_response_id"],
                "duration": params["execution_duration_ms"].is_u64(),
            }),
            json!({
                "tool": if *namespace == "functions" { tool.to_string() } else { format!("{namespace}.{tool}") },
                "thread": thread.id,
                "turn": completed.turn.id,
                "status": if index < 10 { "completed" } else { "failed" },
                "origin": format!("resp-{index}"),
                "duration": true,
            })
        );
        assert!(!event.to_string().contains("PRIVATE_"));
    }
    let turn_event = wait_for_matching_analytics_event(&server, DEFAULT_READ_TIMEOUT, |event| {
        event["event_type"] == "codex_turn_event"
            && event["event_params"]["turn_id"] == completed.turn.id
    })
    .await?;
    assert_eq!(
        json!({
            "total": turn_event["event_params"]["total_tool_call_count"],
            "dynamic": turn_event["event_params"]["dynamic_tool_call_count"],
        }),
        json!({"total": calls.len(), "dynamic": 0})
    );
    assert_eq!(
        response_mock.requests()[10].function_call_output_text("call-9"),
        Some(r#"{"accepted":true}"#.to_string())
    );

    Ok(())
}
