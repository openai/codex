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
use codex_mcp_server::CodexToolCallParam;

use pretty_assertions::assert_eq;
use rmcp::model::CallToolRequestParams;
use rmcp::model::ClientCapabilities;
use rmcp::model::CustomNotification;
use rmcp::model::CustomRequest;
use rmcp::model::ElicitationCapability;
use rmcp::model::FormElicitationCapability;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::JsonRpcMessage;
use rmcp::model::JsonRpcNotification;
use rmcp::model::JsonRpcRequest;
use rmcp::model::JsonRpcResponse;
use rmcp::model::JsonRpcVersion2_0;
use rmcp::model::ProtocolVersion;
use rmcp::model::RequestId;
use serde_json::json;
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
        let program = codex_utils_cargo_bin::cargo_bin("codex-mcp-server")
            .context("should find binary for codex-mcp-server")?;
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
        })
    }

    /// Performs the initialization handshake with the MCP server.
    pub async fn initialize(&mut self) -> anyhow::Result<()> {
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);

        let params = InitializeRequestParams {
            meta: None,
            capabilities: ClientCapabilities {
                elicitation: Some(ElicitationCapability {
                    form: Some(FormElicitationCapability {
                        schema_validation: None,
                    }),
                    url: None,
                }),
                experimental: None,
                extensions: None,
                roots: None,
                sampling: None,
                tasks: None,
            },
            client_info: Implementation {
                name: "elicitation test".into(),
                title: Some("Elicitation Test".into()),
                version: "0.0.0".into(),
                description: None,
                icons: None,
                website_url: None,
            },
            protocol_version: ProtocolVersion::V_2025_03_26,
        };
        let params_value = serde_json::to_value(params)?;

        self.send_jsonrpc_message(JsonRpcMessage::Request(JsonRpcRequest {
            jsonrpc: JsonRpcVersion2_0,
            id: RequestId::Number(request_id),
            request: CustomRequest::new("initialize", Some(params_value)),
        }))
        .await?;

        let initialized = self.read_jsonrpc_message().await?;
        let os_info = os_info::get();
        let build_version = env!("CARGO_PKG_VERSION");
        let originator = codex_core::default_client::originator().value;
        let user_agent = format!(
            "{originator}/{build_version} ({} {}; {}) {} (elicitation test; 0.0.0)",
            os_info.os_type(),
            os_info.version(),
            os_info.architecture().unwrap_or("unknown"),
            codex_core::terminal::user_agent()
        );
        let JsonRpcMessage::Response(JsonRpcResponse {
            jsonrpc,
            id,
            result,
        }) = initialized
        else {
            anyhow::bail!("expected initialize response message, got: {initialized:?}")
        };
        assert_eq!(jsonrpc, JsonRpcVersion2_0);
        assert_eq!(id, RequestId::Number(request_id));
        assert_eq!(
            result,
            json!({
                "capabilities": {
                    "tools": {
                        "listChanged": true
                    },
                },
                "serverInfo": {
                    "name": "codex-mcp-server",
                    "title": "Codex",
                    "version": "0.0.0",
                    "user_agent": user_agent
                },
                "protocolVersion": ProtocolVersion::V_2025_03_26
            })
        );

        // Send notifications/initialized to ack the response.
        self.send_jsonrpc_message(JsonRpcMessage::Notification(JsonRpcNotification {
            jsonrpc: JsonRpcVersion2_0,
            notification: CustomNotification::new("notifications/initialized", None),
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
            meta: None,
            name: "codex".into(),
            arguments: Some(match serde_json::to_value(params)? {
                serde_json::Value::Object(map) => map,
                _ => unreachable!("params serialize to object"),
            }),
            task: None,
        };
        self.send_request(
            "tools/call",
            Some(serde_json::to_value(codex_tool_call_params)?),
        )
        .await
    }

    async fn send_request(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> anyhow::Result<i64> {
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);

        let message = JsonRpcMessage::Request(JsonRpcRequest {
            jsonrpc: JsonRpcVersion2_0,
            id: RequestId::Number(request_id),
            request: CustomRequest::new(method, params),
        });
        self.send_jsonrpc_message(message).await?;
        Ok(request_id)
    }

    pub async fn send_response(
        &mut self,
        id: RequestId,
        result: serde_json::Value,
    ) -> anyhow::Result<()> {
        self.send_jsonrpc_message(JsonRpcMessage::Response(JsonRpcResponse {
            jsonrpc: JsonRpcVersion2_0,
            id,
            result,
        }))
        .await
    }

    async fn send_jsonrpc_message(
        &mut self,
        message: JsonRpcMessage<CustomRequest, serde_json::Value, CustomNotification>,
    ) -> anyhow::Result<()> {
        eprintln!("writing message to stdin: {message:?}");
        let payload = serde_json::to_string(&message)?;
        self.stdin.write_all(payload.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn read_jsonrpc_message(
        &mut self,
    ) -> anyhow::Result<JsonRpcMessage<CustomRequest, serde_json::Value, CustomNotification>> {
        let mut line = String::new();
        self.stdout.read_line(&mut line).await?;
        let message = serde_json::from_str::<
            JsonRpcMessage<CustomRequest, serde_json::Value, CustomNotification>,
        >(&line)?;
        eprintln!("read message from stdout: {message:?}");
        Ok(message)
    }

    pub async fn read_stream_until_request_message(
        &mut self,
    ) -> anyhow::Result<JsonRpcRequest<CustomRequest>> {
        eprintln!("in read_stream_until_request_message()");

        loop {
            let message = self.read_jsonrpc_message().await?;

            match message {
                JsonRpcMessage::Notification(_) => {
                    eprintln!("notification: {message:?}");
                }
                JsonRpcMessage::Request(jsonrpc_request) => {
                    return Ok(jsonrpc_request);
                }
                JsonRpcMessage::Error(_) => {
                    anyhow::bail!("unexpected JSONRPCMessage::Error: {message:?}");
                }
                JsonRpcMessage::Response(_) => {
                    anyhow::bail!("unexpected JSONRPCMessage::Response: {message:?}");
                }
            }
        }
    }

    pub async fn read_stream_until_response_message(
        &mut self,
        request_id: RequestId,
    ) -> anyhow::Result<JsonRpcResponse<serde_json::Value>> {
        eprintln!("in read_stream_until_response_message({request_id:?})");

        loop {
            let message = self.read_jsonrpc_message().await?;
            match message {
                JsonRpcMessage::Notification(_) => {
                    eprintln!("notification: {message:?}");
                }
                JsonRpcMessage::Request(_) => {
                    anyhow::bail!("unexpected JSONRPCMessage::Request: {message:?}");
                }
                JsonRpcMessage::Error(_) => {
                    anyhow::bail!("unexpected JSONRPCMessage::Error: {message:?}");
                }
                JsonRpcMessage::Response(jsonrpc_response) => {
                    if jsonrpc_response.id == request_id {
                        return Ok(jsonrpc_response);
                    }
                }
            }
        }
    }

    /// Reads notifications until a legacy TurnComplete event is observed:
    /// Method "codex/event" with params.msg.type == "task_complete".
    pub async fn read_stream_until_legacy_task_complete_notification(
        &mut self,
    ) -> anyhow::Result<JsonRpcNotification<CustomNotification>> {
        eprintln!("in read_stream_until_legacy_task_complete_notification()");

        loop {
            let message = self.read_jsonrpc_message().await?;
            match message {
                JsonRpcMessage::Notification(notification) => {
                    let is_match = if notification.notification.method == "codex/event" {
                        if let Some(params) = &notification.notification.params {
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
                    } else {
                        eprintln!("ignoring notification: {notification:?}");
                    }
                }
                JsonRpcMessage::Request(_) => {
                    anyhow::bail!("unexpected JSONRPCMessage::Request: {message:?}");
                }
                JsonRpcMessage::Error(_) => {
                    anyhow::bail!("unexpected JSONRPCMessage::Error: {message:?}");
                }
                JsonRpcMessage::Response(_) => {
                    anyhow::bail!("unexpected JSONRPCMessage::Response: {message:?}");
                }
            }
        }
    }
}

impl Drop for McpProcess {
    fn drop(&mut self) {
        // These tests spawn a `codex-mcp-server` child process.
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
