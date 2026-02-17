//! Codex A2A handler — implements [`a2a_rs::AgentExecutor`] to bridge
//! A2A messages into the Codex MCP [`MessageProcessor`] via `tools/call`.
//!
//! Supports **bidirectional streaming**: intermediate `codex/event` MCP
//! notifications are forwarded as A2A `StatusUpdate` and `ArtifactUpdate`
//! events via the [`EventBus`], so SSE clients receive real-time progress.
//!
//! Features:
//! - **ArtifactUpdate streaming** — file patches emitted as artifacts
//! - **Task cancel** — cancels in-flight MCP calls via `CancellationToken`
//! - **Multi-turn context** — subsequent messages reuse `codex-reply` tool

use std::collections::HashMap;
use std::sync::Arc;

use a2a_rs::{
    A2AError, AgentCapabilities, AgentCard, AgentExecutor, AgentInterface,
    AgentProvider, AgentSkill, EventBus, ExecutionEvent, RequestContext,
    completed_task, failed_task,
};
use a2a_rs::types::{
    Artifact, Message, Part, Role, TaskArtifactUpdateEvent,
    TaskState, TaskStatus, TaskStatusUpdateEvent,
};
use serde_json::json;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::outgoing_message::OutgoingJsonRpcMessage;

/// Codex A2A executor — implements [`AgentExecutor`] from `a2a-rs`.
pub struct CodexA2AExecutor {
    /// Sender to feed JSON‐RPC messages into the shared MessageProcessor.
    pub incoming_tx: mpsc::Sender<crate::IncomingMessage>,
    /// Pending MCP request IDs → oneshot senders for responses.
    pub pending:
        Arc<Mutex<HashMap<String, oneshot::Sender<OutgoingJsonRpcMessage>>>>,
    /// Broadcast sender for outgoing MCP notifications — used to subscribe
    /// and forward intermediate `codex/event` notifications as A2A events.
    pub notification_tx: tokio::sync::broadcast::Sender<String>,
    /// Per-task cancellation tokens for aborting in-flight MCP calls.
    cancel_tokens: Arc<Mutex<HashMap<String, CancellationToken>>>,
    /// Maps context_id → thread_id for multi-turn conversations.
    context_threads: Arc<Mutex<HashMap<String, String>>>,
}

