//! Exercise incremental tool updates across a persisted old-to-new app-server resume.

use anyhow::Context;
use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resume_preserves_prefix_and_appends_changed_tool_declaration() -> Result<()> {
    let release = std::env::var_os("CONTRACT_EXECUTOR_BIN").map(std::path::PathBuf::from);
    let server = responses::start_mock_server().await;
    let home = TempDir::new()?;
    let catalog_path = home.path().join("models.json");
    let mut catalog = codex_models_manager::bundled_models_response()?;
    let model_index = catalog
        .models
        .iter()
        .position(|model| model.slug == "gpt-6-astra")
        .context("bundled catalog must contain the test model")?;
    // Force a real declaration change even when the released catalog catches up.
    // The separate fresh-start test still compares each binary's bundled defaults.
    catalog.models[model_index].description = Some("Before the app-server upgrade".into());
    std::fs::write(&catalog_path, serde_json::to_vec(&catalog)?)?;
    MockResponsesConfig::new(&server.uri())
        .with_model("gpt-6-astra")
        .enable_feature(Feature::IncrementalTools)
        .disable_feature(Feature::ModelCatalogInContext)
        .with_root_config(&format!(
            "skills.bundled.enabled = false\nmodel_catalog_json = {}\nmulti_agent_v2.expose_spawn_agent_model_overrides = true",
            serde_json::to_string(&catalog_path)?
        ))
        .write(home.path())?;
    let builder = || {
        TestAppServer::builder()
            .with_codex_home(home.path())
            .with_plugin_startup_tasks()
            .without_managed_config()
            .with_env_overrides(&[
                ("CODEX_APP_SERVER_MANAGED_CONFIG_PATH", None),
                ("CODEX_APP_SERVER_DISABLE_MANAGED_CONFIG", None),
            ])
    };
    let mut old_builder = builder();
    if let Some(program) = release.as_deref() {
        old_builder = old_builder.with_program(program).with_args(&["app-server"]);
    }
    let initial_response = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("before-upgrade"),
            responses::ev_assistant_message("before-message", "Before upgrade"),
            responses::ev_completed("before-upgrade"),
        ]),
    )
    .await;
    let mut old_app = old_builder.build_initialized().await?;
    let thread = old_app
        .start_thread(ThreadStartParams::default())
        .await?
        .thread;
    let completed = old_app
        .start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "Before upgrade".into(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    let initial_request = initial_response.single_request().body_json();
    let initial_input = initial_request["input"]
        .as_array()
        .context("initial request input")?;
    let initial_declarations = spawn_agent_declarations(initial_input);
    let [initial_tool] = initial_declarations.as_slice() else {
        anyhow::bail!("expected one initial incremental spawn_agent declaration");
    };
    assert!(
        initial_tool["description"]
            .as_str()
            .context("initial spawn_agent description")?
            .contains("Before the app-server upgrade")
    );
    // Finish persistence and retire the old process before the new binary resumes.
    assert!(old_app.shutdown_gracefully().await?.success());

    catalog.models[model_index].description = Some("After the app-server upgrade".into());
    std::fs::write(&catalog_path, serde_json::to_vec(&catalog)?)?;
    server.reset().await;
    let resumed_response = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("after-upgrade"),
            responses::ev_assistant_message("after-message", "After upgrade"),
            responses::ev_completed("after-upgrade"),
        ]),
    )
    .await;
    let mut app = builder().build_initialized().await?;
    let resumed: ThreadResumeResponse = app
        .request(|request_id| ClientRequest::ThreadResume {
            request_id,
            params: ThreadResumeParams {
                thread_id: thread.id.clone(),
                ..Default::default()
            },
        })
        .await?;
    assert_eq!(resumed.thread.id, thread.id);
    let completed = app
        .start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread.id,
            input: vec![UserInput::Text {
                text: "After upgrade".into(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    let resumed_request = resumed_response.single_request().body_json();
    let resumed_input = resumed_request["input"]
        .as_array()
        .context("resumed request input")?;
    let retained = resumed_input
        .get(..initial_input.len())
        .context("resume must retain the entire previous request prefix")?;
    assert_eq!(
        json!({
            "instructions": initial_request.get("instructions"),
            "tools": initial_request.get("tools"),
            "input": initial_input,
        }),
        json!({
            "instructions": resumed_request.get("instructions"),
            "tools": resumed_request.get("tools"),
            "input": retained,
        }),
        "resuming on the new binary must preserve the previous inference prefix"
    );
    let appended_declarations = spawn_agent_declarations(&resumed_input[initial_input.len()..]);
    let [updated_tool] = appended_declarations.as_slice() else {
        anyhow::bail!("expected one appended spawn_agent declaration after the retained prefix");
    };
    let updated_description = updated_tool["description"]
        .as_str()
        .context("updated spawn_agent description")?;
    assert!(updated_description.contains("After the app-server upgrade"));
    assert!(!updated_description.contains("Before the app-server upgrade"));
    assert!(app.shutdown_gracefully().await?.success());
    Ok(())
}

fn spawn_agent_declarations(input: &[Value]) -> Vec<&Value> {
    input
        .iter()
        .filter(|item| item["type"] == "additional_tools")
        .filter_map(|item| responses::namespace_child_tool(item, "collaboration", "spawn_agent"))
        .collect()
}
