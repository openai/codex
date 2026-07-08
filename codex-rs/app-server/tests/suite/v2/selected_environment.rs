use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::PathBufExt;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
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
    write_mock_responses_config_toml(
        codex_home.path(),
        &server.uri(),
        &BTreeMap::new(),
        /*auto_compact_limit*/ 100_000,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "compact",
    )?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, app_server.initialize()).await??;

    let request_id = app_server
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ThreadStartResponse {
        cwd,
        runtime_workspace_roots,
        active_permission_profile,
        ..
    } = to_response(response)?;
    let host_cwd = codex_home.path().to_path_buf().abs().canonicalize()?;
    let cwd = cwd.canonicalize()?;
    let runtime_workspace_roots = runtime_workspace_roots
        .into_iter()
        .map(|root| root.canonicalize())
        .collect::<std::io::Result<Vec<_>>>()?;
    assert_eq!(
        (cwd, runtime_workspace_roots, active_permission_profile),
        (
            // TODO(anp): Return the selected environment's native cwd from thread/start.
            host_cwd.clone(),
            // TODO(anp): Derive runtime workspace roots from the selected remote environment.
            vec![host_cwd],
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
    write_mock_responses_config_toml(
        codex_home.path(),
        &server.uri(),
        &BTreeMap::new(),
        /*auto_compact_limit*/ 100_000,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "compact",
    )?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, app_server.initialize()).await??;

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

    let request_id = app_server
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ThreadStartResponse = to_response(response)?;

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
    write_mock_responses_config_toml(
        codex_home.path(),
        &server.uri(),
        &BTreeMap::new(),
        /*auto_compact_limit*/ 100_000,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "compact",
    )?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, app_server.initialize()).await??;
    let (environment_cwd, environment_shell) = {
        let auto_env = app_server.auto_env()?;
        (
            auto_env.selection().cwd.clone(),
            auto_env.environment().info().await?.shell.name,
        )
    };

    let request_id = app_server
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ThreadStartResponse {
        thread,
        runtime_workspace_roots,
        ..
    } = to_response(response)?;
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
    let [runtime_workspace_root] = runtime_workspace_roots.as_slice() else {
        anyhow::bail!("expected one runtime workspace root");
    };
    let expected_workspace_roots = format!(
        "<workspace_roots><root>{}</root></workspace_roots>",
        runtime_workspace_root.as_path().display()
    );
    // TODO(anp): Derive model-visible workspace roots from the selected remote environment and
    // render them using its native path convention.
    assert!(environment_context.contains(&expected_workspace_roots));

    Ok(())
}
