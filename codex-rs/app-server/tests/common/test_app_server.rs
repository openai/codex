use std::collections::VecDeque;
use std::net::SocketAddr;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::timeout;
use tokio_stream::wrappers::ReceiverStream;

use anyhow::Context;
use codex_app_server_protocol::AppsListParams;
use codex_app_server_protocol::CancelLoginAccountParams;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientNotification;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponse;
use codex_app_server_protocol::CollaborationModeListParams;
use codex_app_server_protocol::CommandExecParams;
use codex_app_server_protocol::CommandExecResizeParams;
use codex_app_server_protocol::CommandExecTerminateParams;
use codex_app_server_protocol::CommandExecWriteParams;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::ExperimentalFeatureListParams;
use codex_app_server_protocol::FeedbackUploadParams;
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
use codex_app_server_protocol::ProcessResizePtyParams;
use codex_app_server_protocol::ProcessSpawnParams;
use codex_app_server_protocol::ProcessWriteStdinParams;
use codex_app_server_protocol::RemoteControlClientsListParams;
use codex_app_server_protocol::RemoteControlClientsRevokeParams;
use codex_app_server_protocol::RemoteControlPairingStartParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ReviewStartParams;
use codex_app_server_protocol::RpcError;
use codex_app_server_protocol::SendAddCreditsNudgeEmailParams;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ServerResponse;
use codex_app_server_protocol::SkillsExtraRootsSetParams;
use codex_app_server_protocol::SkillsListParams;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadCompactStartParams;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadInjectItemsParams;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadMemoryModeSetParams;
use codex_app_server_protocol::ThreadMetadataUpdateParams;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadRealtimeAppendAudioParams;
use codex_app_server_protocol::ThreadRealtimeAppendTextParams;
use codex_app_server_protocol::ThreadRealtimeListVoicesParams;
use codex_app_server_protocol::ThreadRealtimeStartParams;
use codex_app_server_protocol::ThreadRealtimeStopParams;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadRollbackParams;
use codex_app_server_protocol::ThreadSearchParams;
use codex_app_server_protocol::ThreadSetNameParams;
use codex_app_server_protocol::ThreadSettingsUpdateParams;
use codex_app_server_protocol::ThreadShellCommandParams;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadTurnsItemsListParams;
use codex_app_server_protocol::ThreadTurnsListParams;
use codex_app_server_protocol::ThreadUnarchiveParams;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnSteerParams;
use codex_app_server_protocol::WindowsSandboxSetupStartParams;
use codex_app_server_transport::NativeServerMessage;
use codex_app_server_transport::decode_grpc_server_message;
use codex_app_server_transport::encode_grpc_client_error;
use codex_app_server_transport::encode_grpc_client_notification;
use codex_app_server_transport::encode_grpc_client_request;
use codex_app_server_transport::encode_grpc_server_response;
use codex_app_server_transport::grpc_proto::ClientMessage;
use codex_app_server_transport::grpc_proto::ServerMessage;
use codex_app_server_transport::grpc_proto::codex_app_server_client::CodexAppServerClient;
use codex_login::default_client::CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR;
use tokio::process::Command;

pub trait IntoServerResponse {
    fn into_server_response(self, request_id: RequestId) -> ServerResponse;
}

macro_rules! server_response_conversions {
    ($($response:ty => $variant:ident),* $(,)?) => {
        $(
            impl IntoServerResponse for $response {
                fn into_server_response(self, request_id: RequestId) -> ServerResponse {
                    ServerResponse::$variant {
                        request_id,
                        response: self,
                    }
                }
            }
        )*
    };
}

