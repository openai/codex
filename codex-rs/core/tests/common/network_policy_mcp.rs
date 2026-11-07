//! Mocks one trusted MCP HTTP endpoint while forwarding real executor RPCs unchanged.

use crate::exec_server::ExecServerProcess;
use crate::is_remote_test_environment;
use anyhow::Context;
use codex_config::types::McpServerConfig;
use codex_exec_server::HttpHeader;
use codex_exec_server::HttpRequestParams;
use codex_exec_server::HttpRequestResponse;
use codex_exec_server::REMOTE_ENVIRONMENT_ID;
use futures::SinkExt;
use futures::StreamExt;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::task::JoinSet;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::http::Uri;

pub const TRUSTED_MCP_URL: &str = "https://chatgpt.com/backend-api/ps/mcp";

pub struct NetworkPolicyMcp {
    pub executor_url: String,
    task: JoinHandle<anyhow::Result<()>>,
    _executor: Option<ExecServerProcess>,
}

impl NetworkPolicyMcp {
    pub fn server_config() -> McpServerConfig {
        serde_json::from_value(json!({
            "url": TRUSTED_MCP_URL,
            "environment_id": REMOTE_ENVIRONMENT_ID,
        }))
        .expect("trusted network policy MCP configuration")
    }

    pub async fn start_with_capabilities(experimental: Value) -> anyhow::Result<Self> {
        let experimental = Arc::new(experimental);
        let executor = if is_remote_test_environment() {
            None
        } else {
            Some(ExecServerProcess::start().await?)
        };
        let upstream_url = match &executor {
            Some(executor) => executor.websocket_url.clone(),
            None => std::env::var("CODEX_TEST_REMOTE_EXEC_SERVER_URL")?,
        };
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let executor_url = format!("ws://{}", listener.local_addr()?);
        let task = tokio::spawn(async move {
            let mut sessions = JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let (stream, _) = accepted?;
                        let upstream_url = upstream_url.clone();
                        let experimental = Arc::clone(&experimental);
                        sessions.spawn(async move {
                            let mut downstream = accept_async(stream).await?;
                            let (mut upstream, _) = connect_async(upstream_url).await?;
                            let mut responses = JoinSet::new();
                            loop {
                                tokio::select! {
                                    frame = downstream.next() => {
                                        let Some(frame) = frame else { break; };
                                        let frame = frame?;
                                        if frame.is_close() { break; }
                                        let request = match &frame {
                                            Message::Text(text) => serde_json::from_str::<Value>(text).ok(),
                                            Message::Binary(bytes) => serde_json::from_slice(bytes).ok(),
                                            _ => None,
                                        };
                                        if let Some(request) = request
                                            && request["method"] == "http/request"
                                        {
                                            let url: Uri = request["params"]["url"].as_str().context("HTTP request URL")?.parse()?;
                                            anyhow::ensure!(
                                                url.host().is_none_or(|host| !host.eq_ignore_ascii_case("chatgpt.com"))
                                                    || request["params"]["url"] == TRUSTED_MCP_URL,
                                                "unsupported request to ChatGPT fixture origin: {url}"
                                            );
                                            if request["params"]["url"] == TRUSTED_MCP_URL {
                                                responses.spawn(respond(request, Arc::clone(&experimental)));
                                                continue;
                                            }
                                        }
                                        upstream.send(frame).await?;
                                    }
                                    frame = upstream.next() => {
                                        let Some(frame) = frame else { break; };
                                        let frame = frame?;
                                        if frame.is_close() { break; }
                                        downstream.send(frame).await?;
                                    }
                                    Some(response) = responses.join_next(), if !responses.is_empty() => {
                                        for message in response?? {
                                            downstream.send(Message::Text(message.to_string().into())).await?;
                                        }
                                    }
                                    else => break,
                                }
                            }
                            anyhow::Ok(())
                        });
                    }
                    Some(session) = sessions.join_next(), if !sessions.is_empty() => session??,
                }
            }
        });
        Ok(Self {
            executor_url,
            task,
            _executor: executor,
        })
    }
}

impl Drop for NetworkPolicyMcp {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn respond(request: Value, experimental: Arc<Value>) -> anyhow::Result<Vec<Value>> {
    let params: HttpRequestParams = serde_json::from_value(request["params"].clone())?;
    anyhow::ensure!(params.url == TRUSTED_MCP_URL, "unexpected MCP endpoint");
    let mut response = HttpRequestResponse {
        status: 405,
        headers: Vec::new(),
        body: Vec::new().into(),
    };
    if params.method == "POST" {
        let body: Value =
            serde_json::from_slice(params.body.as_ref().context("MCP body")?.as_ref())?;
        let result = match body["method"].as_str() {
            Some("initialize") => json!({
                "protocolVersion": body["params"]["protocolVersion"],
                "capabilities": {"tools": {}, "experimental": *experimental},
                "serverInfo": {"name": "network-policy", "version": "1"}
            }),
            Some("tools/list") => json!({"tools": [{
                "name": "network_policy_meta", "inputSchema": {"type": "object"},
                "annotations": {"readOnlyHint": true}
            }]}),
            Some("tools/call") => {
                anyhow::ensure!(
                    body["params"]["name"] == "network_policy_meta",
                    "unknown MCP tool"
                );
                let metadata = body["params"]["_meta"].clone();
                json!({"content": [{"type": "text", "text": metadata.to_string()}],
                    "structuredContent": metadata, "isError": false})
            }
            Some("notifications/initialized") => Value::Null,
            method => anyhow::bail!("unexpected MCP method {method:?}"),
        };
        response.status = if body.get("id").is_some() { 200 } else { 202 };
        response.headers = vec![HttpHeader {
            name: "content-type".to_string(),
            value: "application/json".to_string(),
            value_env_var: None,
        }];
        if response.status == 200 {
            response.body =
                serde_json::to_vec(&json!({"jsonrpc": "2.0", "id": body["id"], "result": result}))?
                    .into();
        }
    }
    let body = params
        .stream_response
        .then(|| std::mem::replace(&mut response.body, Vec::new().into()));
    let mut messages = vec![json!({"id": request["id"], "result": response})];
    if let Some(body) = body {
        messages.push(json!({"method": "http/request/bodyDelta", "params": {
            "requestId": params.request_id, "seq": 1, "deltaBase64": body, "done": true
        }}));
    }
    Ok(messages)
}
