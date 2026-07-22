use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::process::Stdio;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;

use anyhow::Context;
use anyhow::ensure;
use codex_app_server_protocol::AppsInstalledParams;
use codex_app_server_protocol::AppsListParams;
use codex_app_server_protocol::AppsReadParams;
use codex_app_server_protocol::CancelLoginAccountParams;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientNotification;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CollaborationModeListParams;
use codex_app_server_protocol::CommandExecParams;
use codex_app_server_protocol::CommandExecResizeParams;
use codex_app_server_protocol::CommandExecTerminateParams;
use codex_app_server_protocol::CommandExecWriteParams;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::ConsumeAccountRateLimitResetCreditParams;
use codex_app_server_protocol::ExperimentalFeatureListParams;
use codex_app_server_protocol::FsCopyParams;
use codex_app_server_protocol::FsCreateDirectoryParams;
use codex_app_server_protocol::FsGetMetadataParams;
use codex_app_server_protocol::FsReadDirectoryParams;
use codex_app_server_protocol::FsReadFileParams;
use codex_app_server_protocol::FsRemoveParams;
use codex_app_server_protocol::FsUnwatchParams;
use codex_app_server_protocol::FsWatchParams;
use codex_app_server_protocol::FsWriteFileParams;
use codex_app_server_protocol::GetAccountParams;
use codex_app_server_protocol::GetAuthStatusParams;
use codex_app_server_protocol::GetConversationSummaryParams;
use codex_app_server_protocol::HooksListParams;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::ListMcpServerStatusParams;
use codex_app_server_protocol::LoginAccountParams;
use codex_app_server_protocol::MarketplaceAddParams;
use codex_app_server_protocol::MarketplaceRemoveParams;
use codex_app_server_protocol::MarketplaceUpgradeParams;
use codex_app_server_protocol::McpResourceReadParams;
use codex_app_server_protocol::McpServerToolCallParams;
use codex_app_server_protocol::MockExperimentalMethodParams;
use codex_app_server_protocol::ModelListParams;
use codex_app_server_protocol::ModelProviderCapabilitiesReadParams;
use codex_app_server_protocol::PermissionProfileListParams;
use codex_app_server_protocol::PluginInstallParams;
use codex_app_server_protocol::PluginInstalledParams;
use codex_app_server_protocol::PluginListParams;
use codex_app_server_protocol::PluginReadParams;
use codex_app_server_protocol::PluginSkillReadParams;
use codex_app_server_protocol::PluginUninstallParams;
use codex_app_server_protocol::ProcessKillParams;
use codex_app_server_protocol::ProcessSpawnParams;
use codex_app_server_protocol::RemoteControlClientsListParams;
use codex_app_server_protocol::RemoteControlClientsRevokeParams;
use codex_app_server_protocol::RemoteControlPairingStartParams;
use codex_app_server_protocol::RemoteControlPairingStatusParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ReviewStartParams;
use codex_app_server_protocol::SendAddCreditsNudgeEmailParams;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::SkillsExtraRootsSetParams;
use codex_app_server_protocol::SkillsListParams;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadCompactStartParams;
use codex_app_server_protocol::ThreadDeleteParams;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadInjectItemsParams;
use codex_app_server_protocol::ThreadItemsListParams;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadMemoryModeSetParams;
use codex_app_server_protocol::ThreadMetadataUpdateParams;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadRealtimeAppendAudioParams;
use codex_app_server_protocol::ThreadRealtimeAppendSpeechParams;
use codex_app_server_protocol::ThreadRealtimeAppendTextParams;
use codex_app_server_protocol::ThreadRealtimeListVoicesParams;
use codex_app_server_protocol::ThreadRealtimeStartParams;
use codex_app_server_protocol::ThreadRealtimeStopParams;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadRollbackParams;
use codex_app_server_protocol::ThreadSearchOccurrencesParams;
use codex_app_server_protocol::ThreadSearchParams;
use codex_app_server_protocol::ThreadSetNameParams;
use codex_app_server_protocol::ThreadSettingsUpdateParams;
use codex_app_server_protocol::ThreadShellCommandParams;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadTurnsListParams;
use codex_app_server_protocol::ThreadUnarchiveParams;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnEnvironmentParams;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnSteerParams;
use codex_app_server_protocol::WindowsSandboxSetupStartParams;
use codex_exec_server::CODEX_EXEC_SERVER_NOISE_AUTH_TOKEN_ENV_VAR;
use codex_exec_server::CODEX_EXEC_SERVER_NOISE_CHATGPT_ACCOUNT_ID_ENV_VAR;
use codex_exec_server::CODEX_EXEC_SERVER_NOISE_ENVIRONMENT_ID_ENV_VAR;
use codex_exec_server::CODEX_EXEC_SERVER_NOISE_REGISTRY_URL_ENV_VAR;
use codex_exec_server::CODEX_EXEC_SERVER_URL_ENV_VAR;
use codex_login::default_client::CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR;
use core_test_support::is_remote_test_environment;
use core_test_support::test_codex::TestEnv;
use core_test_support::test_codex::test_env;
use serde::de::DeserializeOwned;
use tempfile::TempDir;
use tokio::process::Command;

use crate::json_logging::JsonLogCapture;
use crate::local_websocket_exec_server::LocalWebsocketExecServer;
use crate::rpc_delay::WebsocketDelayInterposer;

pub struct TestAppServer {
    next_request_id: AtomicI64,
    /// Retain this child process until the client is dropped. The Tokio runtime
    /// will make a "best effort" to reap the process after it exits, but it is
    /// not a guarantee. See the `kill_on_drop` documentation for details.
    #[allow(dead_code)]
    process: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    pending_messages: VecDeque<JSONRPCMessage>,
    auto_env: Option<TestEnv>,
    json_logs: JsonLogCapture,
    // Fields drop in declaration order. Tear down the delayed child before
    // removing an owned CODEX_HOME that may still be its cwd on Windows.
    _delayed_exec_server: Option<(LocalWebsocketExecServer, WebsocketDelayInterposer)>,
    _owned_codex_home: Option<TempDir>,
}

pub const DEFAULT_CLIENT_NAME: &str = "codex-app-server-tests";
pub const DISABLE_PLUGIN_STARTUP_TASKS_ARG: &str = "--disable-plugin-startup-tasks-for-tests";
const DISABLE_MANAGED_CONFIG_ENV_VAR: &str = "CODEX_APP_SERVER_DISABLE_MANAGED_CONFIG";
const CODE_MODE_HOST_PATH_ENV_VAR: &str = "CODEX_CODE_MODE_HOST_PATH";
#[cfg(windows)]
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(25);
#[cfg(not(windows))]
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

impl TestAppServer {
    /// Starts building a server with a temporary CODEX_HOME and the standard
    /// automatic test environment.
    pub fn builder() -> TestAppServerBuilder {
        TestAppServerBuilder {
            codex_home: None,
            environment: TestAppServerEnvironment::Auto,
            program: None,
            env_overrides: Vec::new(),
            args: vec![DISABLE_PLUGIN_STARTUP_TASKS_ARG.to_string()],
            exec_server_delay: None,
        }
    }

    pub async fn wait_for_exit(&mut self) -> std::io::Result<ExitStatus> {
        self.process.wait().await
    }

    /// Closes stdio and waits for app-server's graceful thread teardown to finish.
    pub async fn shutdown_gracefully(&mut self) -> std::io::Result<ExitStatus> {
        drop(self.stdin.take());
        self.process.wait().await
    }

