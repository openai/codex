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
use core_test_support::responses;
use tempfile::TempDir;
use tokio::time::timeout;

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
