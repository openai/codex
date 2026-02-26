use futures::Stream;
use futures::StreamExt;
use reqwest::Client;
use reqwest::header::AUTHORIZATION;
use reqwest::header::HeaderValue;
use tokio_stream::wrappers::ReceiverStream;

use crate::error::AgentexError;
use crate::translate::types::CreateTaskParams;
use crate::translate::types::JsonRpcRequest;
use crate::translate::types::JsonRpcResponse;
use crate::translate::types::MessageSendParams;
use crate::translate::types::MessageSendResult;
use crate::translate::types::Task;
use crate::translate::types::TaskMessageUpdate;

/// HTTP client for Agentex JSON-RPC endpoints.
pub struct AgentexClient {
    client: Client,
    base_url: String,
    auth_header: &'static str,
}

impl AgentexClient {
    pub fn new(base_url: String, auth_header: &'static str) -> Self {
        let client = Client::builder()
            .build()
            .unwrap_or_default();

        Self {
            client,
            base_url,
            auth_header,
        }
    }

    fn rpc_url(&self) -> String {
        format!("{}/rpc", self.base_url.trim_end_matches('/'))
    }

    fn auth_value(&self) -> HeaderValue {
        let mut v = HeaderValue::from_static(self.auth_header);
        v.set_sensitive(true);
        v
    }

    /// Create a new task on the Agentex agent.
    pub async fn task_create(
        &self,
        name: &str,
        agent_id: &str,
    ) -> Result<String, AgentexError> {
        let rpc = JsonRpcRequest::new(
            uuid::Uuid::new_v4().to_string(),
            "task/create",
            CreateTaskParams {
                name: name.to_string(),
                agent_id: agent_id.to_string(),
            },
        );

        let resp = self
            .client
            .post(self.rpc_url())
            .header(AUTHORIZATION, self.auth_value())
            .json(&rpc)
            .send()
            .await?;

        let body: JsonRpcResponse<Task> = resp.json().await?;

        if let Some(err) = body.error {
            return Err(AgentexError::Rpc {
                code: err.code,
                message: err.message,
            });
        }

        body.result
            .map(|t| t.id)
            .ok_or_else(|| AgentexError::Parse("missing result in task/create response".into()))
    }

    /// Send a message and receive the full response (non-streaming).
    pub async fn message_send(
        &self,
        params: MessageSendParams,
    ) -> Result<MessageSendResult, AgentexError> {
        let rpc = JsonRpcRequest::new(
            uuid::Uuid::new_v4().to_string(),
            "message/send",
            params,
        );

        let resp = self
            .client
            .post(self.rpc_url())
            .header(AUTHORIZATION, self.auth_value())
            .json(&rpc)
            .send()
            .await?;

        let body: JsonRpcResponse<MessageSendResult> = resp.json().await?;

        if let Some(err) = body.error {
            return Err(AgentexError::Rpc {
                code: err.code,
                message: err.message,
            });
        }

        body.result.ok_or_else(|| {
            AgentexError::Parse("missing result in message/send response".into())
        })
    }

    /// Send a message and receive a stream of `TaskMessageUpdate` events.
    ///
    /// The implementation handles both NDJSON and SSE-style `data:` prefixed
    /// lines from the response body, accommodating either Agentex wire format.
    pub async fn message_send_stream(
        &self,
        params: MessageSendParams,
    ) -> Result<impl Stream<Item = Result<TaskMessageUpdate, AgentexError>> + Send + 'static, AgentexError> {
        let rpc = JsonRpcRequest::new(
            uuid::Uuid::new_v4().to_string(),
            "message/send",
            params,
        );

        let resp = self
            .client
            .post(self.rpc_url())
            .header(AUTHORIZATION, self.auth_value())
            .json(&rpc)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AgentexError::Stream(format!(
                "HTTP {status}: {text}"
            )));
        }

        let mut byte_stream = resp.bytes_stream();
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<TaskMessageUpdate, AgentexError>>(64);

        tokio::spawn(async move {
            let mut line_buf = String::new();

            while let Some(chunk_result) = byte_stream.next().await {
                let chunk = match chunk_result {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(Err(AgentexError::Http(e))).await;
                        return;
                    }
                };

                let text = match std::str::from_utf8(&chunk) {
                    Ok(t) => t,
                    Err(e) => {
                        let _ = tx
                            .send(Err(AgentexError::Parse(format!("invalid UTF-8: {e}"))))
                            .await;
                        return;
                    }
                };

                line_buf.push_str(text);

                // Process complete lines.
                while let Some(newline_pos) = line_buf.find('\n') {
                    let line: String = line_buf.drain(..=newline_pos).collect();
                    let line = line.trim();

                    if line.is_empty() {
                        continue;
                    }

                    // Handle SSE `data: {...}` or raw NDJSON `{...}`.
                    let json_str = line.strip_prefix("data:").map(str::trim).unwrap_or(line);

                    // Skip SSE event-type lines like `event: ...`
                    if json_str.starts_with("event:") {
                        continue;
                    }

                    match serde_json::from_str::<TaskMessageUpdate>(json_str) {
                        Ok(update) => {
                            if tx.send(Ok(update)).await.is_err() {
                                return;
                            }
                        }
                        Err(e) => {
                            // Skip lines that don't parse (e.g., SSE comments).
                            tracing::debug!("skipping unparseable stream line: {e}");
                        }
                    }
                }
            }

            // Process any remaining data in the buffer.
            let remaining = line_buf.trim();
            if !remaining.is_empty() {
                let json_str = remaining
                    .strip_prefix("data:")
                    .map(str::trim)
                    .unwrap_or(remaining);
                if let Ok(update) = serde_json::from_str::<TaskMessageUpdate>(json_str) {
                    let _ = tx.send(Ok(update)).await;
                }
            }
        });

        Ok(ReceiverStream::new(rx))
    }
}
