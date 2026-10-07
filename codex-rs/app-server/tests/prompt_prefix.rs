//! Compare model-visible prefixes from two app-server versions using the same client and inputs.
//! The contract runner supplies a released CLI; ordinary runs check same-version determinism.

use anyhow::Context;
use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prompt_prefix_matches_release() -> Result<()> {
    let release = std::env::var_os("CONTRACT_EXECUTOR_BIN").map(std::path::PathBuf::from);
    let mut prefixes = Vec::new();

    for program in [release.as_deref(), None] {
        let server = responses::start_mock_server().await;
        let response = responses::mount_sse_once(
            &server,
            responses::sse(vec![
                responses::ev_response_created("prefix-response"),
                responses::ev_assistant_message("prefix-message", "Done"),
                responses::ev_completed("prefix-response"),
            ]),
        )
        .await;
        let home = TempDir::new()?;
        // Use each binary's bundled model catalog and feature defaults. Supplying a
        // catalog or base instructions from the test would hide changes to them.
        MockResponsesConfig::new(&server.uri())
            .with_model("gpt-6-astra")
            .write(home.path())?;
        let mut builder = TestAppServer::builder()
            .with_codex_home(home.path())
            .with_plugin_startup_tasks()
            .without_managed_config();
        if release.is_some() {
            // Released CLIs ignore the debug-only managed-config overrides.
            // Give both versions the same host configuration in version-skew runs.
            builder = builder.with_env_overrides(&[
                ("CODEX_APP_SERVER_MANAGED_CONFIG_PATH", None),
                ("CODEX_APP_SERVER_DISABLE_MANAGED_CONFIG", None),
            ]);
        }
        if let Some(program) = program {
            // Reuse the exact package downloaded for executor version-skew tests.
            // Keep it in place so its own bundled resources remain discoverable.
            builder = builder.with_program(program).with_args(&["app-server"]);
        }
        let mut app = builder.build_initialized().await?;
        let thread = app.start_thread(ThreadStartParams::default()).await?.thread;
        let completed = app
            .start_turn_and_wait_for_completion(TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![UserInput::Text {
                    text: "prefix-contract-user-message".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            })
            .await?;
        assert_eq!(completed.turn.status, TurnStatus::Completed);
        let request = response.single_request();
        let body = request.body_json();
        let input = responses::strip_response_item_ids_from_json(json!(request.input()));
        let input = input.as_array().context("request input must be an array")?;
        let user_index = input
            .iter()
            .position(|item| {
                item["role"] == "user"
                    && item["content"][0]["text"] == "prefix-contract-user-message"
            })
            .context("request must contain the submitted user message")?;
        let prefix = responses::strip_metadata_from_json(json!({
            "instructions": body.get("instructions"),
            "tools": body.get("tools"),
            "input": &input[..user_index],
        }));
        // Normalize only values created by this fixture, retaining all prompt
        // text, whitespace, tool descriptions, schemas, and array ordering.
        let mut prefix = serde_json::to_string_pretty(&prefix)?;
        let environment = app.auto_env_params()?;
        for (value, replacement) in [
            (environment.cwd.as_str(), "<ENVIRONMENT_CWD>"),
            (thread.cwd.to_string_lossy().as_ref(), "<CWD>"),
            (home.path().to_string_lossy().as_ref(), "<CODEX_HOME>"),
            (thread.id.as_str(), "<THREAD_ID>"),
            (thread.session_id.as_str(), "<SESSION_ID>"),
        ] {
            // Windows paths can be rendered with either separator, notably in
            // skill roots. Normalize both spellings of these fixture values only.
            for spelling in [value.to_string(), value.replace('\\', "/")] {
                let escaped = serde_json::to_string(&spelling)?;
                prefix = prefix.replace(&escaped[1..escaped.len() - 1], replacement);
            }
        }
        prefixes.push(prefix);
    }

    assert_eq!(
        prefixes[0], prefixes[1],
        "the default inference prefix differs from the released version; gate the change behind a default-off feature key"
    );
    Ok(())
}