server_response_conversions! {
    codex_app_server_protocol::CommandExecutionRequestApprovalResponse
        => CommandExecutionRequestApproval,
    codex_app_server_protocol::FileChangeRequestApprovalResponse => FileChangeRequestApproval,
    codex_app_server_protocol::ToolRequestUserInputResponse => ToolRequestUserInput,
    codex_app_server_protocol::McpServerElicitationRequestResponse => McpServerElicitationRequest,
    codex_app_server_protocol::PermissionsRequestApprovalResponse => PermissionsRequestApproval,
    codex_app_server_protocol::DynamicToolCallResponse => DynamicToolCall,
    codex_app_server_protocol::ChatgptAuthTokensRefreshResponse => ChatgptAuthTokensRefresh,
    codex_app_server_protocol::AttestationGenerateResponse => AttestationGenerate,
    codex_app_server_protocol::ApplyPatchApprovalResponse => ApplyPatchApproval,
    codex_app_server_protocol::ExecCommandApprovalResponse => ExecCommandApproval,
}

macro_rules! typed_request_helpers {
    ($($name:ident($params:ident: $params_type:ty) => $variant:ident),* $(,)?) => {
        $(
            pub async fn $name(
                &mut self,
                $params: $params_type,
            ) -> anyhow::Result<i64> {
                self.send_request(|request_id| ClientRequest::$variant {
                    request_id,
                    params: $params,
                })
                .await
            }
        )*
    };
}

pub struct TestAppServer {
    next_request_id: AtomicI64,
    /// Retain this child process until the client is dropped. The Tokio runtime
    /// will make a "best effort" to reap the process after it exits, but it is
    /// not a guarantee. See the `kill_on_drop` documentation for details.
    #[allow(dead_code)]
    process: Child,
    client_tx: Option<mpsc::Sender<ClientMessage>>,
    server_stream: tonic::Streaming<ServerMessage>,
    pending_messages: VecDeque<NativeServerMessage>,
}

pub const DEFAULT_CLIENT_NAME: &str = "codex-app-server-tests";
pub const DISABLE_PLUGIN_STARTUP_TASKS_ARG: &str = "--disable-plugin-startup-tasks-for-tests";
const DISABLE_MANAGED_CONFIG_ENV_VAR: &str = "CODEX_APP_SERVER_DISABLE_MANAGED_CONFIG";
const GRPC_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

impl TestAppServer {
    pub async fn new(codex_home: &Path) -> anyhow::Result<Self> {
        Self::new_with_env_and_args(codex_home, &[], &[DISABLE_PLUGIN_STARTUP_TASKS_ARG]).await
    }

    pub async fn new_without_managed_config(codex_home: &Path) -> anyhow::Result<Self> {
        Self::new_with_env(codex_home, &[(DISABLE_MANAGED_CONFIG_ENV_VAR, Some("1"))]).await
    }

    pub async fn new_without_managed_config_with_env(
        codex_home: &Path,
        env_overrides: &[(&str, Option<&str>)],
    ) -> anyhow::Result<Self> {
        let mut all_env_overrides = vec![(DISABLE_MANAGED_CONFIG_ENV_VAR, Some("1"))];
        all_env_overrides.extend_from_slice(env_overrides);
        Self::new_with_env(codex_home, &all_env_overrides).await
    }

    pub async fn new_with_plugin_startup_tasks(codex_home: &Path) -> anyhow::Result<Self> {
        Self::new_with_env_and_args(codex_home, &[], &[]).await
    }

    pub async fn new_with_env_and_plugin_startup_tasks(
        codex_home: &Path,
        env_overrides: &[(&str, Option<&str>)],
    ) -> anyhow::Result<Self> {
        Self::new_with_env_and_args(codex_home, env_overrides, &[]).await
    }

    pub async fn new_with_args(codex_home: &Path, args: &[&str]) -> anyhow::Result<Self> {
        let mut all_args = vec![DISABLE_PLUGIN_STARTUP_TASKS_ARG];
        all_args.extend_from_slice(args);
        Self::new_with_env_and_args(codex_home, &[], &all_args).await
    }

    /// Creates a new MCP process, allowing tests to override or remove
    /// specific environment variables for the child process only.
    ///
    /// Pass a tuple of (key, Some(value)) to set/override, or (key, None) to
    /// remove a variable from the child's environment.
    pub async fn new_with_env(
        codex_home: &Path,
        env_overrides: &[(&str, Option<&str>)],
    ) -> anyhow::Result<Self> {
        Self::new_with_env_and_args(
            codex_home,
            env_overrides,
            &[DISABLE_PLUGIN_STARTUP_TASKS_ARG],
        )
        .await
    }

