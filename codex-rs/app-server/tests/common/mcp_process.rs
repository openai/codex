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
use assert_cmd::prelude::*;
use codex_app_server_protocol::AddConversationListenerParams;
use codex_app_server_protocol::ArchiveConversationParams;
use codex_app_server_protocol::CancelLoginChatGptParams;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientNotification;
use codex_app_server_protocol::GetAuthStatusParams;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::InterruptConversationParams;
use codex_app_server_protocol::ListConversationsParams;
use codex_app_server_protocol::LoginApiKeyParams;
use codex_app_server_protocol::NewConversationParams;
use codex_app_server_protocol::RemoveConversationListenerParams;
use codex_app_server_protocol::ResumeConversationParams;
use codex_app_server_protocol::SendUserMessageParams;
use codex_app_server_protocol::SendUserTurnParams;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::SetDefaultModelParams;

use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use std::process::Command as StdCommand;
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
    pending_user_messages: VecDeque<JSONRPCNotification>,
}

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
        // Use assert_cmd to locate the binary path and then switch to tokio::process::Command
        let std_cmd = StdCommand::cargo_bin("codex-app-server")
            .context("should find binary for codex-mcp-server")?;

        let program = std_cmd.get_program().to_owned();

        let mut cmd = Command::new(program);

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.env("CODEX_HOME", codex_home);
        cmd.env("RUST_LOG", "debug");

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
            pending_user_messages: VecDeque::new(),
        })
    }

    /// Performs the initialization handshake with the MCP server.
    pub async fn initialize(&mut self) -> anyhow::Result<()> {
        let params = Some(serde_json::to_value(InitializeParams {
            client_info: ClientInfo {
                name: "codex-app-server-tests".to_string(),
                title: None,
                version: "0.1.0".to_string(),
            },
        })?);
        let req_id = self.send_request("initialize", params).await?;
        let initialized = self.read_jsonrpc_message().await?;
        let JSONRPCMessage::Response(response) = initialized else {
            unreachable!("expected JSONRPCMessage::Response for initialize, got {initialized:?}");
        };
        if response.id != RequestId::Integer(req_id) {
            anyhow::bail!(
                "initialize response id mismatch: expected {}, got {:?}",
                req_id,
                response.id
            );
        }

        // Send notifications/initialized to ack the response.
        self.send_notification(ClientNotification::Initialized)
            .await?;

        Ok(())
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
    pub async fn send_remove_conversation_listener_request(
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

    /// Send a `resumeConversation` JSON-RPC request.
    pub async fn send_resume_conversation_request(
        &mut self,
        params: ResumeConversationParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("resumeConversation", params).await
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

        loop {
            let message = self.read_jsonrpc_message().await?;

            match message {
                JSONRPCMessage::Notification(notification) => {
                    eprintln!("notification: {notification:?}");
                    self.enqueue_user_message(notification);
                }
                JSONRPCMessage::Request(jsonrpc_request) => {
                    return jsonrpc_request.try_into().with_context(
                        || "failed to deserialize ServerRequest from JSONRPCRequest",
                    );
                }
                JSONRPCMessage::Error(_) => {
                    anyhow::bail!("unexpected JSONRPCMessage::Error: {message:?}");
                }
                JSONRPCMessage::Response(_) => {
                    anyhow::bail!("unexpected JSONRPCMessage::Response: {message:?}");
                }
            }
        }
    }

    pub async fn read_stream_until_response_message(
        &mut self,
        request_id: RequestId,
    ) -> anyhow::Result<JSONRPCResponse> {
        eprintln!("in read_stream_until_response_message({request_id:?})");

        loop {
            let message = self.read_jsonrpc_message().await?;
            match message {
                JSONRPCMessage::Notification(notification) => {
                    eprintln!("notification: {notification:?}");
                    self.enqueue_user_message(notification);
                }
                JSONRPCMessage::Request(_) => {
                    anyhow::bail!("unexpected JSONRPCMessage::Request: {message:?}");
                }
                JSONRPCMessage::Error(_) => {
                    anyhow::bail!("unexpected JSONRPCMessage::Error: {message:?}");
                }
                JSONRPCMessage::Response(jsonrpc_response) => {
                    if jsonrpc_response.id == request_id {
                        return Ok(jsonrpc_response);
                    }
                }
            }
        }
    }

    pub async fn read_stream_until_error_message(
        &mut self,
        request_id: RequestId,
    ) -> anyhow::Result<JSONRPCError> {
        loop {
            let message = self.read_jsonrpc_message().await?;
            match message {
                JSONRPCMessage::Notification(notification) => {
                    eprintln!("notification: {notification:?}");
                    self.enqueue_user_message(notification);
                }
                JSONRPCMessage::Request(_) => {
                    anyhow::bail!("unexpected JSONRPCMessage::Request: {message:?}");
                }
                JSONRPCMessage::Response(_) => {
                    // Keep scanning; we're waiting for an error with matching id.
                }
                JSONRPCMessage::Error(err) => {
                    if err.id == request_id {
                        return Ok(err);
                    }
                }
            }
        }
    }

    pub async fn read_stream_until_notification_message(
        &mut self,
        method: &str,
    ) -> anyhow::Result<JSONRPCNotification> {
        eprintln!("in read_stream_until_notification_message({method})");

        if let Some(notification) = self.take_pending_notification_by_method(method) {
            return Ok(notification);
        }

        loop {
            let message = self.read_jsonrpc_message().await?;
            match message {
                JSONRPCMessage::Notification(notification) => {
                    if notification.method == method {
                        return Ok(notification);
                    }
                    self.enqueue_user_message(notification);
                }
                JSONRPCMessage::Request(_) => {
                    anyhow::bail!("unexpected JSONRPCMessage::Request: {message:?}");
                }
                JSONRPCMessage::Error(_) => {
                    anyhow::bail!("unexpected JSONRPCMessage::Error: {message:?}");
                }
                JSONRPCMessage::Response(_) => {
                    anyhow::bail!("unexpected JSONRPCMessage::Response: {message:?}");
                }
            }
        }
    }

    fn take_pending_notification_by_method(&mut self, method: &str) -> Option<JSONRPCNotification> {
        if let Some(pos) = self
            .pending_user_messages
            .iter()
            .position(|notification| notification.method == method)
        {
            return self.pending_user_messages.remove(pos);
        }
        None
    }

    fn enqueue_user_message(&mut self, notification: JSONRPCNotification) {
        if notification.method == "codex/event/user_message" {
            self.pending_user_messages.push_back(notification);
        }
    }
}