    /// Returns the automatically selected test environment retained by this server.
    ///
    /// Tests can use the environment to arrange target-native filesystem fixtures before starting
    /// a thread. Returns an error unless the builder's automatic environment is enabled.
    pub fn auto_env(&self) -> anyhow::Result<&TestEnv> {
        self.auto_env
            .as_ref()
            .context("auto environment is unavailable; enable it on TestAppServer::builder")
    }

    /// Returns app-server protocol parameters for the automatically selected
    /// test environment. Returns an error unless the builder's automatic
    /// environment is enabled.
    pub fn auto_env_params(&self) -> anyhow::Result<TurnEnvironmentParams> {
        let selection = self.auto_env()?.selection();
        Ok(TurnEnvironmentParams {
            environment_id: selection.environment_id.clone(),
            cwd: selection.cwd.clone().into(),
            runtime_workspace_roots: None,
        })
    }

    /// Waits for a JSON stderr event whose structured `event.name` field matches.
    pub async fn wait_for_json_log_event(
        &self,
        event_name: &str,
    ) -> anyhow::Result<serde_json::Value> {
        self.json_logs.wait_for_event(event_name).await
    }

    async fn new_with_program_env_and_args(
        codex_home: &Path,
        program: &Path,
        env_overrides: &[(&str, Option<&str>)],
        args: &[&str],
    ) -> anyhow::Result<Self> {
        let mut cmd = Command::new(program);

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.current_dir(codex_home);
        cmd.env("CODEX_HOME", codex_home);
        cmd.env("RUST_LOG", "warn");
        // Keep integration tests isolated from host managed configuration.
        cmd.env(
            "CODEX_APP_SERVER_MANAGED_CONFIG_PATH",
            codex_home.join("managed_config.toml"),
        );
        cmd.env_remove(CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR);
        cmd.args(args);

        for (k, v) in env_overrides {
            match v {
                Some(val) => {
                    cmd.env(k, val);
                }
                None => {
                    cmd.env_remove(k);
                }
            }
        }

        let mut process = cmd
            .kill_on_drop(true)
            .spawn()
            .context("codex-mcp-server proc should start")?;
        let stdin = process
            .stdin
            .take()
            .ok_or_else(|| anyhow::format_err!("mcp should have stdin fd"))?;
        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| anyhow::format_err!("mcp should have stdout fd"))?;
        let stdout = BufReader::new(stdout);