impl CodexA2AExecutor {
    pub fn new(
        incoming_tx: mpsc::Sender<crate::IncomingMessage>,
        pending: Arc<Mutex<HashMap<String, oneshot::Sender<OutgoingJsonRpcMessage>>>>,
        notification_tx: tokio::sync::broadcast::Sender<String>,
    ) -> Self {
        Self {
            incoming_tx,
            pending,
            notification_tx,
            cancel_tokens: Arc::new(Mutex::new(HashMap::new())),
            context_threads: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Execute a prompt through the MCP codex tool and return the result text.
    /// Supports cancellation via `CancellationToken`.
    async fn execute_via_mcp(
        &self,
        task_id: &str,
        prompt: &str,
        cancel_token: &CancellationToken,
    ) -> Result<String, String> {
        use rmcp::model::*;

        // Check if this context has an existing thread (multi-turn).
        let existing_thread = self.context_threads.lock().await
            .get(task_id)
            .cloned();

        let mcp_request_id = format!("a2a-{task_id}");

        let mcp_call: crate::IncomingMessage = if let Some(thread_id) = &existing_thread {
            // Multi-turn: use codex-reply with existing thread.
            let params = CallToolRequestParams {
                name: "codex-reply".into(),
                arguments: Some(
                    serde_json::from_value(json!({
                        "prompt": prompt,
                        "thread_id": thread_id
                    }))
                    .map_err(|e| format!("JSON error: {e}"))?,
                ),
                meta: None,
                task: None,
            };
            JsonRpcMessage::Request(JsonRpcRequest {
                jsonrpc: Default::default(),
                id: RequestId::String(mcp_request_id.clone().into()),
                request: ClientRequest::CallToolRequest(Request::new(params)),
            })
        } else {
            // First message: use codex tool.
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
            JsonRpcMessage::Request(JsonRpcRequest {
                jsonrpc: Default::default(),
                id: RequestId::String(mcp_request_id.clone().into()),
                request: ClientRequest::CallToolRequest(Request::new(params)),
            })
        };

        // Register oneshot for the MCP response.
        // The key must match what `extract_outgoing_id` returns: the JSON
        // serialization of the `id` field (e.g. `"a2a-task-1"` with quotes).
        let pending_key = serde_json::to_value(rmcp::model::RequestId::String(
            mcp_request_id.into(),
        ))
        .map(|v| v.to_string())
        .unwrap_or_default();

        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .await
            .insert(pending_key, tx);

        // Send to the shared MessageProcessor.
        self.incoming_tx
            .send(mcp_call)
            .await
            .map_err(|_| "Processor channel closed".to_string())?;

        // Wait for response with cancellation support (5 min timeout).
        let mcp_resp = tokio::select! {
            result = tokio::time::timeout(std::time::Duration::from_secs(300), rx) => {
                result
                    .map_err(|_| "Task timed out".to_string())?
                    .map_err(|_| "Response channel dropped".to_string())?
            }
            _ = cancel_token.cancelled() => {
                return Err("Task canceled".to_string());
            }
        };

        // Extract and save thread_id for multi-turn.
        let result_text = extract_mcp_result_text(&mcp_resp);
        if existing_thread.is_none() {
            if let Some(thread_id) = extract_thread_id(&mcp_resp) {
                self.context_threads
                    .lock()
                    .await
                    .insert(task_id.to_string(), thread_id);
            }
        }

        Ok(result_text)
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
        let context_id = context.context_id.clone();
        info!(task_id = %task_id, "A2A task started");

        // Register cancellation token.
        let cancel_token = CancellationToken::new();
        self.cancel_tokens
            .lock()
            .await
            .insert(task_id.clone(), cancel_token.clone());

        // Publish initial "working" status.
        event_bus.publish_status_update(TaskStatusUpdateEvent {
            task_id: task_id.clone(),
            context_id: context_id.clone(),
            status: TaskStatus {
                state: TaskState::Working,
                message: Some(Message {
                    message_id: uuid::Uuid::new_v4().to_string(),
                    context_id: Some(context_id.clone()),
                    task_id: Some(task_id.clone()),
                    role: Role::Agent,
                    parts: vec![Part::text("Processing request...")],
                    metadata: None,
                    extensions: vec![],
                    reference_task_ids: None,
                }),
                timestamp: Some(a2a_rs::types::now_iso8601()),
            },
            metadata: None,
        });

        // Subscribe to MCP notification broadcast and forward codex/event
        // notifications as A2A StatusUpdate + ArtifactUpdate events.
        let mut notif_rx = self.notification_tx.subscribe();
        let forwarder_task_id = task_id.clone();
        let forwarder_context_id = context_id.clone();
        let forwarder_bus = event_bus.clone_sender();
        let forwarder_handle = tokio::spawn(async move {
            while let Ok(json_str) = notif_rx.recv().await {
                // Status updates (thinking, running commands, etc.)
                if let Some(update) = parse_codex_notification_to_status(
                    &json_str,
                    &forwarder_task_id,
                    &forwarder_context_id,
                ) {
                    forwarder_bus.publish(ExecutionEvent::StatusUpdate(update));
                }
                // Artifact updates (file patches)
                if let Some(artifact_event) = parse_codex_notification_to_artifact(
                    &json_str,
                    &forwarder_task_id,
                    &forwarder_context_id,
                ) {
                    forwarder_bus.publish(ExecutionEvent::ArtifactUpdate(artifact_event));
                }
            }
        });

        // Execute the actual MCP call with cancellation support.
        match self.execute_via_mcp(&task_id, &prompt, &cancel_token).await {
            Ok(result_text) => {
                let task = completed_task(&task_id, &context_id, &result_text);
                event_bus.publish(ExecutionEvent::Task(task));
            }
            Err(err_msg) if err_msg == "Task canceled" => {
                // Build a canceled task.
                let task = a2a_rs::types::Task {
                    id: task_id.clone(),
                    context_id: context_id.clone(),
                    status: TaskStatus {
                        state: TaskState::Canceled,
                        message: Some(Message {
                            message_id: uuid::Uuid::new_v4().to_string(),
                            context_id: Some(context_id.clone()),
                            task_id: Some(task_id.clone()),
                            role: Role::Agent,
                            parts: vec![Part::text("Task canceled by user request.")],
                            metadata: None,
                            extensions: vec![],
                            reference_task_ids: None,
                        }),
                        timestamp: Some(a2a_rs::types::now_iso8601()),
                    },
                    artifacts: vec![],
                    history: vec![],
                    metadata: None,
                };
                event_bus.publish(ExecutionEvent::Task(task));
            }
            Err(err_msg) => {
                let task = failed_task(&task_id, &context_id, &err_msg);
                event_bus.publish(ExecutionEvent::Task(task));
            }
        }

        // Cleanup: stop forwarder and remove cancel token.
        forwarder_handle.abort();
        self.cancel_tokens.lock().await.remove(&task_id);

        Ok(())
    }

    async fn cancel(
        &self,
        task_id: &str,
        event_bus: &EventBus,
    ) -> Result<(), A2AError> {
        let token = self.cancel_tokens.lock().await.get(task_id).cloned();
        if let Some(token) = token {
            info!(task_id = %task_id, "Canceling A2A task");
            token.cancel();
            Ok(())
        } else {
            // Task not found or already finished — publish failed status.
            event_bus.publish_status_update(TaskStatusUpdateEvent {
                task_id: task_id.to_string(),
                context_id: String::new(),
                status: TaskStatus {
                    state: TaskState::Failed,
                    message: Some(Message {
                        message_id: uuid::Uuid::new_v4().to_string(),
                        context_id: None,
                        task_id: Some(task_id.to_string()),
                        role: Role::Agent,
                        parts: vec![Part::text(format!("Task {task_id} not found or already finished."))],
                        metadata: None,
                        extensions: vec![],
                        reference_task_ids: None,
                    }),
                    timestamp: Some(a2a_rs::types::now_iso8601()),
                },
                metadata: None,
            });
            Err(A2AError::task_not_cancelable(task_id))
        }
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
                streaming: Some(true),
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
// Helpers
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

/// Extract `threadId` from the MCP response's `structuredContent` for multi-turn.
fn extract_thread_id(msg: &OutgoingJsonRpcMessage) -> Option<String> {
    let v = serde_json::to_value(msg).ok()?;
    v.pointer("/result/structuredContent/threadId")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
}

/// Parse a serialized MCP outgoing message and convert relevant `codex/event`
/// notifications into A2A [`TaskStatusUpdateEvent`]s for streaming.
fn parse_codex_notification_to_status(
    json_str: &str,
    task_id: &str,
    context_id: &str,
) -> Option<TaskStatusUpdateEvent> {
    let v: serde_json::Value = serde_json::from_str(json_str).ok()?;

    // Only handle codex/event notifications.
    let method = v.get("method")?.as_str()?;
    if method != "codex/event" {
        return None;
    }

    let params = v.get("params")?;
    let msg = params.get("msg")?;
    let event_type = msg.get("type")?.as_str()?;

    let status_text = match event_type {
        "agent_message_start" => "Thinking...".to_string(),
        "agent_message_delta" => {
            // Extract the delta text if available.
            msg.get("delta")
                .and_then(|d| d.as_str())
                .unwrap_or("Generating response...")
                .to_string()
        }
        "exec_command_start" | "exec_command_running" => {
            let cmd = msg
                .get("command")
                .and_then(|c| c.as_str())
                .or_else(|| msg.get("call_id").and_then(|c| c.as_str()))
                .unwrap_or("unknown");
            format!("Running command: {cmd}")
        }
        "exec_command_output" => {
            msg.get("output")
                .and_then(|o| o.as_str())
                .map(|s| {
                    let truncated: String = s.chars().take(200).collect();
                    format!("Command output: {truncated}")
                })
                .unwrap_or_else(|| "Command output received".to_string())
        }
        "patch_start" | "patch_apply" => {
            let file = msg
                .get("path")
                .and_then(|p| p.as_str())
                .unwrap_or("file");
            format!("Writing: {file}")
        }
        "session_configured" | "task_started" => "Initializing...".to_string(),
        // Skip events that don't convey meaningful progress.
        "task_complete" | "exec_command_complete" | "background_event" => return None,
        _ => return None,
    };

    Some(TaskStatusUpdateEvent {
        task_id: task_id.to_string(),
        context_id: context_id.to_string(),
        status: TaskStatus {
            state: TaskState::Working,
            message: Some(Message {
                message_id: uuid::Uuid::new_v4().to_string(),
                context_id: Some(context_id.to_string()),
                task_id: Some(task_id.to_string()),
                role: Role::Agent,
                parts: vec![Part::text(status_text)],
                metadata: None,
                extensions: vec![],
                reference_task_ids: None,
            }),
            timestamp: Some(a2a_rs::types::now_iso8601()),
        },
        metadata: None,
    })
}

/// Parse `patch_apply` notifications into A2A [`TaskArtifactUpdateEvent`]s.
/// Each file patch becomes an artifact with the file path and diff content.
fn parse_codex_notification_to_artifact(
    json_str: &str,
    task_id: &str,
    context_id: &str,
) -> Option<TaskArtifactUpdateEvent> {
    let v: serde_json::Value = serde_json::from_str(json_str).ok()?;

    let method = v.get("method")?.as_str()?;
    if method != "codex/event" {
        return None;
    }

    let params = v.get("params")?;
    let msg = params.get("msg")?;
    let event_type = msg.get("type")?.as_str()?;

    // Only emit artifacts for patch_apply (actual file changes).
    if event_type != "patch_apply" {
        return None;
    }

    let path = msg.get("path").and_then(|p| p.as_str()).unwrap_or("unknown");
    let content = msg.get("content")
        .or_else(|| msg.get("diff"))
        .or_else(|| msg.get("patch"))
        .and_then(|c| c.as_str())
        .unwrap_or("");

    Some(TaskArtifactUpdateEvent {
        task_id: task_id.to_string(),
        context_id: context_id.to_string(),
        artifact: Artifact {
            artifact_id: format!("patch-{}", uuid::Uuid::new_v4()),
            name: Some(path.to_string()),
            description: Some(format!("File change: {path}")),
            parts: vec![Part::text(format!("--- {path}\n{content}"))],
            metadata: None,
            extensions: vec![],
        },
        append: false,
        last_chunk: true,
        metadata: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2a_rs::{EventBus, ExecutionEvent, RequestContext};
    use a2a_rs::types::{SendMessageRequest, TaskState};
    use crate::outgoing_message::{OutgoingMessage, OutgoingResponse};
    use rmcp::model::{JsonRpcMessage, JsonRpcResponse, RequestId};
    use tokio::sync::mpsc;

    /// Integration test: verifies shared PendingMap routing.
    ///
    /// Flow:
    /// 1. CodexA2AExecutor sends MCP tools/call via `incoming_tx`
    /// 2. Fake processor reads it, creates response, sends via `outgoing_tx`
    /// 3. Router task reads `outgoing_rx`, finds match in shared_pending, routes to oneshot
    /// 4. Executor's `execute_via_mcp` completes → publishes completed Task event
    #[tokio::test]
    async fn e2e_shared_pending_routes_mcp_response_to_executor() {
        // Shared pending map (the fix).
        let shared_pending: crate::PendingMap =
            Arc::new(Mutex::new(HashMap::new()));

        // Channels.
        let (incoming_tx, mut incoming_rx) = mpsc::channel::<crate::IncomingMessage>(16);
        let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel::<OutgoingMessage>();
        let (notif_tx, _) = tokio::sync::broadcast::channel::<String>(16);

        // Router task — simulates the stdout writer in lib.rs.
        let router_pending = shared_pending.clone();
        let router_notif_tx = notif_tx.clone();
        let router_handle = tokio::spawn(async move {
            while let Some(outgoing_message) = outgoing_rx.recv().await {
                let msg: crate::outgoing_message::OutgoingJsonRpcMessage = outgoing_message.into();
                // Route via shared pending map.
                let id_str = crate::extract_outgoing_id(&msg);
                if let Some(id) = id_str {
                    let mut map = router_pending.lock().await;
                    if let Some(tx) = map.remove(&id) {
                        let _ = tx.send(msg.clone());
                    }
                }
                // Forward to notification broadcast.
                if let Ok(json) = serde_json::to_string(&msg) {
                    let _ = router_notif_tx.send(json);
                }
            }
        });

        // Fake MCP processor — reads incoming request, sends back a response.
        let fake_processor = tokio::spawn(async move {
            while let Some(msg) = incoming_rx.recv().await {
                if let JsonRpcMessage::Request(req) = msg {
                    let response_id = req.id.clone();
                    let result_value = serde_json::json!({
                        "content": [{"type": "text", "text": "Hello from test!"}],
                        "isError": false
                    });
                    let _ = outgoing_tx.send(OutgoingMessage::Response(
                        OutgoingResponse {
                            id: response_id,
                            result: result_value,
                        },
                    ));
                }
            }
        });

        // Create executor with shared pending.
        let executor = CodexA2AExecutor::new(
            incoming_tx.clone(),
            shared_pending.clone(),
            notif_tx.clone(),
        );

        // Create event bus and subscribe.
        let event_bus = EventBus::new(16);
        let mut event_rx = event_bus.subscribe();

        // Build request context.
        let context = RequestContext {
            task_id: Some("test-task-1".to_string()),
            context_id: "test-ctx-1".to_string(),
            request: SendMessageRequest {
                message: Message {
                    message_id: "m1".to_string(),
                    context_id: Some("test-ctx-1".to_string()),
                    task_id: None,
                    role: a2a_rs::types::Role::User,
                    parts: vec![Part::text("say hi")],
                    metadata: None,
                    extensions: vec![],
                    reference_task_ids: None,
                },
                configuration: None,
                metadata: None,
            },
        };

        // Execute — should complete without timeout.
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            executor.execute(context, &event_bus),
        )
        .await;

        assert!(result.is_ok(), "execute should not timeout");
        assert!(result.unwrap().is_ok(), "execute should succeed");

        // Collect events.
        let mut got_status = false;
        let mut got_task = false;
        let mut task_state = None;
        let mut result_text = String::new();

        while let Ok(event) = event_rx.try_recv() {
            match event {
                ExecutionEvent::StatusUpdate(s) => {
                    assert_eq!(s.task_id, "test-task-1");
                    got_status = true;
                }
                ExecutionEvent::Task(t) => {
                    task_state = Some(t.status.state.clone());
                    for artifact in &t.artifacts {
                        for part in &artifact.parts {
                            if let Some(txt) = &part.text {
                                result_text = txt.to_string();
                            }
                        }
                    }
                    got_task = true;
                }
                _ => {}
            }
        }

        assert!(got_status, "should have received at least one status update");
        assert!(got_task, "should have received task event");
        assert_eq!(task_state, Some(TaskState::Completed));
        assert!(
            result_text.contains("Hello from test!"),
            "task result should contain 'Hello from test!' but got: {result_text}"
        );

        // Cleanup.
        router_handle.abort();
        fake_processor.abort();
    }

    /// Verify cancel aborts an in-flight MCP call.
    #[tokio::test]
    async fn cancel_aborts_pending_mcp_call() {
        let shared_pending: crate::PendingMap =
            Arc::new(Mutex::new(HashMap::new()));
        let (incoming_tx, mut incoming_rx) = mpsc::channel::<crate::IncomingMessage>(16);
        let (notif_tx, _) = tokio::sync::broadcast::channel::<String>(16);

        // Black-hole processor — never responds.
        let _blackhole = tokio::spawn(async move {
            while let Some(_) = incoming_rx.recv().await {
                // Intentionally don't respond.
            }
        });

        let executor = CodexA2AExecutor::new(
            incoming_tx.clone(),
            shared_pending.clone(),
            notif_tx.clone(),
        );

        let event_bus = EventBus::new(16);
        let mut event_rx = event_bus.subscribe();

        let context = RequestContext {
            task_id: Some("cancel-test-1".to_string()),
            context_id: "cancel-ctx-1".to_string(),
            request: SendMessageRequest {
                message: Message {
                    message_id: "m1".to_string(),
                    context_id: Some("cancel-ctx-1".to_string()),
                    task_id: None,
                    role: a2a_rs::types::Role::User,
                    parts: vec![Part::text("do something slow")],
                    metadata: None,
                    extensions: vec![],
                    reference_task_ids: None,
                },
                configuration: None,
                metadata: None,
            },
        };

        // Spawn execute in background.
        let exec_bus = event_bus.clone_sender();
        let exec_handle = tokio::spawn({
            let executor_ref = &executor;
            // We can't borrow across spawn easily, so test cancel token directly.
            async move {}
        });

        // Instead, test the cancel mechanism directly via CancellationToken.
        let token = CancellationToken::new();
        let token_clone = token.clone();

        let pending_clone = shared_pending.clone();
        let incoming_tx_clone = incoming_tx.clone();
        let exec_task = tokio::spawn(async move {
            let executor = CodexA2AExecutor::new(
                incoming_tx_clone,
                pending_clone,
                notif_tx.clone(),
            );
            executor.execute_via_mcp("cancel-test-1", "do something", &token_clone).await
        });

        // Give the executor time to send the request.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Cancel it.
        token.cancel();

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            exec_task,
        ).await;

        assert!(result.is_ok(), "should complete quickly after cancel");
        let inner = result.unwrap().unwrap();
        assert!(inner.is_err(), "should return error");
        assert_eq!(inner.unwrap_err(), "Task canceled");

        exec_handle.abort();
    }

    /// Verify that `extract_outgoing_id` correctly extracts the request ID.
    #[test]
    fn extract_id_from_response() {
        let msg: crate::outgoing_message::OutgoingJsonRpcMessage =
            JsonRpcMessage::Response(JsonRpcResponse {
                jsonrpc: Default::default(),
                id: RequestId::String("a2a-task-42".into()),
                result: serde_json::Value::Null.into(),
            });
        let id = crate::extract_outgoing_id(&msg);
        assert_eq!(id, Some("\"a2a-task-42\"".to_string()));
    }

    /// Verify that notification parsing works for codex/event.
    #[test]
    fn parse_codex_event_notification() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "codex/event",
            "params": {
                "msg": {
                    "type": "agent_message_delta",
                    "delta": "thinking hard..."
                }
            }
        })
        .to_string();

        let result = parse_codex_notification_to_status(&json, "t1", "c1");
        assert!(result.is_some());
        let update = result.unwrap();
        assert_eq!(update.task_id, "t1");
        assert_eq!(update.status.state, TaskState::Working);
        let msg = update.status.message.unwrap();
        assert!(msg.parts[0].text.as_deref().unwrap().contains("thinking hard"));
    }

    /// Verify that non-codex notifications are ignored.
    #[test]
    fn parse_non_codex_notification_returns_none() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "other/event",
            "params": {}
        })
        .to_string();
        assert!(parse_codex_notification_to_status(&json, "t1", "c1").is_none());
    }

    /// Verify that patch_apply produces an ArtifactUpdate event.
    #[test]
    fn patch_apply_produces_artifact_update() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "codex/event",
            "params": {
                "msg": {
                    "type": "patch_apply",
                    "path": "/src/main.rs",
                    "content": "+fn new_function() {}"
                }
            }
        })
        .to_string();

        let result = parse_codex_notification_to_artifact(&json, "t1", "c1");
        assert!(result.is_some());
        let event = result.unwrap();
        assert_eq!(event.task_id, "t1");
        assert_eq!(event.artifact.name.as_deref(), Some("/src/main.rs"));
        assert!(event.artifact.parts[0].text.as_deref().unwrap().contains("+fn new_function"));
        assert!(event.last_chunk);
    }

    /// Verify that non-patch events don't produce artifacts.
    #[test]
    fn non_patch_does_not_produce_artifact() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "codex/event",
            "params": {
                "msg": {
                    "type": "agent_message_delta",
                    "delta": "hello"
                }
            }
        })
        .to_string();
        assert!(parse_codex_notification_to_artifact(&json, "t1", "c1").is_none());
    }
}
