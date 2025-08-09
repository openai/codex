use std::path::Path;
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
use assert_cmd::prelude::*;
use codex_core::protocol::InputItem;
use codex_mcp_server::CodexToolCallParam;
use codex_mcp_server::CodexToolCallReplyParam;
use codex_mcp_server::mcp_protocol::ConversationCreateArgs;
use codex_mcp_server::mcp_protocol::ConversationId;
use codex_mcp_server::mcp_protocol::ConversationSendMessageArgs;
use codex_mcp_server::mcp_protocol::ConversationStreamArgs;
use codex_mcp_server::mcp_protocol::ToolCallRequestParams;

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
use uuid::Uuid;

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

    pub async fn send_codex_reply_tool_call(
        &mut self,
        session_id: &str,
        prompt: &str,
    ) -> anyhow::Result<i64> {
        let codex_tool_call_params = CallToolRequestParams {
            name: "codex-reply".to_string(),
            arguments: Some(serde_json::to_value(CodexToolCallReplyParam {
                prompt: prompt.to_string(),
                session_id: session_id.to_string(),
            })?),
        };
        self.send_request(
            mcp_types::CallToolRequest::METHOD,
            Some(serde_json::to_value(codex_tool_call_params)?),
        )
        .await
    }

    pub async fn send_user_message_tool_call(
        &mut self,
        message: &str,
        session_id: &str,
    ) -> anyhow::Result<i64> {
        let params = ToolCallRequestParams::ConversationSendMessage(ConversationSendMessageArgs {
            conversation_id: ConversationId(Uuid::parse_str(session_id)?),
            content: vec![InputItem::Text {
                text: message.to_string(),
            }],
            parent_message_id: None,
            conversation_overrides: None,
        });
        self.send_request(
            mcp_types::CallToolRequest::METHOD,
            Some(serde_json::to_value(params)?),
        )
        .await
    }

    pub async fn send_conversation_stream_tool_call(
        &mut self,
        session_id: &str,
    ) -> anyhow::Result<i64> {
        let params = ToolCallRequestParams::ConversationStream(ConversationStreamArgs {
            conversation_id: ConversationId(Uuid::parse_str(session_id)?),
        });
        self.send_request(
            mcp_types::CallToolRequest::METHOD,
            Some(serde_json::to_value(params)?),
        )
        .await
    }

    pub async fn send_conversation_create_tool_call(
        &mut self,
        prompt: &str,
        model: &str,
        cwd: &str,
    ) -> anyhow::Result<i64> {
        let params = ToolCallRequestParams::ConversationCreate(ConversationCreateArgs {
            prompt: prompt.to_string(),
            model: model.to_string(),
            cwd: cwd.to_string(),
            approval_policy: None,
            sandbox: None,
            config: None,
            profile: None,
            base_instructions: None,
        });
        self.send_request(
            mcp_types::CallToolRequest::METHOD,
            Some(serde_json::to_value(params)?),
        )
        .await
    }

    pub async fn send_conversation_create_with_args(
        &mut self,
        args: ConversationCreateArgs,
    ) -> anyhow::Result<i64> {
        let params = ToolCallRequestParams::ConversationCreate(args);
        self.send_request(
            mcp_types::CallToolRequest::METHOD,
            Some(serde_json::to_value(params)?),
        )
        .await
    }

    /// Create a conversation and return its conversation_id as a string.
    pub async fn create_conversation_and_get_id(
        &mut self,
        prompt: &str,
        model: &str,
        cwd: &str,
    ) -> anyhow::Result<String> {
        let req_id = self
            .send_conversation_create_tool_call(prompt, model, cwd)
            .await?;
        let resp = self
            .read_stream_until_response_message(RequestId::Integer(req_id))
            .await?;
        let conv_id = resp.result["structuredContent"]["conversation_id"]
            .as_str()
            .ok_or_else(|| anyhow::format_err!("missing conversation_id"))?
            .to_string();
        Ok(conv_id)
    }

    /// Connect stream for a conversation and wait for the initial_state notification.
    /// Returns (requestId, params) where params are the initial_state notification params.
    pub async fn connect_stream_and_expect_initial_state(
        &mut self,
        session_id: &str,
    ) -> anyhow::Result<(i64, serde_json::Value)> {
        let req_id = self.send_conversation_stream_tool_call(session_id).await?;
        // Wait for stream() tool-call response first
        let _ = self
            .read_stream_until_response_message(RequestId::Integer(req_id))
            .await?;
        // Then the initial_state notification
        let note = self
            .read_stream_until_notification_method("notifications/initial_state")
            .await?;
        let params = note
            .params
            .ok_or_else(|| anyhow::format_err!("initial_state must have params"))?;
        Ok((req_id, params))
    }

    /// Wait for an agent_message with a bounded timeout. Returns Some(params) if received, None on timeout.
    pub async fn maybe_wait_for_agent_message(
        &mut self,
        dur: Duration,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        match tokio::time::timeout(dur, self.wait_for_agent_message()).await {
            Ok(Ok(v)) => Ok(Some(v)),
            Ok(Err(e)) => Err(e),
            Err(_elapsed) => Ok(None),
        }
    }

    /// Send a user message to a conversation and wait for the OK tool-call response.
    pub async fn send_user_message_and_wait_ok(
        &mut self,
        message: &str,
        session_id: &str,
    ) -> anyhow::Result<()> {
        let req_id = self
            .send_user_message_tool_call(message, session_id)
            .await?;
        let _ = self
            .read_stream_until_response_message(RequestId::Integer(req_id))
            .await?;
        Ok(())
    }

    /// Wait until an agent_message notification arrives; returns its params.
    pub async fn wait_for_agent_message(&mut self) -> anyhow::Result<serde_json::Value> {
        let note = self
            .read_stream_until_notification_method("agent_message")
            .await?;
        note.params
            .ok_or_else(|| anyhow::format_err!("agent_message missing params"))
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

    pub async fn read_stream_until_notification_method(
        &mut self,
        method: &str,
    ) -> anyhow::Result<JSONRPCNotification> {
        loop {
            let message = self.read_jsonrpc_message().await?;
            match message {
                JSONRPCMessage::Notification(n) => {
                    if n.method == method {
                        return Ok(n);
                    }
                }
                JSONRPCMessage::Request(_) => {
                    // ignore
                }
                JSONRPCMessage::Error(_) => {
                    anyhow::bail!("unexpected JSONRPCMessage::Error: {message:?}");
                }
                JSONRPCMessage::Response(_) => {
                    // ignore
                }
            }
        }
    }

    pub async fn read_stream_until_configured_response_message(
        &mut self,
    ) -> anyhow::Result<String> {
        loop {
            let message = self.read_jsonrpc_message().await?;
            eprint!("message: {message:?}");

            match message {
                JSONRPCMessage::Notification(notification) => {
                    if notification.method == "session_configured" {
                        if let Some(params) = notification.params {
                            if let Some(msg) = params.get("msg") {
                                if let Some(session_id) =
                                    msg.get("session_id").and_then(|v| v.as_str())
                                {
                                    return Ok(session_id.to_string());
                                }
                            }
                        }
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

    pub async fn send_notification(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> anyhow::Result<()> {
        self.send_jsonrpc_message(JSONRPCMessage::Notification(JSONRPCNotification {
            jsonrpc: JSONRPC_VERSION.into(),
            method: method.to_string(),
            params,
        }))
        .await
    }
}