        // Forward child's stderr to our stderr so failures are visible even
        // when stdout/stderr are captured by the test harness.
        let json_logs = JsonLogCapture::default();
        if let Some(stderr) = process.stderr.take() {
            let json_logs = json_logs.clone();
            let mut stderr_reader = BufReader::new(stderr).lines();
            tokio::spawn(async move {
                while let Ok(Some(line)) = stderr_reader.next_line().await {
                    json_logs.record(line.clone());
                    eprintln!("[mcp stderr] {line}");
                }
            });
        }
        Ok(Self {
            next_request_id: AtomicI64::new(0),
            process,
            stdin: Some(stdin),
            stdout,
            pending_messages: VecDeque::new(),
            auto_env: None,
            json_logs,
            _delayed_exec_server: None,
            _owned_codex_home: None,
        })
    }

    /// Performs the initialization handshake with the MCP server.
    pub async fn initialize(&mut self) -> anyhow::Result<()> {
        let initialized = self
            .initialize_with_client_info(ClientInfo {
                name: DEFAULT_CLIENT_NAME.to_string(),
                title: None,
                version: "0.1.0".to_string(),
            })
            .await?;
        let JSONRPCMessage::Response(_) = initialized else {
            unreachable!("expected JSONRPCMessage::Response for initialize, got {initialized:?}");
        };
        Ok(())
    }

    /// Sends initialize with the provided client info and returns the response/error message.
    pub async fn initialize_with_client_info(
        &mut self,
        client_info: ClientInfo,
    ) -> anyhow::Result<JSONRPCMessage> {
        self.initialize_with_capabilities(
            client_info,
            Some(InitializeCapabilities {
                experimental_api: true,
                ..Default::default()
            }),
        )
        .await
    }

    pub async fn initialize_with_capabilities(
        &mut self,
        client_info: ClientInfo,
        capabilities: Option<InitializeCapabilities>,
    ) -> anyhow::Result<JSONRPCMessage> {
        self.initialize_with_params(InitializeParams {
            client_info,
            capabilities,
        })
        .await
    }

    async fn initialize_with_params(
        &mut self,
        params: InitializeParams,
    ) -> anyhow::Result<JSONRPCMessage> {
        let params = Some(serde_json::to_value(params)?);
        let request_id = self.send_request("initialize", params).await?;
        let message = self.read_jsonrpc_message().await?;
        match message {
            JSONRPCMessage::Response(response) => {
                if response.id != RequestId::Integer(request_id) {
                    anyhow::bail!(
                        "initialize response id mismatch: expected {}, got {:?}",
                        request_id,
                        response.id
                    );
                }

                // Send notifications/initialized to ack the response.
                self.send_notification(ClientNotification::Initialized)
                    .await?;

                Ok(JSONRPCMessage::Response(response))
            }
            JSONRPCMessage::Error(error) => {
                if error.id != RequestId::Integer(request_id) {
                    anyhow::bail!(
                        "initialize error id mismatch: expected {}, got {:?}",
                        request_id,
                        error.id
                    );
                }
                Ok(JSONRPCMessage::Error(error))
            }
            JSONRPCMessage::Notification(notification) => {
                anyhow::bail!("unexpected JSONRPCMessage::Notification: {notification:?}");
            }
            JSONRPCMessage::Request(request) => {
                anyhow::bail!("unexpected JSONRPCMessage::Request: {request:?}");
            }
        }
    }

    /// Send a `getAuthStatus` JSON-RPC request.
    pub async fn send_get_auth_status_request(
        &mut self,
        params: GetAuthStatusParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("getAuthStatus", params).await
    }

    /// Send a `getConversationSummary` JSON-RPC request.
    pub async fn send_get_conversation_summary_request(
        &mut self,
        params: GetConversationSummaryParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("getConversationSummary", params).await
    }

    /// Send an `account/rateLimits/read` JSON-RPC request.
    pub async fn send_get_account_rate_limits_request(&mut self) -> anyhow::Result<i64> {
        self.send_request("account/rateLimits/read", /*params*/ None)
            .await
    }

    /// Send an `account/rateLimitResetCredit/consume` JSON-RPC request.
    pub async fn send_consume_account_rate_limit_reset_credit_request(
        &mut self,
        params: ConsumeAccountRateLimitResetCreditParams,
    ) -> anyhow::Result<i64> {
        self.send_request(
            "account/rateLimitResetCredit/consume",
            Some(serde_json::to_value(params)?),
        )
        .await
    }

    /// Send an `account/sendAddCreditsNudgeEmail` JSON-RPC request.
    pub async fn send_add_credits_nudge_email_request(
        &mut self,
        params: SendAddCreditsNudgeEmailParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("account/sendAddCreditsNudgeEmail", params)
            .await
    }

    /// Send an `account/read` JSON-RPC request.
    pub async fn send_get_account_request(
        &mut self,
        params: GetAccountParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("account/read", params).await
    }

    /// Send an `account/login/start` JSON-RPC request with ChatGPT auth tokens.
    pub async fn send_chatgpt_auth_tokens_login_request(
        &mut self,
        access_token: String,
        chatgpt_account_id: String,
        chatgpt_plan_type: Option<String>,
    ) -> anyhow::Result<i64> {
        let params = LoginAccountParams::ChatgptAuthTokens {
            access_token,
            chatgpt_account_id,
            chatgpt_plan_type,
        };
        self.send_login_account_request(serde_json::to_value(params)?)
            .await
    }

    /// Send a `thread/start` JSON-RPC request.
    pub async fn send_thread_start_request(
        &mut self,
        params: ThreadStartParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/start", params).await
    }

    /// Sends a `thread/start` request selecting the builder's automatic
    /// environment. Returns an error if `params` already select environments
    /// so the caller cannot accidentally override the fixture.
    pub async fn send_thread_start_request_with_auto_env(
        &mut self,
        mut params: ThreadStartParams,
    ) -> anyhow::Result<i64> {
        ensure!(
            params.environments.is_none(),
            "send_thread_start_request_with_auto_env requires params.environments to be omitted"
        );
        params.environments = Some(vec![self.auto_env_params()?]);
        self.send_thread_start_request(params).await
    }

    /// Starts a thread using the standard automatic test environment.
    pub async fn start_thread(
        &mut self,
        params: ThreadStartParams,
    ) -> anyhow::Result<ThreadStartResponse> {
        let request_id = self.send_thread_start_request_with_auto_env(params).await?;
        tokio::time::timeout(DEFAULT_REQUEST_TIMEOUT, self.read_response(request_id)).await?
    }

    /// Send a `thread/resume` JSON-RPC request.
    pub async fn send_thread_resume_request(
        &mut self,
        params: ThreadResumeParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/resume", params).await
    }

    /// Send a `thread/fork` JSON-RPC request.
    pub async fn send_thread_fork_request(
        &mut self,
        params: ThreadForkParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/fork", params).await
    }

    /// Send a `thread/archive` JSON-RPC request.
    pub async fn send_thread_archive_request(
        &mut self,
        params: ThreadArchiveParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/archive", params).await
    }

    /// Send a `thread/delete` JSON-RPC request.
    pub async fn send_thread_delete_request(
        &mut self,
        params: ThreadDeleteParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/delete", params).await
    }

    /// Send a `thread/name/set` JSON-RPC request.
    pub async fn send_thread_set_name_request(
        &mut self,
        params: ThreadSetNameParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/name/set", params).await
    }

    /// Send a `thread/metadata/update` JSON-RPC request.
    pub async fn send_thread_metadata_update_request(
        &mut self,
        params: ThreadMetadataUpdateParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/metadata/update", params).await
    }

    /// Send a `thread/settings/update` JSON-RPC request.
    pub async fn send_thread_settings_update_request(
        &mut self,
        params: ThreadSettingsUpdateParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/settings/update", params).await
    }

    /// Send a `thread/unsubscribe` JSON-RPC request.
    pub async fn send_thread_unsubscribe_request(
        &mut self,
        params: ThreadUnsubscribeParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/unsubscribe", params).await
    }

    /// Send a `thread/unarchive` JSON-RPC request.
    pub async fn send_thread_unarchive_request(
        &mut self,
        params: ThreadUnarchiveParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/unarchive", params).await
    }

    /// Send a `thread/compact/start` JSON-RPC request.
    pub async fn send_thread_compact_start_request(
        &mut self,
        params: ThreadCompactStartParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/compact/start", params).await
    }

    /// Send a `thread/shellCommand` JSON-RPC request.
    pub async fn send_thread_shell_command_request(
        &mut self,
        params: ThreadShellCommandParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/shellCommand", params).await
    }

    /// Send a `thread/rollback` JSON-RPC request.
    pub async fn send_thread_rollback_request(
        &mut self,
        params: ThreadRollbackParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/rollback", params).await
    }

    /// Send a `thread/list` JSON-RPC request.
    pub async fn send_thread_list_request(
        &mut self,
        params: ThreadListParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/list", params).await
    }

    /// Send a `thread/search` JSON-RPC request.
    pub async fn send_thread_search_request(
        &mut self,
        params: ThreadSearchParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/search", params).await
    }

    /// Send a `thread/searchOccurrences` JSON-RPC request.
    pub async fn send_thread_search_occurrences_request(
        &mut self,
        params: ThreadSearchOccurrencesParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/searchOccurrences", params).await
    }

    /// Send a `thread/loaded/list` JSON-RPC request.
    pub async fn send_thread_loaded_list_request(
        &mut self,
        params: ThreadLoadedListParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/loaded/list", params).await
    }

    /// Send a `thread/read` JSON-RPC request.
    pub async fn send_thread_read_request(
        &mut self,
        params: ThreadReadParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/read", params).await
    }

    /// Send a `thread/turns/list` JSON-RPC request.
    pub async fn send_thread_turns_list_request(
        &mut self,
        params: ThreadTurnsListParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/turns/list", params).await
    }

    /// Send a `thread/items/list` JSON-RPC request.
    pub async fn send_thread_items_list_request(
        &mut self,
        params: ThreadItemsListParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/items/list", params).await
    }

    /// Send a `model/list` JSON-RPC request.
    pub async fn send_list_models_request(
        &mut self,
        params: ModelListParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("model/list", params).await
    }

    /// Send a `modelProvider/capabilities/read` JSON-RPC request.
    pub async fn send_model_provider_capabilities_read_request(
        &mut self,
        params: ModelProviderCapabilitiesReadParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("modelProvider/capabilities/read", params)
            .await
    }

    /// Send an `experimentalFeature/list` JSON-RPC request.
    pub async fn send_experimental_feature_list_request(
        &mut self,
        params: ExperimentalFeatureListParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("experimentalFeature/list", params).await
    }

    /// Send a `permissionProfile/list` JSON-RPC request.
    pub async fn send_permission_profile_list_request(
        &mut self,
        params: PermissionProfileListParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("permissionProfile/list", params).await
    }

    /// Send an `experimentalFeature/enablement/set` JSON-RPC request.
    pub async fn send_experimental_feature_enablement_set_request(
        &mut self,
        params: codex_app_server_protocol::ExperimentalFeatureEnablementSetParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("experimentalFeature/enablement/set", params)
            .await
    }

    /// Send a `remoteControl/enable` JSON-RPC request.
    pub async fn send_remote_control_enable_request(&mut self) -> anyhow::Result<i64> {
        self.send_request("remoteControl/enable", /*params*/ None)
            .await
    }

    /// Send a runtime-only `remoteControl/enable` JSON-RPC request.
    pub async fn send_remote_control_ephemeral_enable_request(&mut self) -> anyhow::Result<i64> {
        self.send_request(
            "remoteControl/enable",
            Some(serde_json::json!({ "ephemeral": true })),
        )
        .await
    }

    /// Send a `remoteControl/disable` JSON-RPC request.
    pub async fn send_remote_control_disable_request(&mut self) -> anyhow::Result<i64> {
        self.send_request("remoteControl/disable", /*params*/ None)
            .await
    }

    /// Send a runtime-only `remoteControl/disable` JSON-RPC request.
    pub async fn send_remote_control_ephemeral_disable_request(&mut self) -> anyhow::Result<i64> {
        self.send_request(
            "remoteControl/disable",
            Some(serde_json::json!({ "ephemeral": true })),
        )
        .await
    }

    /// Send a `remoteControl/status/read` JSON-RPC request.
    pub async fn send_remote_control_status_read_request(&mut self) -> anyhow::Result<i64> {
        self.send_request("remoteControl/status/read", /*params*/ None)
            .await
    }

    /// Send a `remoteControl/pairing/start` JSON-RPC request.
    pub async fn send_remote_control_pairing_start_request(
        &mut self,
        params: RemoteControlPairingStartParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("remoteControl/pairing/start", params)
            .await
    }

    /// Send a `remoteControl/pairing/status` JSON-RPC request.
    pub async fn send_remote_control_pairing_status_request(
        &mut self,
        params: RemoteControlPairingStatusParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("remoteControl/pairing/status", params)
            .await
    }

    /// Send a `remoteControl/client/list` JSON-RPC request.
    pub async fn send_remote_control_clients_list_request(
        &mut self,
        params: RemoteControlClientsListParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("remoteControl/client/list", params).await
    }

    /// Send a `remoteControl/client/revoke` JSON-RPC request.
    pub async fn send_remote_control_clients_revoke_request(
        &mut self,
        params: RemoteControlClientsRevokeParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("remoteControl/client/revoke", params)
            .await
    }

    /// Send an `app/list` JSON-RPC request.
    pub async fn send_apps_list_request(&mut self, params: AppsListParams) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("app/list", params).await
    }

    /// Send an `app/installed` JSON-RPC request.
    pub async fn send_apps_installed_request(
        &mut self,
        params: AppsInstalledParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("app/installed", params).await
    }

    /// Send an `app/read` JSON-RPC request.
    pub async fn send_apps_read_request(&mut self, params: AppsReadParams) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("app/read", params).await
    }

    /// Send an `mcpServer/resource/read` JSON-RPC request.
    pub async fn send_mcp_resource_read_request(
        &mut self,
        params: McpResourceReadParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("mcpServer/resource/read", params).await
    }

    /// Send an `mcpServer/tool/call` JSON-RPC request.
    pub async fn send_mcp_server_tool_call_request(
        &mut self,
        params: McpServerToolCallParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("mcpServer/tool/call", params).await
    }

    /// Send a `skills/list` JSON-RPC request.
    pub async fn send_skills_list_request(
        &mut self,
        params: SkillsListParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("skills/list", params).await
    }

    /// Send a `skills/extraRoots/set` JSON-RPC request.
    pub async fn send_skills_extra_roots_set_request(
        &mut self,
        params: SkillsExtraRootsSetParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("skills/extraRoots/set", params).await
    }

    /// Send a `hooks/list` JSON-RPC request.
    pub async fn send_hooks_list_request(
        &mut self,
        params: HooksListParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("hooks/list", params).await
    }

    /// Send a `marketplace/add` JSON-RPC request.
    pub async fn send_marketplace_add_request(
        &mut self,
        params: MarketplaceAddParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("marketplace/add", params).await
    }

    /// Send a `marketplace/remove` JSON-RPC request.
    pub async fn send_marketplace_remove_request(
        &mut self,
        params: MarketplaceRemoveParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("marketplace/remove", params).await
    }

    /// Send a `marketplace/upgrade` JSON-RPC request.
    pub async fn send_marketplace_upgrade_request(
        &mut self,
        params: MarketplaceUpgradeParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("marketplace/upgrade", params).await
    }

    /// Send a `plugin/install` JSON-RPC request.
    pub async fn send_plugin_install_request(
        &mut self,
        params: PluginInstallParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("plugin/install", params).await
    }

    /// Send a `plugin/uninstall` JSON-RPC request.
    pub async fn send_plugin_uninstall_request(
        &mut self,
        params: PluginUninstallParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("plugin/uninstall", params).await
    }

    /// Send a `plugin/list` JSON-RPC request.
    pub async fn send_plugin_list_request(
        &mut self,
        params: PluginListParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("plugin/list", params).await
    }

    /// Send a `plugin/installed` JSON-RPC request.
    pub async fn send_plugin_installed_request(
        &mut self,
        params: PluginInstalledParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("plugin/installed", params).await
    }

    /// Send a `plugin/read` JSON-RPC request.
    pub async fn send_plugin_read_request(
        &mut self,
        params: PluginReadParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("plugin/read", params).await
    }

    /// Send a `plugin/skill/read` JSON-RPC request.
    pub async fn send_plugin_skill_read_request(
        &mut self,
        params: PluginSkillReadParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("plugin/skill/read", params).await
    }

    /// Send an `mcpServerStatus/list` JSON-RPC request.
    pub async fn send_list_mcp_server_status_request(
        &mut self,
        params: ListMcpServerStatusParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("mcpServerStatus/list", params).await
    }

    /// Send a JSON-RPC request with raw params for protocol-level validation tests.
    pub async fn send_raw_request(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> anyhow::Result<i64> {
        self.send_request(method, params).await
    }
    /// Send a `collaborationMode/list` JSON-RPC request.
    pub async fn send_list_collaboration_modes_request(
        &mut self,
        params: CollaborationModeListParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("collaborationMode/list", params).await
    }

    /// Send a `mock/experimentalMethod` JSON-RPC request.
    pub async fn send_mock_experimental_method_request(
        &mut self,
        params: MockExperimentalMethodParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("mock/experimentalMethod", params).await
    }

    /// Send a `thread/memoryMode/set` JSON-RPC request (v2, experimental).
    pub async fn send_thread_memory_mode_set_request(
        &mut self,
        params: ThreadMemoryModeSetParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/memoryMode/set", params).await
    }

    /// Send a `turn/start` JSON-RPC request (v2).
    pub async fn send_turn_start_request(
        &mut self,
        params: TurnStartParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("turn/start", params).await
    }

    /// Start a turn and return its matching typed completion notification.
    pub async fn start_turn_and_wait_for_completion(
        &mut self,
        params: TurnStartParams,
    ) -> anyhow::Result<TurnCompletedNotification> {
        let thread_id = params.thread_id.clone();
        let request_id = self.send_turn_start_request(params).await?;
        let response = self
            .read_stream_until_response_message(RequestId::Integer(request_id))
            .await?;
        let TurnStartResponse { turn } = crate::to_response(response)?;
        let notification = self
            .read_stream_until_matching_notification(
                "turn/completed for started turn",
                |notification| {
                    notification.method == "turn/completed"
                        && notification.params.as_ref().is_some_and(|params| {
                            serde_json::from_value::<TurnCompletedNotification>(params.clone())
                                .is_ok_and(|completed| {
                                    completed.thread_id == thread_id && completed.turn.id == turn.id
                                })
                        })
                },
            )
            .await?;
        let params = notification
            .params
            .context("turn/completed notification must include params")?;
        let completed = serde_json::from_value(params)
            .context("failed to deserialize turn/completed notification")?;
        Ok(completed)
    }

    /// Send a `thread/inject_items` JSON-RPC request (v2).
    pub async fn send_thread_inject_items_request(
        &mut self,
        params: ThreadInjectItemsParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/inject_items", params).await
    }

    /// Send a `command/exec` JSON-RPC request (v2).
    pub async fn send_command_exec_request(
        &mut self,
        params: CommandExecParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("command/exec", params).await
    }

    /// Send a `process/spawn` JSON-RPC request (v2).
    pub async fn send_process_spawn_request(
        &mut self,
        params: ProcessSpawnParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("process/spawn", params).await
    }

    /// Send a `process/kill` JSON-RPC request (v2).
    pub async fn send_process_kill_request(
        &mut self,
        params: ProcessKillParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("process/kill", params).await
    }

    /// Send a `command/exec/write` JSON-RPC request (v2).
    pub async fn send_command_exec_write_request(
        &mut self,
        params: CommandExecWriteParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("command/exec/write", params).await
    }

    /// Send a `command/exec/resize` JSON-RPC request (v2).
    pub async fn send_command_exec_resize_request(
        &mut self,
        params: CommandExecResizeParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("command/exec/resize", params).await
    }

    /// Send a `command/exec/terminate` JSON-RPC request (v2).
    pub async fn send_command_exec_terminate_request(
        &mut self,
        params: CommandExecTerminateParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("command/exec/terminate", params).await
    }

    /// Send a `turn/interrupt` JSON-RPC request (v2).
    pub async fn send_turn_interrupt_request(
        &mut self,
        params: TurnInterruptParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("turn/interrupt", params).await
    }

    /// Send a `thread/realtime/start` JSON-RPC request (v2).
    pub async fn send_thread_realtime_start_request(
        &mut self,
        params: ThreadRealtimeStartParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/realtime/start", params).await
    }

    /// Send a `thread/realtime/appendAudio` JSON-RPC request (v2).
    pub async fn send_thread_realtime_append_audio_request(
        &mut self,
        params: ThreadRealtimeAppendAudioParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/realtime/appendAudio", params)
            .await
    }

    /// Send a `thread/realtime/appendText` JSON-RPC request (v2).
    pub async fn send_thread_realtime_append_text_request(
        &mut self,
        params: ThreadRealtimeAppendTextParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/realtime/appendText", params)
            .await
    }

    /// Send a `thread/realtime/appendSpeech` JSON-RPC request (v2).
    pub async fn send_thread_realtime_append_speech_request(
        &mut self,
        params: ThreadRealtimeAppendSpeechParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/realtime/appendSpeech", params)
            .await
    }

    /// Send a `thread/realtime/stop` JSON-RPC request (v2).
    pub async fn send_thread_realtime_stop_request(
        &mut self,
        params: ThreadRealtimeStopParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/realtime/stop", params).await
    }

    pub async fn send_thread_realtime_list_voices_request(
        &mut self,
        params: ThreadRealtimeListVoicesParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/realtime/listVoices", params)
            .await
    }

    /// Deterministically clean up an intentionally in-flight turn.
    ///
    /// Some tests assert behavior while a turn is still running. Returning from those tests
    /// without an explicit interrupt + terminal turn notification wait can leave in-flight work
    /// racing teardown and intermittently show up as `LEAK` in nextest.
    ///
    /// In rare races, the turn can also fail or complete on its own after we send
    /// `turn/interrupt` but before the server emits the interrupt response. The helper treats a
    /// buffered matching `turn/completed` notification as sufficient terminal cleanup in that
    /// case so teardown does not flap on timing.
    pub async fn interrupt_turn_and_wait_for_aborted(
        &mut self,
        thread_id: String,
        turn_id: String,
        read_timeout: std::time::Duration,
    ) -> anyhow::Result<()> {
        let interrupt_request_id = self
            .send_turn_interrupt_request(TurnInterruptParams {
                thread_id: thread_id.clone(),
                turn_id: turn_id.clone(),
            })
            .await?;
        match tokio::time::timeout(
            read_timeout,
            self.read_stream_until_response_message(RequestId::Integer(interrupt_request_id)),
        )
        .await
        {
            Ok(result) => {
                result.with_context(|| "failed while waiting for turn interrupt response")?;
            }
            Err(err) => {
                if self.pending_turn_completed_notification(&thread_id, &turn_id) {
                    return Ok(());
                }
                return Err(err).with_context(|| "timed out waiting for turn interrupt response");
            }
        }
        match tokio::time::timeout(
            read_timeout,
            self.read_stream_until_notification_message("turn/completed"),
        )
        .await
        {
            Ok(result) => {
                result.with_context(|| "failed while waiting for terminal turn notification")?;
            }
            Err(err) => {
                if self.pending_turn_completed_notification(&thread_id, &turn_id) {
                    return Ok(());
                }
                return Err(err)
                    .with_context(|| "timed out waiting for terminal turn notification");
            }
        }
        Ok(())
    }

    /// Send a `turn/steer` JSON-RPC request (v2).
    pub async fn send_turn_steer_request(
        &mut self,
        params: TurnSteerParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("turn/steer", params).await
    }

    /// Send a `review/start` JSON-RPC request (v2).
    pub async fn send_review_start_request(
        &mut self,
        params: ReviewStartParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("review/start", params).await
    }

    pub async fn send_windows_sandbox_setup_start_request(
        &mut self,
        params: WindowsSandboxSetupStartParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("windowsSandbox/setupStart", params).await
    }

    pub async fn send_config_read_request(
        &mut self,
        params: ConfigReadParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("config/read", params).await
    }

    pub async fn send_config_requirements_read_request(&mut self) -> anyhow::Result<i64> {
        self.send_request("configRequirements/read", /*params*/ None)
            .await
    }

    pub async fn send_config_value_write_request(
        &mut self,
        params: ConfigValueWriteParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("config/value/write", params).await
    }

    pub async fn send_config_batch_write_request(
        &mut self,
        params: ConfigBatchWriteParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("config/batchWrite", params).await
    }

    pub async fn send_fs_read_file_request(
        &mut self,
        params: FsReadFileParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("fs/readFile", params).await
    }

    pub async fn send_fs_write_file_request(
        &mut self,
        params: FsWriteFileParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("fs/writeFile", params).await
    }

    pub async fn send_fs_create_directory_request(
        &mut self,
        params: FsCreateDirectoryParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("fs/createDirectory", params).await
    }

    pub async fn send_fs_get_metadata_request(
        &mut self,
        params: FsGetMetadataParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("fs/getMetadata", params).await
    }

    pub async fn send_fs_read_directory_request(
        &mut self,
        params: FsReadDirectoryParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("fs/readDirectory", params).await
    }

    pub async fn send_fs_remove_request(&mut self, params: FsRemoveParams) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("fs/remove", params).await
    }

    pub async fn send_fs_copy_request(&mut self, params: FsCopyParams) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("fs/copy", params).await
    }

    pub async fn send_fs_watch_request(&mut self, params: FsWatchParams) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("fs/watch", params).await
    }

    pub async fn send_fs_unwatch_request(
        &mut self,
        params: FsUnwatchParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("fs/unwatch", params).await
    }

    /// Send an `account/logout` JSON-RPC request.
    pub async fn send_logout_account_request(&mut self) -> anyhow::Result<i64> {
        self.send_request("account/logout", /*params*/ None).await
    }

    /// Send an `account/login/start` JSON-RPC request.
    pub async fn send_login_account_request(
        &mut self,
        params: serde_json::Value,
    ) -> anyhow::Result<i64> {
        self.send_request("account/login/start", Some(params)).await
    }

    /// Send an `account/login/start` JSON-RPC request for API key login.
    pub async fn send_login_account_api_key_request(
        &mut self,
        api_key: &str,
    ) -> anyhow::Result<i64> {
        let params = serde_json::json!({
            "type": "apiKey",
            "apiKey": api_key,
        });
        self.send_login_account_request(params).await
    }

    /// Send an `account/login/start` JSON-RPC request for managed Amazon Bedrock login.
    pub async fn send_login_account_amazon_bedrock_request(
        &mut self,
        api_key: &str,
        region: &str,
    ) -> anyhow::Result<i64> {
        let params = serde_json::json!({
            "type": "amazonBedrock",
            "apiKey": api_key,
            "region": region,
        });
        self.send_request("account/login/start", Some(params)).await
    }

    /// Send an `account/login/start` JSON-RPC request for ChatGPT login.
    pub async fn send_login_account_chatgpt_request(&mut self) -> anyhow::Result<i64> {
        let params = serde_json::json!({
            "type": "chatgpt"
        });
        self.send_login_account_request(params).await
    }

    /// Send an `account/login/start` JSON-RPC request for ChatGPT device code login.
    pub async fn send_login_account_chatgpt_device_code_request(&mut self) -> anyhow::Result<i64> {
        let params = serde_json::json!({
            "type": "chatgptDeviceCode"
        });
        self.send_login_account_request(params).await
    }

    /// Send an `account/login/cancel` JSON-RPC request.
    pub async fn send_cancel_login_account_request(
        &mut self,
        params: CancelLoginAccountParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("account/login/cancel", params).await
    }

    /// Send a `fuzzyFileSearch` JSON-RPC request.
    pub async fn send_fuzzy_file_search_request(
        &mut self,
        query: &str,
        roots: Vec<String>,
        cancellation_token: Option<String>,
    ) -> anyhow::Result<i64> {
        let mut params = serde_json::json!({
            "query": query,
            "roots": roots,
        });
        if let Some(token) = cancellation_token {
            params["cancellationToken"] = serde_json::json!(token);
        }
        self.send_request("fuzzyFileSearch", Some(params)).await
    }

    pub async fn send_fuzzy_file_search_session_start_request(
        &mut self,
        session_id: &str,
        roots: Vec<String>,
    ) -> anyhow::Result<i64> {
        let params = serde_json::json!({
            "sessionId": session_id,
            "roots": roots,
        });
        self.send_request("fuzzyFileSearch/sessionStart", Some(params))
            .await
    }

    pub async fn start_fuzzy_file_search_session(
        &mut self,
        session_id: &str,
        roots: Vec<String>,
    ) -> anyhow::Result<JSONRPCResponse> {
        let request_id = self
            .send_fuzzy_file_search_session_start_request(session_id, roots)
            .await?;
        self.read_stream_until_response_message(RequestId::Integer(request_id))
            .await
    }

    pub async fn send_fuzzy_file_search_session_update_request(
        &mut self,
        session_id: &str,
        query: &str,
    ) -> anyhow::Result<i64> {
        let params = serde_json::json!({
            "sessionId": session_id,
            "query": query,
        });
        self.send_request("fuzzyFileSearch/sessionUpdate", Some(params))
            .await
    }

    pub async fn update_fuzzy_file_search_session(
        &mut self,
        session_id: &str,
        query: &str,
    ) -> anyhow::Result<JSONRPCResponse> {
        let request_id = self
            .send_fuzzy_file_search_session_update_request(session_id, query)
            .await?;
        self.read_stream_until_response_message(RequestId::Integer(request_id))
            .await
    }

    pub async fn send_fuzzy_file_search_session_stop_request(
        &mut self,
        session_id: &str,
    ) -> anyhow::Result<i64> {
        let params = serde_json::json!({
            "sessionId": session_id,
        });
        self.send_request("fuzzyFileSearch/sessionStop", Some(params))
            .await
    }

    pub async fn stop_fuzzy_file_search_session(
        &mut self,
        session_id: &str,
    ) -> anyhow::Result<JSONRPCResponse> {
        let request_id = self
            .send_fuzzy_file_search_session_stop_request(session_id)
            .await?;
        self.read_stream_until_response_message(RequestId::Integer(request_id))
            .await
    }

    /// Sends a typed protocol request and waits for its deserialized response.
    ///
    /// The request builder receives a fresh ID so tests do not need to manage
    /// the JSON-RPC request ID themselves.
    pub async fn request<T: DeserializeOwned>(
        &mut self,
        make_request: impl FnOnce(RequestId) -> ClientRequest,
    ) -> anyhow::Result<T> {
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let request = make_request(RequestId::Integer(request_id));
        ensure!(
            request.id() == &RequestId::Integer(request_id),
            "typed request must use the supplied request ID"
        );
        let request = serde_json::from_value::<JSONRPCRequest>(serde_json::to_value(request)?)?;
        self.send_jsonrpc_message(JSONRPCMessage::Request(request))
            .await?;
        tokio::time::timeout(DEFAULT_REQUEST_TIMEOUT, self.read_response(request_id)).await?
    }

    async fn send_request(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> anyhow::Result<i64> {
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);

        let message = JSONRPCMessage::Request(JSONRPCRequest {
            id: RequestId::Integer(request_id),
            method: method.to_string(),
            params,
            trace: None,
        });
        self.send_jsonrpc_message(message).await?;
        Ok(request_id)
    }

    pub async fn send_response(
        &mut self,
        id: RequestId,
        result: serde_json::Value,
    ) -> anyhow::Result<()> {
        self.send_jsonrpc_message(JSONRPCMessage::Response(JSONRPCResponse { id, result }))
            .await
    }

    pub async fn send_error(
        &mut self,
        id: RequestId,
        error: JSONRPCErrorError,
    ) -> anyhow::Result<()> {
        self.send_jsonrpc_message(JSONRPCMessage::Error(JSONRPCError { id, error }))
            .await
    }

    pub async fn send_notification(
        &mut self,
        notification: ClientNotification,
    ) -> anyhow::Result<()> {
        let value = serde_json::to_value(notification)?;
        self.send_jsonrpc_message(JSONRPCMessage::Notification(JSONRPCNotification {
            method: value
                .get("method")
                .and_then(|m| m.as_str())
                .ok_or_else(|| anyhow::format_err!("notification missing method field"))?
                .to_string(),
            params: value.get("params").cloned(),
        }))
        .await
    }

    async fn send_jsonrpc_message(&mut self, message: JSONRPCMessage) -> anyhow::Result<()> {
        eprintln!("writing message to stdin: {message:?}");
        let Some(stdin) = self.stdin.as_mut() else {
            anyhow::bail!("mcp stdin closed");
        };
        let payload = serde_json::to_string(&message)?;
        stdin.write_all(payload.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        Ok(())
    }

    async fn read_jsonrpc_message(&mut self) -> anyhow::Result<JSONRPCMessage> {
        let mut line = String::new();
        self.stdout.read_line(&mut line).await?;
        let message = serde_json::from_str::<JSONRPCMessage>(&line)?;
        eprintln!("read message from stdout: {message:?}");
        Ok(message)
    }

    pub async fn read_stream_until_request_message(&mut self) -> anyhow::Result<ServerRequest> {
        eprintln!("in read_stream_until_request_message()");

        let message = self
            .read_stream_until_message(|message| matches!(message, JSONRPCMessage::Request(_)))
            .await?;

        let JSONRPCMessage::Request(jsonrpc_request) = message else {
            unreachable!("expected JSONRPCMessage::Request, got {message:?}");
        };
        jsonrpc_request
            .try_into()
            .with_context(|| "failed to deserialize ServerRequest from JSONRPCRequest")
    }

    pub async fn read_stream_until_response_message(
        &mut self,
        request_id: RequestId,
    ) -> anyhow::Result<JSONRPCResponse> {
        eprintln!("in read_stream_until_response_message({request_id:?})");

        let message = self
            .read_stream_until_message(|message| {
                Self::message_request_id(message) == Some(&request_id)
            })
            .await?;

        let JSONRPCMessage::Response(response) = message else {
            unreachable!("expected JSONRPCMessage::Response, got {message:?}");
        };
        Ok(response)
    }

    /// Reads and deserializes the successful response for an integer request ID.
    ///
    /// This does not impose a timeout, so callers can retain suite-specific
    /// timeout policies when requests need different latency budgets.
    pub async fn read_response<T: DeserializeOwned>(
        &mut self,
        request_id: i64,
    ) -> anyhow::Result<T> {
        let response = self
            .read_stream_until_response_message(RequestId::Integer(request_id))
            .await?;
        serde_json::from_value(response.result)
            .with_context(|| format!("failed to deserialize response for request {request_id}"))
    }

    pub async fn read_stream_until_error_message(
        &mut self,
        request_id: RequestId,
    ) -> anyhow::Result<JSONRPCError> {
        let message = self
            .read_stream_until_message(|message| {
                Self::message_request_id(message) == Some(&request_id)
            })
            .await?;

        let JSONRPCMessage::Error(err) = message else {
            unreachable!("expected JSONRPCMessage::Error, got {message:?}");
        };
        Ok(err)
    }

    pub async fn read_stream_until_notification_message(
        &mut self,
        method: &str,
    ) -> anyhow::Result<JSONRPCNotification> {
        eprintln!("in read_stream_until_notification_message({method})");

        let message = self
            .read_stream_until_message(|message| {
                matches!(
                    message,
                    JSONRPCMessage::Notification(notification) if notification.method == method
                )
            })
            .await?;

        let JSONRPCMessage::Notification(notification) = message else {
            unreachable!("expected JSONRPCMessage::Notification, got {message:?}");
        };
        Ok(notification)
    }

    /// Reads and deserializes the parameters of the next matching notification.
    ///
    /// This does not impose a timeout, so callers can retain suite-specific
    /// timeout policies when notifications need different latency budgets.
    pub async fn read_notification<T: DeserializeOwned>(
        &mut self,
        method: &str,
    ) -> anyhow::Result<T> {
        let notification = self.read_stream_until_notification_message(method).await?;
        let params = notification
            .params
            .with_context(|| format!("notification `{method}` is missing parameters"))?;
        serde_json::from_value(params)
            .with_context(|| format!("failed to deserialize notification `{method}`"))
    }

    pub async fn read_stream_until_matching_notification<F>(
        &mut self,
        description: &str,
        predicate: F,
    ) -> anyhow::Result<JSONRPCNotification>
    where
        F: Fn(&JSONRPCNotification) -> bool,
    {
        eprintln!("in read_stream_until_matching_notification({description})");

        let message = self
            .read_stream_until_message(|message| {
                matches!(
                    message,
                    JSONRPCMessage::Notification(notification) if predicate(notification)
                )
            })
            .await?;

        let JSONRPCMessage::Notification(notification) = message else {
            unreachable!("expected JSONRPCMessage::Notification, got {message:?}");
        };
        Ok(notification)
    }

    pub async fn read_next_message(&mut self) -> anyhow::Result<JSONRPCMessage> {
        self.read_stream_until_message(|_| true).await
    }

    /// Clears any buffered messages so future reads only consider new stream items.
    ///
    /// We call this when e.g. we want to validate against the next turn and no longer care about
    /// messages buffered from the prior turn.
    pub fn clear_message_buffer(&mut self) {
        self.pending_messages.clear();
    }

    pub fn pending_notification_methods(&self) -> Vec<String> {
        self.pending_messages
            .iter()
            .filter_map(|message| match message {
                JSONRPCMessage::Notification(notification) => Some(notification.method.clone()),
                _ => None,
            })
            .collect()
    }

    /// Reads the stream until a message matches `predicate`, buffering any non-matching messages
    /// for later reads.
    async fn read_stream_until_message<F>(&mut self, predicate: F) -> anyhow::Result<JSONRPCMessage>
    where
        F: Fn(&JSONRPCMessage) -> bool,
    {
        if let Some(message) = self.take_pending_message(&predicate) {
            return Ok(message);
        }

        loop {
            let message = self.read_jsonrpc_message().await?;
            if predicate(&message) {
                return Ok(message);
            }
            self.pending_messages.push_back(message);
        }
    }

    fn take_pending_message<F>(&mut self, predicate: &F) -> Option<JSONRPCMessage>
    where
        F: Fn(&JSONRPCMessage) -> bool,
    {
        if let Some(pos) = self.pending_messages.iter().position(predicate) {
            return self.pending_messages.remove(pos);
        }
        None
    }

    fn pending_turn_completed_notification(&self, thread_id: &str, turn_id: &str) -> bool {
        self.pending_messages.iter().any(|message| {
            let JSONRPCMessage::Notification(notification) = message else {
                return false;
            };
            if notification.method != "turn/completed" {
                return false;
            }
            let Some(params) = notification.params.as_ref() else {
                return false;
            };
            let Ok(payload) = serde_json::from_value::<TurnCompletedNotification>(params.clone())
            else {
                return false;
            };
            payload.thread_id == thread_id && payload.turn.id == turn_id
        })
    }

    fn message_request_id(message: &JSONRPCMessage) -> Option<&RequestId> {
        match message {
            JSONRPCMessage::Request(request) => Some(&request.id),
            JSONRPCMessage::Response(response) => Some(&response.id),
            JSONRPCMessage::Error(err) => Some(&err.id),
            JSONRPCMessage::Notification(_) => None,
        }
    }
}