    pub async fn new_with_program_and_env(
        codex_home: &Path,
        program: &Path,
        env_overrides: &[(&str, Option<&str>)],
    ) -> anyhow::Result<Self> {
        Self::new_with_program_env_and_args(
            codex_home,
            program,
            env_overrides,
            &[DISABLE_PLUGIN_STARTUP_TASKS_ARG],
        )
        .await
    }

    async fn new_with_env_and_args(
        codex_home: &Path,
        env_overrides: &[(&str, Option<&str>)],
        args: &[&str],
    ) -> anyhow::Result<Self> {
        let program = codex_utils_cargo_bin::cargo_bin("codex-app-server")
            .context("should find binary for codex-app-server")?;
        Self::new_with_program_env_and_args(codex_home, &program, env_overrides, args).await
    }

    async fn new_with_program_env_and_args(
        codex_home: &Path,
        program: &Path,
        env_overrides: &[(&str, Option<&str>)],
        args: &[&str],
    ) -> anyhow::Result<Self> {
        let mut cmd = Command::new(program);

        cmd.arg("--listen").arg("grpc://127.0.0.1:0");
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::null());
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
        let stderr = process
            .stderr
            .take()
            .context("app-server should have stderr fd")?;
        let mut stderr_reader = BufReader::new(stderr).lines();
        let deadline = Instant::now() + GRPC_CONNECT_TIMEOUT;
        let bind_addr = loop {
            let line = timeout(
                deadline.saturating_duration_since(Instant::now()),
                stderr_reader.next_line(),
            )
            .await
            .context("timed out waiting for app-server gRPC address")?
            .context("failed to read app-server stderr")?
            .context("app-server exited before reporting its gRPC address")?;
            eprintln!("[mcp stderr] {line}");
            if let Some(bind_addr) = line
                .split_whitespace()
                .find_map(|token| token.strip_prefix("grpc://"))
                .and_then(|addr| addr.parse::<SocketAddr>().ok())
            {
                break bind_addr;
            }
        };
        tokio::spawn(async move {
            while let Ok(Some(line)) = stderr_reader.next_line().await {
                eprintln!("[mcp stderr] {line}");
            }
        });

