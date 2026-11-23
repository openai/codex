//! Shared real app-server and mock-inference fixture for remote CLI tests.

use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_final_assistant_message_sse_response;
use codex_app_server_client::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY;
use codex_app_server_client::RemoteAppServerClient;
use codex_app_server_client::RemoteAppServerConnectArgs;
use codex_app_server_client::RemoteAppServerEndpoint;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use core_test_support::responses;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use tokio::process::Command;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockGuard;
use wiremock::MockServer;
use wiremock::matchers::method;
use wiremock::matchers::path;

pub struct RemoteExecFixture {
    pub endpoint: String,
    pub caller_home: TempDir,
    pub requests: MockGuard,
    codex: PathBuf,
    _server: TestAppServer,
    _model: MockServer,
}

impl RemoteExecFixture {
    pub async fn start(
        codex: &Path,
        server_home: &Path,
        response: &str,
        expected_requests: u64,
        configure: impl FnOnce(MockResponsesConfig) -> MockResponsesConfig,
        env_overrides: &[(&str, Option<&str>)],
    ) -> Result<(Self, RemoteAppServerClient, String)> {
        let model = responses::start_mock_server().await;
        let requests = Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .respond_with(responses::sse_response(
                create_final_assistant_message_sse_response(response)?,
            ))
            .expect(expected_requests)
            .mount_as_scoped(&model)
            .await;
        configure(
            MockResponsesConfig::new(&model.uri())
                .with_root_config("features.plugins = false\nanalytics.enabled = false"),
        )
        .write(server_home)?;
        let server = TestAppServer::builder()
            .with_program(codex)
            .with_codex_home(server_home)
            .with_plugin_startup_tasks()
            .without_managed_config()
            .with_args(&["app-server", "--listen", "unix://"])
            .with_env_overrides(env_overrides)
            .build()
            .await?;
        let socket_path = codex_app_server::app_server_control_socket_path(server_home)?;
        timeout(Duration::from_secs(30), async {
            while !socket_path.as_path().try_exists()? {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            anyhow::Ok(())
        })
        .await??;
        let endpoint = format!("unix://{}", socket_path.as_path().display());
        let app = RemoteAppServerClient::connect(RemoteAppServerConnectArgs {
            required_human_approval: false,
            endpoint: RemoteAppServerEndpoint::UnixSocket { socket_path },
            client_name: "remote-cli-e2e-test".to_string(),
            client_version: "0.1.0".to_string(),
            experimental_api: true,
            mcp_server_openai_form_elicitation: false,
            opt_out_notification_methods: Vec::new(),
            channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
        })
        .await?;
        let started: ThreadStartResponse = app
            .request_typed(ClientRequest::ThreadStart {
                request_id: RequestId::Integer(1),
                params: ThreadStartParams::default(),
            })
            .await?;
        Ok((
            Self {
                endpoint,
                caller_home: TempDir::new()?,
                requests,
                codex: codex.to_path_buf(),
                _server: server,
                _model: model,
            },
            app,
            started.thread.id,
        ))
    }

    pub fn caller_command(&self) -> Command {
        let mut command = Command::new(&self.codex);
        command
            .env("CODEX_HOME", self.caller_home.path())
            .current_dir(self.caller_home.path())
            .kill_on_drop(true);
        command
    }
}