/// Builder for TestAppServer.
pub struct TestAppServerBuilder {
    codex_home: Option<PathBuf>,
    environment: TestAppServerEnvironment,
    program: Option<PathBuf>,
    env_overrides: Vec<(String, Option<String>)>,
    args: Vec<String>,
    exec_server_delay: Option<Duration>,
}

enum TestAppServerEnvironment {
    Auto,
    None,
}

impl TestAppServerBuilder {
    /// Uses this existing CODEX_HOME instead of a temporary one.
    pub fn with_codex_home(mut self, codex_home: &Path) -> Self {
        self.codex_home = Some(codex_home.to_path_buf());
        self
    }

    /// Starts app-server without the standard automatic test environment.
    pub fn without_auto_env(mut self) -> Self {
        self.environment = TestAppServerEnvironment::None;
        self
    }

    /// Uses this app-server binary instead of the standard test binary.
    pub fn with_program(mut self, program: &Path) -> Self {
        self.program = Some(program.to_path_buf());
        self
    }

    /// Adds command-line arguments after the default test arguments.
    pub fn with_args(mut self, args: &[&str]) -> Self {
        self.args
            .extend(args.iter().map(|argument| (*argument).to_string()));
        self
    }

    /// Enables startup tasks that the default test arguments disable.
    pub fn with_plugin_startup_tasks(mut self) -> Self {
        self.args
            .retain(|argument| argument != DISABLE_PLUGIN_STARTUP_TASKS_ARG);
        self
    }