        let mut client = CodexAppServerClient::connect(format!("http://{bind_addr}"))
            .await
            .context("connect to app-server gRPC endpoint")?
            .max_decoding_message_size(16 * 1024 * 1024)
            .max_encoding_message_size(16 * 1024 * 1024);
        let (client_tx, client_rx) = mpsc::channel(128);
        let server_stream = client
            .session(ReceiverStream::new(client_rx))
            .await
            .context("open app-server gRPC session")?
            .into_inner();
        Ok(Self {
            next_request_id: AtomicI64::new(0),
            process,
            client_tx: Some(client_tx),
            server_stream,
            pending_messages: VecDeque::new(),
        })
    }

    /// Performs the initialization handshake with the app server.
    pub async fn initialize(&mut self) -> anyhow::Result<()> {
        match self
            .initialize_with_client_info(ClientInfo {
                name: DEFAULT_CLIENT_NAME.to_string(),
                title: None,
                version: "0.1.0".to_string(),
            })
            .await?
        {
            Ok(_) => Ok(()),
            Err(error) => anyhow::bail!("initialize failed: {error:?}"),
        }
    }

    /// Sends initialize with the provided client info and returns the typed response or error.
    pub async fn initialize_with_client_info(
        &mut self,
        client_info: ClientInfo,
    ) -> anyhow::Result<Result<ClientResponse, RpcError>> {
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
    ) -> anyhow::Result<Result<ClientResponse, RpcError>> {
        self.initialize_with_params(InitializeParams {
            client_info,
            capabilities,
        })
        .await
    }

    async fn initialize_with_params(
        &mut self,
        params: InitializeParams,
    ) -> anyhow::Result<Result<ClientResponse, RpcError>> {
        let request_id = self
            .send_request(|request_id| ClientRequest::Initialize { request_id, params })
            .await?;
        let request_id = RequestId::Integer(request_id);
        let message = self
            .read_stream_until_message(|message| {
                Self::message_request_id(message) == Some(&request_id)
            })
            .await?;
        match message {
            NativeServerMessage::Response(response) => {
                self.send_notification(ClientNotification::Initialized)
                    .await?;
                Ok(Ok(response))
            }
            NativeServerMessage::Error { error, .. } => Ok(Err(error)),
            NativeServerMessage::Notification(notification) => {
                anyhow::bail!("unexpected server notification during initialize: {notification:?}")
            }
            NativeServerMessage::Request(request) => {
                anyhow::bail!("unexpected server request during initialize: {request:?}")
            }
        }
    }

    typed_request_helpers! {
        send_get_auth_status_request(params: GetAuthStatusParams) => GetAuthStatus,
        send_get_conversation_summary_request(params: GetConversationSummaryParams)
            => GetConversationSummary,
        send_add_credits_nudge_email_request(params: SendAddCreditsNudgeEmailParams)
            => SendAddCreditsNudgeEmail,
        send_get_account_request(params: GetAccountParams) => GetAccount,
        send_feedback_upload_request(params: FeedbackUploadParams) => FeedbackUpload,
        send_thread_start_request(params: ThreadStartParams) => ThreadStart,
        send_thread_resume_request(params: ThreadResumeParams) => ThreadResume,
        send_thread_fork_request(params: ThreadForkParams) => ThreadFork,
        send_thread_archive_request(params: ThreadArchiveParams) => ThreadArchive,
        send_thread_set_name_request(params: ThreadSetNameParams) => ThreadSetName,
        send_thread_metadata_update_request(params: ThreadMetadataUpdateParams)
            => ThreadMetadataUpdate,
        send_thread_settings_update_request(params: ThreadSettingsUpdateParams)
            => ThreadSettingsUpdate,
        send_thread_unsubscribe_request(params: ThreadUnsubscribeParams) => ThreadUnsubscribe,
        send_thread_unarchive_request(params: ThreadUnarchiveParams) => ThreadUnarchive,
        send_thread_compact_start_request(params: ThreadCompactStartParams) => ThreadCompactStart,
        send_thread_shell_command_request(params: ThreadShellCommandParams) => ThreadShellCommand,
        send_thread_rollback_request(params: ThreadRollbackParams) => ThreadRollback,
        send_thread_list_request(params: ThreadListParams) => ThreadList,
        send_thread_search_request(params: ThreadSearchParams) => ThreadSearch,
        send_thread_loaded_list_request(params: ThreadLoadedListParams) => ThreadLoadedList,
        send_thread_read_request(params: ThreadReadParams) => ThreadRead,
        send_thread_turns_list_request(params: ThreadTurnsListParams) => ThreadTurnsList,
        send_thread_turns_items_list_request(params: ThreadTurnsItemsListParams)
            => ThreadTurnsItemsList,
        send_list_models_request(params: ModelListParams) => ModelList,
        send_model_provider_capabilities_read_request(
            params: ModelProviderCapabilitiesReadParams
        ) => ModelProviderCapabilitiesRead,
        send_experimental_feature_list_request(params: ExperimentalFeatureListParams)
            => ExperimentalFeatureList,
        send_permission_profile_list_request(params: PermissionProfileListParams)
            => PermissionProfileList,
        send_experimental_feature_enablement_set_request(
            params: codex_app_server_protocol::ExperimentalFeatureEnablementSetParams
        ) => ExperimentalFeatureEnablementSet,
        send_remote_control_pairing_start_request(params: RemoteControlPairingStartParams)
            => RemoteControlPairingStart,
        send_remote_control_clients_list_request(params: RemoteControlClientsListParams)
            => RemoteControlClientsList,
        send_remote_control_clients_revoke_request(params: RemoteControlClientsRevokeParams)
            => RemoteControlClientsRevoke,
        send_apps_list_request(params: AppsListParams) => AppsList,
        send_mcp_resource_read_request(params: McpResourceReadParams) => McpResourceRead,
        send_mcp_server_tool_call_request(params: McpServerToolCallParams) => McpServerToolCall,
        send_skills_list_request(params: SkillsListParams) => SkillsList,
        send_skills_extra_roots_set_request(params: SkillsExtraRootsSetParams)
            => SkillsExtraRootsSet,
        send_hooks_list_request(params: HooksListParams) => HooksList,
        send_marketplace_add_request(params: MarketplaceAddParams) => MarketplaceAdd,
        send_marketplace_remove_request(params: MarketplaceRemoveParams) => MarketplaceRemove,
        send_marketplace_upgrade_request(params: MarketplaceUpgradeParams) => MarketplaceUpgrade,
        send_plugin_install_request(params: PluginInstallParams) => PluginInstall,
        send_plugin_uninstall_request(params: PluginUninstallParams) => PluginUninstall,
        send_plugin_list_request(params: PluginListParams) => PluginList,
        send_plugin_installed_request(params: PluginInstalledParams) => PluginInstalled,
        send_plugin_read_request(params: PluginReadParams) => PluginRead,
        send_plugin_skill_read_request(params: PluginSkillReadParams) => PluginSkillRead,
        send_list_mcp_server_status_request(params: ListMcpServerStatusParams)
            => McpServerStatusList,
        send_list_collaboration_modes_request(params: CollaborationModeListParams)
            => CollaborationModeList,
        send_mock_experimental_method_request(params: MockExperimentalMethodParams)
            => MockExperimentalMethod,
        send_thread_memory_mode_set_request(params: ThreadMemoryModeSetParams)
            => ThreadMemoryModeSet,
        send_turn_start_request(params: TurnStartParams) => TurnStart,
        send_thread_inject_items_request(params: ThreadInjectItemsParams) => ThreadInjectItems,
        send_command_exec_request(params: CommandExecParams) => OneOffCommandExec,
        send_process_spawn_request(params: ProcessSpawnParams) => ProcessSpawn,
        send_process_write_stdin_request(params: ProcessWriteStdinParams) => ProcessWriteStdin,
        send_process_resize_pty_request(params: ProcessResizePtyParams) => ProcessResizePty,
        send_process_kill_request(params: ProcessKillParams) => ProcessKill,
        send_command_exec_write_request(params: CommandExecWriteParams) => CommandExecWrite,
        send_command_exec_resize_request(params: CommandExecResizeParams) => CommandExecResize,
        send_command_exec_terminate_request(params: CommandExecTerminateParams)
            => CommandExecTerminate,
        send_turn_interrupt_request(params: TurnInterruptParams) => TurnInterrupt,
        send_thread_realtime_start_request(params: ThreadRealtimeStartParams)
            => ThreadRealtimeStart,
        send_thread_realtime_append_audio_request(params: ThreadRealtimeAppendAudioParams)
            => ThreadRealtimeAppendAudio,
        send_thread_realtime_append_text_request(params: ThreadRealtimeAppendTextParams)
            => ThreadRealtimeAppendText,
        send_thread_realtime_stop_request(params: ThreadRealtimeStopParams)
            => ThreadRealtimeStop,
        send_thread_realtime_list_voices_request(params: ThreadRealtimeListVoicesParams)
            => ThreadRealtimeListVoices,
        send_turn_steer_request(params: TurnSteerParams) => TurnSteer,
        send_review_start_request(params: ReviewStartParams) => ReviewStart,
        send_windows_sandbox_setup_start_request(params: WindowsSandboxSetupStartParams)
            => WindowsSandboxSetupStart,
        send_config_read_request(params: ConfigReadParams) => ConfigRead,
        send_config_value_write_request(params: ConfigValueWriteParams) => ConfigValueWrite,
        send_config_batch_write_request(params: ConfigBatchWriteParams) => ConfigBatchWrite,
        send_fs_read_file_request(params: FsReadFileParams) => FsReadFile,
        send_fs_write_file_request(params: FsWriteFileParams) => FsWriteFile,
        send_fs_create_directory_request(params: FsCreateDirectoryParams) => FsCreateDirectory,
        send_fs_get_metadata_request(params: FsGetMetadataParams) => FsGetMetadata,
        send_fs_read_directory_request(params: FsReadDirectoryParams) => FsReadDirectory,
        send_fs_remove_request(params: FsRemoveParams) => FsRemove,
        send_fs_copy_request(params: FsCopyParams) => FsCopy,
        send_fs_watch_request(params: FsWatchParams) => FsWatch,
        send_fs_unwatch_request(params: FsUnwatchParams) => FsUnwatch,
        send_cancel_login_account_request(params: CancelLoginAccountParams) => CancelLoginAccount,
    }

    pub async fn send_get_account_rate_limits_request(&mut self) -> anyhow::Result<i64> {
        self.send_request(|request_id| ClientRequest::GetAccountRateLimits {
            request_id,
            params: None,
        })
        .await
    }

    pub async fn send_remote_control_enable_request(&mut self) -> anyhow::Result<i64> {
        self.send_request(|request_id| ClientRequest::RemoteControlEnable {
            request_id,
            params: None,
        })
        .await
    }

    pub async fn send_remote_control_disable_request(&mut self) -> anyhow::Result<i64> {
        self.send_request(|request_id| ClientRequest::RemoteControlDisable {
            request_id,
            params: None,
        })
        .await
    }

    pub async fn send_remote_control_status_read_request(&mut self) -> anyhow::Result<i64> {
        self.send_request(|request_id| ClientRequest::RemoteControlStatusRead {
            request_id,
            params: None,
        })
        .await
    }

    pub async fn send_chatgpt_auth_tokens_login_request(
        &mut self,
        access_token: String,
        chatgpt_account_id: String,
        chatgpt_plan_type: Option<String>,
    ) -> anyhow::Result<i64> {
        self.send_login_account_request(LoginAccountParams::ChatgptAuthTokens {
            access_token,
            chatgpt_account_id,
            chatgpt_plan_type,
        })
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

    pub async fn send_logout_account_request(&mut self) -> anyhow::Result<i64> {
        self.send_request(|request_id| ClientRequest::LogoutAccount {
            request_id,
            params: None,
        })
        .await
    }

    pub async fn send_login_account_api_key_request(
        &mut self,
        api_key: &str,
    ) -> anyhow::Result<i64> {
        self.send_login_account_request(LoginAccountParams::ApiKey {
            api_key: api_key.to_string(),
        })
        .await
    }

    pub async fn send_login_account_chatgpt_request(&mut self) -> anyhow::Result<i64> {
        self.send_login_account_request(LoginAccountParams::Chatgpt {
            codex_streamlined_login: false,
        })
        .await
    }

    pub async fn send_login_account_chatgpt_device_code_request(&mut self) -> anyhow::Result<i64> {
        self.send_login_account_request(LoginAccountParams::ChatgptDeviceCode)
            .await
    }

    async fn send_login_account_request(
        &mut self,
        params: LoginAccountParams,
    ) -> anyhow::Result<i64> {
        self.send_request(|request_id| ClientRequest::LoginAccount { request_id, params })
            .await
    }

    pub async fn send_fuzzy_file_search_request(
        &mut self,
        query: &str,
        roots: Vec<String>,
        cancellation_token: Option<String>,
    ) -> anyhow::Result<i64> {
        let params = codex_app_server_protocol::FuzzyFileSearchParams {
            query: query.to_string(),
            roots,
            cancellation_token,
        };
        self.send_request(|request_id| ClientRequest::FuzzyFileSearch { request_id, params })
            .await
    }

    pub async fn send_fuzzy_file_search_session_start_request(
        &mut self,
        session_id: &str,
        roots: Vec<String>,
    ) -> anyhow::Result<i64> {
        let params = codex_app_server_protocol::FuzzyFileSearchSessionStartParams {
            session_id: session_id.to_string(),
            roots,
        };
        self.send_request(|request_id| ClientRequest::FuzzyFileSearchSessionStart {
            request_id,
            params,
        })
        .await
    }

    pub async fn start_fuzzy_file_search_session(
        &mut self,
        session_id: &str,
        roots: Vec<String>,
    ) -> anyhow::Result<ClientResponse> {
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
        let params = codex_app_server_protocol::FuzzyFileSearchSessionUpdateParams {
            session_id: session_id.to_string(),
            query: query.to_string(),
        };
        self.send_request(|request_id| ClientRequest::FuzzyFileSearchSessionUpdate {
            request_id,
            params,
        })
        .await
    }

    pub async fn update_fuzzy_file_search_session(
        &mut self,
        session_id: &str,
        query: &str,
    ) -> anyhow::Result<ClientResponse> {
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
        let params = codex_app_server_protocol::FuzzyFileSearchSessionStopParams {
            session_id: session_id.to_string(),
        };
        self.send_request(|request_id| ClientRequest::FuzzyFileSearchSessionStop {
            request_id,
            params,
        })
        .await
    }

    pub async fn stop_fuzzy_file_search_session(
        &mut self,
        session_id: &str,
    ) -> anyhow::Result<ClientResponse> {
        let request_id = self
            .send_fuzzy_file_search_session_stop_request(session_id)
            .await?;
        self.read_stream_until_response_message(RequestId::Integer(request_id))
            .await
    }

    async fn send_request<F>(&mut self, build_request: F) -> anyhow::Result<i64>
    where
        F: FnOnce(RequestId) -> ClientRequest,
    {
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let request = build_request(RequestId::Integer(request_id));
        eprintln!("writing native gRPC request: {request:?}");
        self.send_client_message(encode_grpc_client_request(request, None)?)
            .await?;
        Ok(request_id)
    }

    pub async fn send_response<R>(&mut self, id: RequestId, response: R) -> anyhow::Result<()>
    where
        R: IntoServerResponse,
    {
        let response = response.into_server_response(id);
        eprintln!("writing native gRPC response: {response:?}");
        self.send_client_message(encode_grpc_server_response(response)?)
            .await
    }

    pub async fn send_error(&mut self, id: RequestId, error: RpcError) -> anyhow::Result<()> {
        eprintln!("writing native gRPC error: {error:?}");
        self.send_client_message(encode_grpc_client_error(id, error)?)
            .await
    }

    pub async fn send_notification(
        &mut self,
        notification: ClientNotification,
    ) -> anyhow::Result<()> {
        eprintln!("writing native gRPC notification: {notification:?}");
        self.send_client_message(encode_grpc_client_notification(notification)?)
            .await
    }

    async fn send_client_message(&mut self, client_message: ClientMessage) -> anyhow::Result<()> {
        let Some(client_tx) = self.client_tx.as_ref() else {
            anyhow::bail!("app-server gRPC session closed");
        };
        client_tx
            .send(client_message)
            .await
            .context("send app-server gRPC message")?;
        Ok(())
    }

    async fn read_native_message(&mut self) -> anyhow::Result<NativeServerMessage> {
        let message = self
            .server_stream
            .message()
            .await
            .context("read app-server gRPC message")?
            .context("app-server gRPC session ended")?;
        let message = decode_grpc_server_message(message)?;
        eprintln!("read native gRPC message: {message:?}");
        Ok(message)
    }

    pub async fn read_stream_until_request_message(&mut self) -> anyhow::Result<ServerRequest> {
        eprintln!("in read_stream_until_request_message()");

        let message = self
            .read_stream_until_message(|message| matches!(message, NativeServerMessage::Request(_)))
            .await?;

        let NativeServerMessage::Request(request) = message else {
            unreachable!("expected native server request, got {message:?}");
        };
        Ok(request)
    }

    pub async fn read_stream_until_response_message(
        &mut self,
        request_id: RequestId,
    ) -> anyhow::Result<ClientResponse> {
        eprintln!("in read_stream_until_response_message({request_id:?})");

        let message = self
            .read_stream_until_message(|message| {
                matches!(
                    message,
                    NativeServerMessage::Response(response) if response.id() == &request_id
                )
            })
            .await?;

        let NativeServerMessage::Response(response) = message else {
            unreachable!("expected native client response, got {message:?}");
        };
        Ok(response)
    }

    pub async fn read_stream_until_error_message(
        &mut self,
        request_id: RequestId,
    ) -> anyhow::Result<RpcError> {
        let message = self
            .read_stream_until_message(|message| {
                matches!(
                    message,
                    NativeServerMessage::Error {
                        request_id: message_request_id,
                        ..
                    } if message_request_id == &request_id
                )
            })
            .await?;

        let NativeServerMessage::Error { error, .. } = message else {
            unreachable!("expected native RPC error, got {message:?}");
        };
        Ok(error)
    }

    pub async fn read_stream_until_notification_message(
        &mut self,
        method: &str,
    ) -> anyhow::Result<ServerNotification> {
        eprintln!("in read_stream_until_notification_message({method})");

        let message = self
            .read_stream_until_message(|message| {
                matches!(
                    message,
                    NativeServerMessage::Notification(notification)
                        if notification.to_string() == method
                )
            })
            .await?;

        let NativeServerMessage::Notification(notification) = message else {
            unreachable!("expected native server notification, got {message:?}");
        };
        Ok(notification)
    }

    pub async fn read_stream_until_matching_notification<F>(
        &mut self,
        description: &str,
        predicate: F,
    ) -> anyhow::Result<ServerNotification>
    where
        F: Fn(&ServerNotification) -> bool,
    {
        eprintln!("in read_stream_until_matching_notification({description})");

        let message = self
            .read_stream_until_message(|message| {
                matches!(
                    message,
                    NativeServerMessage::Notification(notification) if predicate(notification)
                )
            })
            .await?;

        let NativeServerMessage::Notification(notification) = message else {
            unreachable!("expected native server notification, got {message:?}");
        };
        Ok(notification)
    }

    pub async fn read_next_message(&mut self) -> anyhow::Result<NativeServerMessage> {
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
                NativeServerMessage::Notification(notification) => Some(notification.to_string()),
                _ => None,
            })
            .collect()
    }

    /// Reads the stream until a message matches `predicate`, buffering any non-matching messages
    /// for later reads.
    async fn read_stream_until_message<F>(
        &mut self,
        predicate: F,
    ) -> anyhow::Result<NativeServerMessage>
    where
        F: Fn(&NativeServerMessage) -> bool,
    {
        if let Some(message) = self.take_pending_message(&predicate) {
            return Ok(message);
        }

        loop {
            let message = self.read_native_message().await?;
            if predicate(&message) {
                return Ok(message);
            }
            self.pending_messages.push_back(message);
        }
    }

    fn take_pending_message<F>(&mut self, predicate: &F) -> Option<NativeServerMessage>
    where
        F: Fn(&NativeServerMessage) -> bool,
    {
        if let Some(pos) = self.pending_messages.iter().position(predicate) {
            return self.pending_messages.remove(pos);
        }
        None
    }

    fn pending_turn_completed_notification(&self, thread_id: &str, turn_id: &str) -> bool {
        self.pending_messages.iter().any(|message| {
            matches!(
                message,
                NativeServerMessage::Notification(ServerNotification::TurnCompleted(payload))
                    if payload.thread_id == thread_id && payload.turn.id == turn_id
            )
        })
    }

    fn message_request_id(message: &NativeServerMessage) -> Option<&RequestId> {
        match message {
            NativeServerMessage::Request(request) => Some(request.id()),
            NativeServerMessage::Response(response) => Some(response.id()),
            NativeServerMessage::Error { request_id, .. } => Some(request_id),
            NativeServerMessage::Notification(_) => None,
        }
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
        // 1. Close the request side of the gRPC stream.
        // 2. Poll briefly for graceful exit.
        // 3. If still alive, request termination with `start_kill()`.
        // 4. Poll `try_wait()` until the OS reports the child exited, with a short timeout.
        drop(self.client_tx.take());

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
