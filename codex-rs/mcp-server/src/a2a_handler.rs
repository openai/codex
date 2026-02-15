//! Codex A2A handler — implements [`a2a_rs::AgentExecutor`] to bridge
//! A2A messages into the Codex MCP [`MessageProcessor`] via `tools/call`.

use std::collections::HashMap;
use std::sync::Arc;

use a2a_rs::{
    A2AError, AgentCapabilities, AgentCard, AgentExecutor, AgentInterface,
    AgentProvider, AgentSkill, EventBus, ExecutionEvent, RequestContext,
    SendMessageResponse,
    completed_task, failed_task,
};
use serde_json::json;
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::info;

use crate::outgoing_message::OutgoingJsonRpcMessage;

/// Codex A2A executor — implements [`AgentExecutor`] from `a2a-rs`.
pub struct CodexA2AExecutor {
    /// Sender to feed JSON‐RPC messages into the shared MessageProcessor.
    pub incoming_tx: mpsc::Sender<crate::IncomingMessage>,
    /// Pending MCP request IDs → oneshot senders for responses.
    pub pending:
        Arc<Mutex<HashMap<String, oneshot::Sender<OutgoingJsonRpcMessage>>>>,
}

impl CodexA2AExecutor {
    pub fn new(
        incoming_tx: mpsc::Sender<crate::IncomingMessage>,
        pending: Arc<Mutex<HashMap<String, oneshot::Sender<OutgoingJsonRpcMessage>>>>,
    ) -> Self {
        Self {
            incoming_tx,
            pending,
        }
    }

    /// Execute a prompt through the MCP codex tool and return the result text.
    async fn execute_via_mcp(&self, task_id: &str, prompt: &str) -> Result<String, String> {
        use rmcp::model::*;

        let mcp_request_id = format!("a2a-{task_id}");

        let params = CallToolRequestParams {
            name: "codex".into(),
            arguments: Some(
                serde_json::from_value(json!({
                    "prompt": prompt
                }))
                .map_err(|e| format!("JSON error: {e}"))?,
            ),
            meta: None,
            task: None,
        };

        let mcp_call: crate::IncomingMessage = JsonRpcMessage::Request(JsonRpcRequest {
            jsonrpc: Default::default(),
            id: RequestId::String(mcp_request_id.clone().into()),
            request: ClientRequest::CallToolRequest(Request::new(params)),
        });

        // Register oneshot for the MCP response.
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .await
            .insert(mcp_request_id, tx);

        // Send to the shared MessageProcessor.
        self.incoming_tx
            .send(mcp_call)
            .await
            .map_err(|_| "Processor channel closed".to_string())?;

        // Wait for response (5 min timeout).
        let mcp_resp = tokio::time::timeout(std::time::Duration::from_secs(300), rx)
            .await
            .map_err(|_| "Task timed out".to_string())?
            .map_err(|_| "Response channel dropped".to_string())?;

        Ok(extract_mcp_result_text(&mcp_resp))
    }
}

impl AgentExecutor for CodexA2AExecutor {
    async fn execute(
        &self,
        context: RequestContext,
        event_bus: &EventBus,
    ) -> Result<(), A2AError> {
        // Extract text from message parts.
        let prompt = context
            .request
            .message
            .parts
            .iter()
            .filter_map(|p| p.text.as_deref())
            .collect::<Vec<_>>()
            .join("\n");

        if prompt.is_empty() {
            return Err(A2AError::invalid_params("No text content in message"));
        }

        let task_id = context.task_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        info!(task_id = %task_id, "A2A task started");

        match self.execute_via_mcp(&task_id, &prompt).await {
            Ok(result_text) => {
                let task = completed_task(&task_id, &context.context_id, &result_text);
                event_bus.publish(ExecutionEvent::Task(task));
            }
            Err(err_msg) => {
                let task = failed_task(&task_id, &context.context_id, &err_msg);
                event_bus.publish(ExecutionEvent::Task(task));
            }
        }

        Ok(())
    }

    async fn cancel(
        &self,
        task_id: &str,
        _event_bus: &EventBus,
    ) -> Result<(), A2AError> {
        Err(A2AError::task_not_cancelable(task_id))
    }

    fn agent_card(&self, base_url: &str) -> AgentCard {
        AgentCard {
            name: "codex".into(),
            description:
                "OpenAI Codex CLI — an AI coding agent that reads, writes, and executes code."
                    .into(),
            supported_interfaces: vec![AgentInterface {
                url: format!("{base_url}/"),
                protocol_binding: "HTTP+JSON".into(),
                tenant: None,
                protocol_version: "1.0".into(),
            }],
            provider: Some(AgentProvider {
                organization: "OpenAI".into(),
                url: "https://openai.com".into(),
            }),
            version: "0.1.0".into(),
            documentation_url: None,
            capabilities: AgentCapabilities {
                streaming: Some(false),
                push_notifications: Some(false),
                extended_agent_card: None,
            },
            default_input_modes: vec!["text".into()],
            default_output_modes: vec!["text".into()],
            skills: vec![AgentSkill {
                id: "codex".into(),
                name: "Codex Coding Agent".into(),
                description:
                    "Execute coding tasks: read files, write code, run commands, debug issues."
                        .into(),
                tags: vec!["coding".into(), "shell".into(), "files".into()],
                examples: vec![],
                input_modes: None,
                output_modes: None,
            }],
            icon_url: None,
        }
    }
}

// ================================================================
// Helper
// ================================================================

fn extract_mcp_result_text(msg: &OutgoingJsonRpcMessage) -> String {
    if let Ok(v) = serde_json::to_value(msg) {
        if let Some(content) = v.pointer("/result/content") {
            if let Some(arr) = content.as_array() {
                let texts: Vec<&str> = arr
                    .iter()
                    .filter_map(|c| c.get("text").and_then(|t| t.as_str()))
                    .collect();
                if !texts.is_empty() {
                    return texts.join("\n");
                }
            }
        }
        if let Some(result) = v.get("result") {
            return result.to_string();
        }
    }
    "No result".into()
}