    /// Adds child-process environment overrides.
    ///
    /// Some values set variables and None values remove inherited variables.
    pub fn with_env_overrides(mut self, env_overrides: &[(&str, Option<&str>)]) -> Self {
        self.env_overrides
            .extend(env_overrides.iter().map(|(key, value)| {
                (
                    (*key).to_string(),
                    value.map(std::string::ToString::to_string),
                )
            }));
        self
    }

    /// Prevents the child from loading managed configuration.
    pub fn without_managed_config(self) -> Self {
        self.with_env_overrides(&[(DISABLE_MANAGED_CONFIG_ENV_VAR, Some("1"))])
    }

    /// Configures the child to emit JSON logs at the requested Rust log level.
    pub fn with_json_logging(self, rust_log: impl Into<String>) -> Self {
        let rust_log = rust_log.into();
        let mut builder = self.with_env_overrides(&[("LOG_FORMAT", Some("json"))]);
        builder
            .env_overrides
            .push(("RUST_LOG".to_string(), Some(rust_log)));
        builder
    }

    /// Adds this fixed one-way delay to the app-server/exec-server RPC stream.
    /// A 15ms delay contributes roughly 30ms to a round trip.
    pub fn with_exec_server_delay(mut self, exec_server_delay: Duration) -> Self {
        self.exec_server_delay = Some(exec_server_delay);
        self
    }

