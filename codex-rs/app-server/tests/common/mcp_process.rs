use std::collections::VecDeque;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;

use anyhow::Context;
use codex_app_server_protocol::AddConversationListenerParams;
use codex_app_server_protocol::AppsListParams;
use codex_app_server_protocol::ArchiveConversationParams;
use codex_app_server_protocol::CancelLoginAccountParams;
use codex_app_server_protocol::CancelLoginChatGptParams;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientNotification;
use codex_app_server_protocol::CollaborationModeListParams;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::ExperimentalFeatureListParams;
use codex_app_server_protocol::FeedbackUploadParams;
use codex_app_server_protocol::ForkConversationParams;
use codex_app_server_protocol::GetAccountParams;
use codex_app_server_protocol::GetAuthStatusParams;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::InterruptConversationParams;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::ListConversationsParams;
use codex_app_server_protocol::LoginAccountParams;
use codex_app_server_protocol::LoginApiKeyParams;
use codex_app_server_protocol::MockExperimentalMethodParams;
use codex_app_server_protocol::ModelListParams;
use codex_app_server_protocol::NewConversationParams;
use codex_app_server_protocol::RemoveConversationListenerParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ResumeConversationParams;
use codex_app_server_protocol::ReviewStartParams;
use codex_app_server_protocol::SendUserMessageParams;
use codex_app_server_protocol::SendUserTurnParams;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::SetDefaultModelParams;
use codex_app_server_protocol::SkillsListParams;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadCloseParams;
use codex_app_server_protocol::ThreadCompactStartParams;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadRollbackParams;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadUnarchiveParams;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnSteerParams;
use codex_core::default_client::CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR;
use tokio::process::Command;

pub struct McpProcess {
    next_request_id: AtomicI64,
    /// Retain this child process until the client is dropped. The Tokio runtime
    /// will make a "best effort" to reap the process after it exits, but it is
    /// not a guarantee. See the `kill_on_drop` documentation for details.
    #[allow(dead_code)]
    process: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    pending_messages: VecDeque<JSONRPCMessage>,
}

pub const DEFAULT_CLIENT_NAME: &str = "codex-app-server-tests";

