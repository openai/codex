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
use codex_mcp_server::CodexToolCallParam;
use codex_protocol::mcp_protocol::AddConversationListenerParams;
use codex_protocol::mcp_protocol::CancelLoginChatGptParams;
use codex_protocol::mcp_protocol::GetAuthStatusParams;
use codex_protocol::mcp_protocol::InterruptConversationParams;
use codex_protocol::mcp_protocol::NewConversationParams;
use codex_protocol::mcp_protocol::RemoveConversationListenerParams;
use codex_protocol::mcp_protocol::SendUserMessageParams;
use codex_protocol::mcp_protocol::SendUserTurnParams;

use mcp_types::CallToolRequestParams;
use mcp_types::ClientCapabilities;
use mcp_types::Implementation;
use mcp_types::InitializeRequestParams;
use mcp_types::JSONRPC_VERSION;
use mcp_types::JSONRPCMessage;
use mcp_types::JSONRPCNotification;
use mcp_types::JSONRPCRequest;
use mcp_types::JSONRPCResponse;
use mcp_types::ModelContextProtocolNotification;
use mcp_types::ModelContextProtocolRequest;
use mcp_types::RequestId;
use pretty_assertions::assert_eq;
use serde_json::json;
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
}

impl McpProcess {
    pub async fn new(codex_home: &Path) -> anyhow::Result<Self> {
        // Use assert_cmd to locate the binary path and then switch to tokio::process::Command
        let std_cmd = StdCommand::cargo_bin("codex-mcp-server")
            .context("should find binary for codex-mcp-server")?;

        let program = std_cmd.get_program().to_owned();

        let mut cmd = Command::new(program);

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.env("CODEX_HOME", codex_home);
        cmd.env("RUST_LOG", "debug");

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
        Ok(Self {
            next_request_id: AtomicI64::new(0),
            process,
            stdin,
            stdout,
        })
    }

    /// Performs the initialization handshake with the MCP server.
    pub async fn initialize(&mut self) -> anyhow::Result<()> {
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);

        let params = InitializeRequestParams {
            capabilities: ClientCapabilities {
                elicitation: Some(json!({})),
                experimental: None,
                roots: None,
                sampling: None,
            },
            client_info: Implementation {
                name: "elicitation test".into(),
                title: Some("Elicitation Test".into()),
                version: "0.0.0".into(),
            },
            protocol_version: mcp_types::MCP_SCHEMA_VERSION.into(),
        };
        let params_value = serde_json::to_value(params)?;

        self.send_jsonrpc_message(JSONRPCMessage::Request(JSONRPCRequest {
            jsonrpc: JSONRPC_VERSION.into(),
            id: RequestId::Integer(request_id),
            method: mcp_types::InitializeRequest::METHOD.into(),
            params: Some(params_value),
        }))
        .await?;

        let initialized = self.read_jsonrpc_message().await?;
        assert_eq!(
            JSONRPCMessage::Response(JSONRPCResponse {
                jsonrpc: JSONRPC_VERSION.into(),
                id: RequestId::Integer(request_id),
                result: json!({
                    "capabilities": {
                        "tools": {
                            "listChanged": true
                        },
                    },
                    "serverInfo": {
                        "name": "codex-mcp-server",
                        "title": "Codex",
                        "version": "0.0.0"
                    },
                    "protocolVersion": mcp_types::MCP_SCHEMA_VERSION
                })
            }),
            initialized
        );

        // Send notifications/initialized to ack the response.
        self.send_jsonrpc_message(JSONRPCMessage::Notification(JSONRPCNotification {
            jsonrpc: JSONRPC_VERSION.into(),
            method: mcp_types::InitializedNotification::METHOD.into(),
            params: None,
        }))
        .await?;

        Ok(())
    }

    /// Returns the id used to make the request so it can be used when
    /// correlating notifications.
    pub async fn send_codex_tool_call(
        &mut self,
        params: CodexToolCallParam,
    ) -> anyhow::Result<i64> {
        let codex_tool_call_params = CallToolRequestParams {
            name: "codex".to_string(),
            arguments: Some(serde_json::to_value(params)?),
        };
        self.send_request(
            mcp_types::CallToolRequest::METHOD,
            Some(serde_json::to_value(codex_tool_call_params)?),
        )
        .await
    }

    /// Send a `newConversation` JSON-RPC request.
    pub async fn send_new_conversation_request(
        &mut self,
        params: NewConversationParams,
    ) -> anyhow::Result<i64> {
        let params = Some(serde_json::to_value(params)?);
        self.send_request("newConversation", params).await
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

    async fn send_request(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> anyhow::Result<i64> {
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);

        let message = JSONRPCMessage::Request(JSONRPCRequest {
            jsonrpc: JSONRPC_VERSION.into(),
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
        self.send_jsonrpc_message(JSONRPCMessage::Response(JSONRPCResponse {
            jsonrpc: JSONRPC_VERSION.into(),
            id,
            result,
        }))
        .await
    }

    async fn send_jsonrpc_message(&mut self, message: JSONRPCMessage) -> anyhow::Result<()> {
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
        Ok(message)
    }

    pub async fn read_stream_until_request_message(&mut self) -> anyhow::Result<JSONRPCRequest> {
        loop {
            let message = self.read_jsonrpc_message().await?;
            eprint!("message: {message:?}");

            match message {
                JSONRPCMessage::Notification(_) => {
                    eprintln!("notification: {message:?}");
                }
                JSONRPCMessage::Request(jsonrpc_request) => {
                    return Ok(jsonrpc_request);
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
        loop {
            let message = self.read_jsonrpc_message().await?;
            eprint!("message: {message:?}");

            match message {
                JSONRPCMessage::Notification(_) => {
                    eprintln!("notification: {message:?}");
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
    ) -> anyhow::Result<mcp_types::JSONRPCError> {
        loop {
            let message = self.read_jsonrpc_message().await?;
            eprint!("message: {message:?}");

            match message {
                JSONRPCMessage::Notification(_) => {
                    eprintln!("notification: {message:?}");
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
        loop {
            let message = self.read_jsonrpc_message().await?;
            eprint!("message: {message:?}");

            match message {
                JSONRPCMessage::Notification(notification) => {
                    if notification.method == method {
                        return Ok(notification);
                    }
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

    /// Reads notifications until a legacy TaskComplete event is observed:
    /// Method "codex/event" with params.msg.type == "task_complete".
    pub async fn read_stream_until_legacy_task_complete_notification(
        &mut self,
    ) -> anyhow::Result<JSONRPCNotification> {
        loop {
            let message = self.read_jsonrpc_message().await?;
            eprint!("message: {message:?}");

            match message {
                JSONRPCMessage::Notification(notification) => {
                    let is_match = if notification.method == "codex/event" {
                        if let Some(params) = &notification.params {
                            params
                                .get("msg")
                                .and_then(|m| m.get("type"))
                                .and_then(|t| t.as_str())
                                == Some("task_complete")
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if is_match {
                        return Ok(notification);
                    }
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
}