    /// Builds a server and completes its standard initialization handshake.
    pub async fn build_initialized(self) -> anyhow::Result<TestAppServer> {
        self.build_initialized_with_timeout(DEFAULT_REQUEST_TIMEOUT)
            .await
    }

    /// Builds and initializes a server while preserving a suite-specific timeout.
    pub async fn build_initialized_with_timeout(
        self,
        timeout: Duration,
    ) -> anyhow::Result<TestAppServer> {
        let mut server = self.build().await?;
        tokio::time::timeout(timeout, server.initialize()).await??;
        Ok(server)
    }

    /// Builds a server with a temporary CODEX_HOME and automatic environment
    /// by default.
    pub async fn build(self) -> anyhow::Result<TestAppServer> {
        let Self {
            codex_home,
            environment,
            program,
            mut env_overrides,
            args,
            exec_server_delay,
        } = self;
        let (codex_home, owned_codex_home) = match codex_home {
            Some(codex_home) => (codex_home, None),
            None => {
                let owned_codex_home = TempDir::new()?;
                (
                    owned_codex_home.path().to_path_buf(),
                    Some(owned_codex_home),
                )
            }
        };
        let (auto_env, delayed_exec_server) = match environment {
            TestAppServerEnvironment::Auto => {
                let environments_toml = codex_home.join("environments.toml");
                ensure!(
                    !environments_toml.try_exists().with_context(|| format!(
                        "check whether {} exists",
                        environments_toml.display()
                    ))?,
                    "automatic environment cannot be used when {} exists",
                    environments_toml.display()
                );
                let (auto_env, delayed_exec_server) = match exec_server_delay {
                    Some(added_delay) => {
                        ensure!(
                            !is_remote_test_environment(),
                            "TestAppServer exec-server delay only supports the local test environment"
                        );
                        let exec_server_program =
                            codex_utils_cargo_bin::cargo_bin("exec-server")
                                .context("should find binary for delayed exec-server fixture")?;
                        // Local auto environments normally use stdio. Start a
                        // host-local WebSocket fixture so the delay interposer has a
                        // socket stream to wrap.
                        let local_websocket_exec_server =
                            LocalWebsocketExecServer::start(&codex_home, &exec_server_program)
                                .await?;
                        let interposer = WebsocketDelayInterposer::start(
                            local_websocket_exec_server.websocket_url(),
                            added_delay,
                        )
                        .await?;
                        let auto_env = TestEnv::local_with_exec_server_url(Some(
                            interposer.websocket_url().to_string(),
                        ))
                        .await?;
                        (auto_env, Some((local_websocket_exec_server, interposer)))
                    }
                    None => (test_env().await?, None),
                };
                // Noise registry configuration takes precedence over the URL-based
                // provider, so clear inherited values to keep the selection hermetic.
                let mut auto_env_overrides = vec![
                    (
                        CODEX_EXEC_SERVER_URL_ENV_VAR.to_string(),
                        auto_env.exec_server_url().map(str::to_string),
                    ),
                    (
                        CODEX_EXEC_SERVER_NOISE_REGISTRY_URL_ENV_VAR.to_string(),
                        None,
                    ),
                    (
                        CODEX_EXEC_SERVER_NOISE_ENVIRONMENT_ID_ENV_VAR.to_string(),
                        None,
                    ),
                    (CODEX_EXEC_SERVER_NOISE_AUTH_TOKEN_ENV_VAR.to_string(), None),
                    (
                        CODEX_EXEC_SERVER_NOISE_CHATGPT_ACCOUNT_ID_ENV_VAR.to_string(),
                        None,
                    ),
                ];
                auto_env_overrides.append(&mut env_overrides);
                env_overrides = auto_env_overrides;
                (Some(auto_env), delayed_exec_server)
            }
            TestAppServerEnvironment::None => {
                ensure!(
                    exec_server_delay.is_none(),
                    "exec-server delay requires the automatic test environment"
                );
                (None, None)
            }
        };
        if !env_overrides
            .iter()
            .any(|(key, _)| key == CODE_MODE_HOST_PATH_ENV_VAR)
            && let Ok(code_mode_host_program) =
                codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")
        {
            env_overrides.insert(
                0,
                (
                    CODE_MODE_HOST_PATH_ENV_VAR.to_string(),
                    Some(code_mode_host_program.to_string_lossy().into_owned()),
                ),
            );
        }
        let program = match program {
            Some(program) => program,
            None => codex_utils_cargo_bin::cargo_bin("codex-app-server")
                .context("should find binary for codex-app-server")?,
        };
        let env_overrides = env_overrides
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_deref()))
            .collect::<Vec<_>>();
        let args = args.iter().map(String::as_str).collect::<Vec<_>>();
        let mut app_server = TestAppServer::new_with_program_env_and_args(
            &codex_home,
            &program,
            &env_overrides,
            &args,
        )
        .await?;
        app_server.auto_env = auto_env;
        app_server._owned_codex_home = owned_codex_home;
        app_server._delayed_exec_server = delayed_exec_server;
        Ok(app_server)
    }
}