impl McpProcess {
    pub async fn new(codex_home: &Path) -> anyhow::Result<Self> {
        Self::new_with_env(codex_home, &[]).await
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
        let program = codex_utils_cargo_bin::cargo_bin("codex-app-server")
            .context("should find binary for codex-app-server")?;
        let mut cmd = Command::new(program);

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.env("CODEX_HOME", codex_home);
        cmd.env("RUST_LOG", "debug");
        cmd.env_remove(CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR);

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
        if let Some(stderr) = process.stderr.take() {
            let mut stderr_reader = BufReader::new(stderr).lines();
            tokio::spawn(async move {
                while let Ok(Some(line)) = stderr_reader.next_line().await {
                    eprintln!("[mcp stderr] {line}");
                }
            });
        }
        Ok(Self {
            next_request_id: AtomicI64::new(0),
            process,
            stdin,
            stdout,
            pending_messages: VecDeque::new(),
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

    /// Send a `newConversation` JSON-RPC request.
    pub async fn send_new_conversation_request(
        &mut self,
        params: NewConversationParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("newConversation", params).await
    }

    /// Send an `archiveConversation` JSON-RPC request.
    pub async fn send_archive_conversation_request(
        &mut self,
        params: ArchiveConversationParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("archiveConversation", params).await
    }

    /// Send an `addConversationListener` JSON-RPC request.
    pub async fn send_add_conversation_listener_request(
        &mut self,
        params: AddConversationListenerParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("addConversationListener", params).await
    }

    /// Send a `sendUserMessage` JSON-RPC request with a single text item.
    pub async fn send_send_user_message_request(
        &mut self,
        params: SendUserMessageParams,
    ) -> anyhow::Result<i64> {
        // Wire format expects variants in camelCase; text item uses external tagging.
        let params = Some(serde_json::to_value(params)?);
        self.send_request("sendUserMessage", params).await
    }

    /// Send a `removeConversationListener` JSON-RPC request.
    pub async fn send_remove_thread_listener_request(
        &mut self,
        params: RemoveConversationListenerParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("removeConversationListener", params)
            .await
    }

    /// Send a `sendUserTurn` JSON-RPC request.
    pub async fn send_send_user_turn_request(
        &mut self,
        params: SendUserTurnParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("sendUserTurn", params).await
    }

    /// Send a `interruptConversation` JSON-RPC request.
    pub async fn send_interrupt_conversation_request(
        &mut self,
        params: InterruptConversationParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("interruptConversation", params).await
    }

    /// Send a `getAuthStatus` JSON-RPC request.
    pub async fn send_get_auth_status_request(
        &mut self,
        params: GetAuthStatusParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("getAuthStatus", params).await
    }

    /// Send a `getUserSavedConfig` JSON-RPC request.
    pub async fn send_get_user_saved_config_request(&mut self) -> anyhow::Result<i64> {
        self.send_request("getUserSavedConfig", None).await
    }

    /// Send a `getUserAgent` JSON-RPC request.
    pub async fn send_get_user_agent_request(&mut self) -> anyhow::Result<i64> {
        self.send_request("getUserAgent", None).await
    }

    /// Send an `account/rateLimits/read` JSON-RPC request.
    pub async fn send_get_account_rate_limits_request(&mut self) -> anyhow::Result<i64> {
        self.send_request("account/rateLimits/read", None).await
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
        let params = Some(serde_json::to_value(params)?);
        self.send_request("account/login/start", params).await
    }

    /// Send a `feedback/upload` JSON-RPC request.
    pub async fn send_feedback_upload_request(
        &mut self,
        params: FeedbackUploadParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("feedback/upload", params).await
    }

    /// Send a `userInfo` JSON-RPC request.
    pub async fn send_user_info_request(&mut self) -> anyhow::Result<i64> {
        self.send_request("userInfo", None).await
    }

    /// Send a `setDefaultModel` JSON-RPC request.
    pub async fn send_set_default_model_request(
        &mut self,
        params: SetDefaultModelParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("setDefaultModel", params).await
    }

    /// Send a `listConversations` JSON-RPC request.
    pub async fn send_list_conversations_request(
        &mut self,
        params: ListConversationsParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("listConversations", params).await
    }

    /// Send a `thread/start` JSON-RPC request.
    pub async fn send_thread_start_request(
        &mut self,
        params: ThreadStartParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/start", params).await
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

    /// Send a `thread/close` JSON-RPC request.
    pub async fn send_thread_close_request(
        &mut self,
        params: ThreadCloseParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("thread/close", params).await
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

    /// Send a `model/list` JSON-RPC request.
    pub async fn send_list_models_request(
        &mut self,
        params: ModelListParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("model/list", params).await
    }

    /// Send an `experimentalFeature/list` JSON-RPC request.
    pub async fn send_experimental_feature_list_request(
        &mut self,
        params: ExperimentalFeatureListParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("experimentalFeature/list", params).await
    }

    /// Send an `app/list` JSON-RPC request.
    pub async fn send_apps_list_request(&mut self, params: AppsListParams) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("app/list", params).await
    }

    /// Send a `skills/list` JSON-RPC request.
    pub async fn send_skills_list_request(
        &mut self,
        params: SkillsListParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("skills/list", params).await
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

    /// Send a `resumeConversation` JSON-RPC request.
    pub async fn send_resume_conversation_request(
        &mut self,
        params: ResumeConversationParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("resumeConversation", params).await
    }

    /// Send a `forkConversation` JSON-RPC request.
    pub async fn send_fork_conversation_request(
        &mut self,
        params: ForkConversationParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("forkConversation", params).await
    }

    /// Send a `loginApiKey` JSON-RPC request.
    pub async fn send_login_api_key_request(
        &mut self,
        params: LoginApiKeyParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("loginApiKey", params).await
    }

    /// Send a `loginChatGpt` JSON-RPC request.
    pub async fn send_login_chat_gpt_request(&mut self) -> anyhow::Result<i64> {
        self.send_request("loginChatGpt", None).await
    }

    /// Send a `turn/start` JSON-RPC request (v2).
    pub async fn send_turn_start_request(
        &mut self,
        params: TurnStartParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("turn/start", params).await
    }

    /// Send a `turn/interrupt` JSON-RPC request (v2).
    pub async fn send_turn_interrupt_request(
        &mut self,
        params: TurnInterruptParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("turn/interrupt", params).await
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

    /// Send a `cancelLoginChatGpt` JSON-RPC request.
    pub async fn send_cancel_login_chat_gpt_request(
        &mut self,
        params: CancelLoginChatGptParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("cancelLoginChatGpt", params).await
    }

    /// Send a `logoutChatGpt` JSON-RPC request.
    pub async fn send_logout_chat_gpt_request(&mut self) -> anyhow::Result<i64> {
        self.send_request("logoutChatGpt", None).await
    }

    pub async fn send_config_read_request(
        &mut self,
        params: ConfigReadParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("config/read", params).await
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

    /// Send an `account/logout` JSON-RPC request.
    pub async fn send_logout_account_request(&mut self) -> anyhow::Result<i64> {
        self.send_request("account/logout", None).await
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
        self.send_request("account/login/start", Some(params)).await
    }

    /// Send an `account/login/start` JSON-RPC request for ChatGPT login.
    pub async fn send_login_account_chatgpt_request(&mut self) -> anyhow::Result<i64> {
        let params = serde_json::json!({
            "type": "chatgpt"
        });
        self.send_request("account/login/start", Some(params)).await
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
        let payload = serde_json::to_string(&message)?;
        self.stdin.write_all(payload.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;
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

    fn message_request_id(message: &JSONRPCMessage) -> Option<&RequestId> {
        match message {
            JSONRPCMessage::Request(request) => Some(&request.id),
            JSONRPCMessage::Response(response) => Some(&response.id),
            JSONRPCMessage::Error(err) => Some(&err.id),
            JSONRPCMessage::Notification(_) => None,
        }
    }
}

impl Drop for McpProcess {
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
        // 1. Request termination with `start_kill()`.
        // 2. Poll `try_wait()` until the OS reports the child exited, with a short timeout.
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
