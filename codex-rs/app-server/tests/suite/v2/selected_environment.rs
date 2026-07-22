use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::PathBufExt;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput as V2UserInput;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const AGENTS_INSTRUCTIONS: &str = "selected environment workspace instructions";
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

fn write_mock_config(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    MockResponsesConfig::new(server_uri)
        .with_root_config("compact_prompt = \"compact\"\nmodel_auto_compact_token_limit = 100000")
        .with_provider_config("supports_websockets = false")
        .write(codex_home)
}

fn text_turn_params(thread_id: String, prompt: &str) -> TurnStartParams {
    TurnStartParams {
        thread_id,
        input: vec![V2UserInput::Text {
            text: prompt.to_string(),
            text_elements: Vec::new(),
        }],
        ..Default::default()
    }
}

#[tokio::test]
async fn thread_start_reports_selected_environment_metadata() -> Result<()> {
    let server = responses::start_mock_server().await;
    let codex_home = TempDir::new()?;
    write_mock_config(codex_home.path(), &server.uri())?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let selected_workspace_roots = app_server
        .auto_env()?
        .selection()
        .workspace_roots
        .iter()
        .filter_map(|root| root.to_abs_path().ok())
        .collect::<Vec<_>>();

    let ThreadStartResponse {
        cwd,
        runtime_workspace_roots,
        active_permission_profile,
        ..
    } = app_server
        .start_thread(ThreadStartParams::default())
        .await?;
    let host_cwd = codex_home.path().to_path_buf().abs().canonicalize()?;
    let cwd = cwd.canonicalize()?;
    assert_eq!(
        (cwd, runtime_workspace_roots, active_permission_profile),
        (
            // TODO(anp): Return the selected environment's native cwd from thread/start.
            host_cwd,
            selected_workspace_roots,
            // TODO(anp): Report the implicit built-in permission profile instead of None.
            None,
        )
    );

    Ok(())
}

#[tokio::test]
async fn thread_start_reports_selected_environment_instruction_source() -> Result<()> {
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
    write_mock_config(codex_home.path(), &server.uri())?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let (agents_source, environment_cwd) = {
        let auto_env = app_server.auto_env()?;
        let environment_cwd = auto_env.selection().cwd.clone();
        let agents_source = environment_cwd.join("AGENTS.md")?;
        auto_env
            .environment()
            .get_filesystem()
            .write_file(
                &agents_source,
                AGENTS_INSTRUCTIONS.as_bytes().to_vec(),
                /*sandbox*/ None,
            )
            .await?;
        (agents_source, environment_cwd)
    };

    let response = app_server
        .start_thread(ThreadStartParams::default())
        .await?;

    assert_eq!(response.instruction_sources, vec![agents_source.into()]);
    timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.start_turn_and_wait_for_completion(text_turn_params(
            response.thread.id,
            "inspect workspace instructions",
        )),
    )
    .await??;

    let user_context = response_mock.single_request().message_input_texts("user");
    let instructions = user_context
        .iter()
        .find(|text| text.starts_with("# AGENTS.md instructions"))
        .context("selected environment instructions should be model visible")?;
    let expected_instructions = format!(
        "# AGENTS.md instructions for {}\n\n<INSTRUCTIONS>\n{AGENTS_INSTRUCTIONS}\n</INSTRUCTIONS>",
        environment_cwd.inferred_native_path_string()
    );
    assert_eq!(instructions, &expected_instructions);

    Ok(())
}

#[tokio::test]
async fn turn_model_context_uses_selected_environment() -> Result<()> {
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
    write_mock_config(codex_home.path(), &server.uri())?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let (environment_cwd, environment_shell) = {
        let auto_env = app_server.auto_env()?;
        (
            auto_env.selection().cwd.clone(),
            auto_env.environment().info().await?.shell.name,
        )
    };

    let thread = app_server
        .start_thread(ThreadStartParams::default())
        .await?
        .thread;
    timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.start_turn_and_wait_for_completion(text_turn_params(
            thread.id,
            "inspect the selected environment",
        )),
    )
    .await??;

    let user_context = response_mock.single_request().message_input_texts("user");
    let environment_context = user_context
        .iter()
        .find(|text| text.starts_with("<environment_context>"))
        .context("selected environment context should be model visible")?;
    let shell = environment_context
        .lines()
        .find(|line| line.trim_start().starts_with("<shell>"))
        .map(str::trim)
        .map(str::to_string);
    let cwd = environment_context
        .lines()
        .find(|line| line.trim_start().starts_with("<cwd>"))
        .map(str::trim)
        .map(str::to_string);
    assert_eq!(
        (shell, cwd),
        (
            Some(format!("<shell>{environment_shell}</shell>")),
            Some(format!(
                "<cwd>{}</cwd>",
                environment_cwd.inferred_native_path_string()
            )),
        )
    );
    Ok(())
}
