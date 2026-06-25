use std::collections::BTreeMap;
use std::path::PathBuf;
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
use codex_utils_path_uri::PathUri;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::MockServer;

const AGENTS_INSTRUCTIONS: &str = "selected environment workspace instructions";
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

struct SelectedEnvironmentFixture {
    app_server: TestAppServer,
    agents_source: PathUri,
    environment_cwd: PathUri,
    environment_shell: String,
    response_mock: responses::ResponseMock,
    codex_home: TempDir,
    _server: MockServer,
}

impl SelectedEnvironmentFixture {
    async fn new() -> Result<Self> {
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

        let mut app_server = TestAppServer::new_with_auto_env(codex_home.path()).await?;
        timeout(DEFAULT_READ_TIMEOUT, app_server.initialize()).await??;

        let (agents_source, environment_cwd, environment_shell) = {
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
            let environment_shell = auto_env.environment().info().await?.shell.name;
            (agents_source, environment_cwd, environment_shell)
        };

        Ok(Self {
            app_server,
            agents_source,
            environment_cwd,
            environment_shell,
            response_mock,
            codex_home,
            _server: server,
        })
    }

    async fn start_thread(&mut self) -> Result<ThreadStartResponse> {
        let request_id = self
            .app_server
            .send_thread_start_request_with_auto_env(ThreadStartParams::default())
            .await?;
        let response: JSONRPCResponse = timeout(
            DEFAULT_READ_TIMEOUT,
            self.app_server
                .read_stream_until_response_message(RequestId::Integer(request_id)),
        )
        .await??;
        to_response(response)
    }
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
    let mut fixture = SelectedEnvironmentFixture::new().await?;
    let ThreadStartResponse {
        cwd,
        runtime_workspace_roots,
        active_permission_profile,
        ..
    } = fixture.start_thread().await?;
    let host_cwd = fixture
        .codex_home
        .path()
        .to_path_buf()
        .abs()
        .canonicalize()?;
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
    let mut fixture = SelectedEnvironmentFixture::new().await?;
    let response = fixture.start_thread().await?;

    assert_eq!(
        response.instruction_sources,
        vec![fixture.agents_source.clone().into()]
    );
    timeout(
        DEFAULT_READ_TIMEOUT,
        fixture
            .app_server
            .start_turn_and_wait_for_completion(text_turn_params(
                response.thread.id,
                "inspect workspace instructions",
            )),
    )
    .await??;

    let user_context = fixture
        .response_mock
        .single_request()
        .message_input_texts("user");
    let instructions = user_context
        .iter()
        .find(|text| text.starts_with("# AGENTS.md instructions"))
        .context("selected environment instructions should be model visible")?;
    let expected_instructions = format!(
        "# AGENTS.md instructions for {}\n\n<INSTRUCTIONS>\n{AGENTS_INSTRUCTIONS}\n</INSTRUCTIONS>",
        fixture.environment_cwd.inferred_native_path_string()
    );
    assert_eq!(instructions, &expected_instructions);

    Ok(())
}

#[tokio::test]
async fn turn_model_context_uses_selected_environment() -> Result<()> {
    let mut fixture = SelectedEnvironmentFixture::new().await?;
    let thread = fixture.start_thread().await?.thread;
    timeout(
        DEFAULT_READ_TIMEOUT,
        fixture
            .app_server
            .start_turn_and_wait_for_completion(text_turn_params(
                thread.id,
                "inspect the selected environment",
            )),
    )
    .await??;

    let user_context = fixture
        .response_mock
        .single_request()
        .message_input_texts("user");
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
            Some(format!("<shell>{}</shell>", fixture.environment_shell)),
            Some(format!(
                "<cwd>{}</cwd>",
                fixture.environment_cwd.inferred_native_path_string()
            )),
        )
    );
    let host_cwd = fixture
        .codex_home
        .path()
        .to_path_buf()
        .abs()
        .canonicalize()?;
    let model_workspace_root = environment_context
        .split_once("<workspace_roots><root>")
        .and_then(|(_, rest)| rest.split_once("</root></workspace_roots>"))
        .map(|(root, _)| {
            // Decode ampersands last so entity-like path text stays literal.
            PathBuf::from(
                root.replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&quot;", "\"")
                    .replace("&apos;", "'")
                    .replace("&amp;", "&"),
            )
            .abs()
        })
        .context("model context should include a workspace root")?
        .canonicalize()?;
    // TODO(anp): Derive model-visible workspace roots from the selected remote environment and
    // render them using its native path convention.
    assert_eq!(model_workspace_root, host_cwd);

    Ok(())
}