impl Drop for TestAppServer {
    fn drop(&mut self) {
        // These tests spawn a `codex-app-server` child process.
        //
        // We keep that child alive for the test and rely on Tokio's `kill_on_drop(true)` when this
        // helper is dropped. Tokio documents kill-on-drop as best-effort: dropping requests
        // termination, but it does not guarantee the child has fully exited and been reaped before
        // teardown continues.
        //
        // That makes cleanup timing nondeterministic. Leak detection can occasionally observe the
        // child still alive at teardown and report `LEAK`, which makes the test flaky.
        //
        // Drop can't be async, so we do a bounded synchronous cleanup:
        //
        // 1. Close stdin to request a graceful shutdown via EOF.
        // 2. Poll briefly for graceful exit.
        // 3. If still alive, request termination with `start_kill()`.
        // 4. Poll `try_wait()` until the OS reports the child exited, with a short timeout.
        drop(self.stdin.take());

        let graceful_start = std::time::Instant::now();
        let graceful_timeout = std::time::Duration::from_millis(200);
        while graceful_start.elapsed() < graceful_timeout {
            match self.process.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(5)),
                Err(_) => return,
            }
        }

        let _ = self.process.start_kill();

        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(5);
        while start.elapsed() < timeout {
            match self.process.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
                Err(_) => return,
            }
        }
    }
}
