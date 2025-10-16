use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::codex::Session;
use crate::delegate_tool::DelegateEventReceiver;
use crate::delegate_tool::DelegateInvocationMode;
use crate::delegate_tool::DelegateToolAdapter;
use crate::delegate_tool::DelegateToolContext;
use crate::delegate_tool::DelegateToolError;
use crate::delegate_tool::DelegateToolEvent;
use crate::delegate_tool::DelegateToolRequest;
use crate::delegate_tool::DelegateToolRun;
use crate::function_tool::FunctionCallError;
use crate::openai_tools::JsonSchema;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::user_notification::UserNotification;
use async_trait::async_trait;
use codex_protocol::ConversationId;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::LazyLock;
use std::time::Duration;

pub struct DelegateToolHandler;

pub static DELEGATE_TOOL: LazyLock<ToolSpec> = LazyLock::new(|| {
    let mut context_props = BTreeMap::new();
    context_props.insert(
        "working_directory".to_string(),
        JsonSchema::String {
            description: Some("Override the delegate's working directory".to_string()),
        },
    );
    context_props.insert(
        "hints".to_string(),
        JsonSchema::Array {
            items: Box::new(JsonSchema::String {
                description: Some("Optional high-level hints to guide the delegate".to_string()),
            }),
            description: Some("Additional hints for the delegate".to_string()),
        },
    );

    let mut batch_entry_props = BTreeMap::new();
    batch_entry_props.insert(
        "agent_id".to_string(),
        JsonSchema::String {
            description: Some("Identifier of the sub-agent to invoke".to_string()),
        },
    );
    batch_entry_props.insert(
        "prompt".to_string(),
        JsonSchema::String {
            description: Some("Instructions passed to the sub-agent".to_string()),
        },
    );
    batch_entry_props.insert(
        "mode".to_string(),
        JsonSchema::String {
            description: Some(
                "Invocation mode. Use \"immediate\" for blocking delegation (default) or \
                 \"detached\" to run in the background."
                    .to_string(),
            ),
        },
    );
    batch_entry_props.insert(
        "context".to_string(),
        JsonSchema::Object {
            properties: context_props.clone(),
            required: None,
            additional_properties: Some(false.into()),
        },
    );

    let mut properties = BTreeMap::new();
    properties.insert(
        "agent_id".to_string(),
        JsonSchema::String {
            description: Some("Identifier of the sub-agent to invoke".to_string()),
        },
    );
    properties.insert(
        "prompt".to_string(),
        JsonSchema::String {
            description: Some("Instructions passed to the sub-agent".to_string()),
        },
    );
    properties.insert(
        "context".to_string(),
        JsonSchema::Object {
            properties: context_props,
            required: None,
            additional_properties: Some(false.into()),
        },
    );
    properties.insert(
        "mode".to_string(),
        JsonSchema::String {
            description: Some(
                "Invocation mode. Use \"immediate\" for blocking delegation (default) or \
                 \"detached\" to run in the background."
                    .to_string(),
            ),
        },
    );
    properties.insert(
        "batch".to_string(),
        JsonSchema::Array {
            items: Box::new(JsonSchema::Object {
                properties: batch_entry_props,
                required: Some(vec!["agent_id".to_string(), "prompt".to_string()]),
                additional_properties: Some(false.into()),
            }),
            description: Some(
                "Invoke multiple delegates in one call; each entry must supply agent_id and prompt"
                    .to_string(),
            ),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "delegate_agent".to_string(),
        description: r#"Delegates work to a configured sub-agent.
Provide the agent id, a prompt, and optional context such as working directory overrides.
"#
        .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: None,
            additional_properties: Some(false.into()),
        },
    })
});

#[derive(Debug, Deserialize)]
struct DelegateToolArgs {
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    context: Option<DelegateToolArgsContext>,
    #[serde(default)]
    mode: Option<DelegateInvocationMode>,
    #[serde(default)]
    batch: Vec<DelegateToolBatchArgs>,
}

#[derive(Debug, Default, Deserialize)]
struct DelegateToolArgsContext {
    working_directory: Option<String>,
    #[serde(default)]
    hints: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct DelegateToolBatchArgs {
    agent_id: String,
    prompt: String,
    #[serde(default)]
    context: Option<DelegateToolArgsContext>,
    #[serde(default)]
    mode: Option<DelegateInvocationMode>,
}

impl From<DelegateToolArgsContext> for DelegateToolContext {
    fn from(value: DelegateToolArgsContext) -> Self {
        Self {
            working_directory: value.working_directory,
            hints: value.hints,
        }
    }
}

#[derive(Debug, Serialize)]
struct DelegateToolResponse {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
struct DelegateToolBatchRun {
    agent_id: String,
    run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
struct DelegateToolBatchResponse {
    status: &'static str,
    runs: Vec<DelegateToolBatchRun>,
}

#[async_trait]
impl ToolHandler for DelegateToolHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session, payload, ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "delegate_agent handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: DelegateToolArgs = serde_json::from_str(&arguments).map_err(|e| {
            FunctionCallError::RespondToModel(format!("failed to parse function arguments: {e}"))
        })?;

        if !args.batch.is_empty() && (args.agent_id.is_some() || args.prompt.is_some()) {
            return Err(FunctionCallError::RespondToModel(
                "when `batch` is provided, omit top-level `agent_id` and `prompt` fields".into(),
            ));
        }

        if !args.batch.is_empty() && args.mode.is_some() {
            return Err(FunctionCallError::RespondToModel(
                "`mode` cannot be combined with `batch`; set the mode on each batch entry instead"
                    .into(),
            ));
        }

        let adapter = session.delegate_adapter().ok_or_else(|| {
            FunctionCallError::RespondToModel("delegate tool is not available".to_string())
        })?;

        let mut events = adapter.subscribe().await;
        let conversation_id = session.conversation_id();

        if !args.batch.is_empty() {
            let runs =
                handle_batch_entries(adapter.as_ref(), &mut events, &conversation_id, args.batch)
                    .await?;

            let response = DelegateToolBatchResponse { status: "ok", runs };
            let content = serde_json::to_string(&response).map_err(|e| {
                FunctionCallError::Fatal(format!("failed to serialize response: {e}"))
            })?;

            return Ok(ToolOutput::Function {
                content,
                success: Some(true),
            });
        }

        let agent_id = args.agent_id.ok_or_else(|| {
            FunctionCallError::RespondToModel("missing `agent_id` for delegate_agent call".into())
        })?;
        let prompt = args.prompt.ok_or_else(|| {
            FunctionCallError::RespondToModel("missing `prompt` for delegate_agent call".into())
        })?;

        let mode = args.mode.unwrap_or_default();

        let request = DelegateToolRequest {
            agent_id: agent_id.clone(),
            prompt: prompt.clone(),
            context: args.context.unwrap_or_default().into(),
            caller_conversation_id: Some(conversation_id.to_string()),
            mode,
            batch: Vec::new(),
        };

        let run = adapter.delegate(request).await.map_err(map_adapter_error)?;

        if mode == DelegateInvocationMode::Detached {
            let response = DelegateToolResponse {
                status: "accepted",
                agent_id: None,
                run_id: None,
                summary: None,
                duration_ms: None,
            };
            let content = serde_json::to_string(&response).map_err(|e| {
                FunctionCallError::Fatal(format!("failed to serialize response: {e}"))
            })?;

            let session_clone = Arc::clone(&session);
            let run_id = run.run_id.clone();
            let agent_id = run.agent_id.clone();
            tokio::spawn(async move {
                monitor_detached_run(events, session_clone, run_id, agent_id).await;
            });

            return Ok(ToolOutput::Function {
                content,
                success: Some(true),
            });
        }

        let (summary, duration) = wait_for_completion(&mut events, &run)
            .await
            .map_err(FunctionCallError::RespondToModel)?;

        let response = DelegateToolResponse {
            status: "ok",
            agent_id: Some(run.agent_id),
            run_id: Some(run.run_id),
            summary,
            duration_ms: duration.map(|d| d.as_millis() as u64),
        };

        let content = serde_json::to_string(&response)
            .map_err(|e| FunctionCallError::Fatal(format!("failed to serialize response: {e}")))?;

        Ok(ToolOutput::Function {
            content,
            success: Some(true),
        })
    }
}

async fn wait_for_completion(
    events: &mut DelegateEventReceiver,
    run: &DelegateToolRun,
) -> Result<(Option<String>, Option<Duration>), String> {
    let mut collected = String::new();

    while let Some(event) = events.recv().await {
        if event_run_id(&event) != run.run_id {
            continue;
        }

        match event {
            DelegateToolEvent::Delta { chunk, .. } => {
                collected.push_str(&chunk);
            }
            DelegateToolEvent::Completed {
                output, duration, ..
            } => {
                let summary = output.or_else(|| {
                    if collected.trim().is_empty() {
                        None
                    } else {
                        Some(collected.clone())
                    }
                });
                return Ok((summary, Some(duration)));
            }
            DelegateToolEvent::Failed { error, .. } => {
                return Err(error);
            }
            _ => {}
        }
    }

    Err("delegate run ended unexpectedly".to_string())
}

fn event_run_id(event: &DelegateToolEvent) -> &str {
    match event {
        DelegateToolEvent::Started { run_id, .. }
        | DelegateToolEvent::Delta { run_id, .. }
        | DelegateToolEvent::Completed { run_id, .. }
        | DelegateToolEvent::Failed { run_id, .. } => run_id,
    }
}

async fn handle_batch_entries(
    adapter: &dyn DelegateToolAdapter,
    events: &mut DelegateEventReceiver,
    conversation_id: &ConversationId,
    batch: Vec<DelegateToolBatchArgs>,
) -> Result<Vec<DelegateToolBatchRun>, FunctionCallError> {
    let mut runs = Vec::with_capacity(batch.len());
    let mut launched = Vec::with_capacity(batch.len());
    let conversation_id = conversation_id.to_string();

    for entry in batch {
        let mode = entry.mode.unwrap_or_default();
        if mode == DelegateInvocationMode::Detached {
            return Err(FunctionCallError::RespondToModel(
                "detached delegation is not supported within `batch` requests".to_string(),
            ));
        }

        let request = DelegateToolRequest {
            agent_id: entry.agent_id.clone(),
            prompt: entry.prompt.clone(),
            context: entry.context.unwrap_or_default().into(),
            caller_conversation_id: Some(conversation_id.clone()),
            mode,
            batch: Vec::new(),
        };

        let run = adapter.delegate(request).await.map_err(map_adapter_error)?;
        launched.push(run);
    }

    let mut interested: HashSet<String> = launched.iter().map(|run| run.run_id.clone()).collect();
    let mut collected: HashMap<String, String> = HashMap::new();
    let mut summaries: HashMap<String, (Option<String>, Option<Duration>)> = HashMap::new();

    while !interested.is_empty() {
        let event = events.recv().await.ok_or_else(|| {
            FunctionCallError::RespondToModel("delegate run ended unexpectedly".to_string())
        })?;

        let run_id = event_run_id(&event).to_string();
        if !interested.contains(&run_id) {
            continue;
        }

        match event {
            DelegateToolEvent::Delta { chunk, .. } => {
                collected.entry(run_id).or_default().push_str(&chunk);
            }
            DelegateToolEvent::Completed {
                output, duration, ..
            } => {
                let summary = output.or_else(|| {
                    collected.remove(&run_id).and_then(|text| {
                        if text.trim().is_empty() {
                            None
                        } else {
                            Some(text)
                        }
                    })
                });
                summaries.insert(run_id.clone(), (summary, Some(duration)));
                interested.remove(&run_id);
            }
            DelegateToolEvent::Failed { error, .. } => {
                return Err(FunctionCallError::RespondToModel(error));
            }
            _ => {}
        }
    }

    for run in launched {
        let (summary, duration) = summaries.remove(&run.run_id).unwrap_or((None, None));
        runs.push(DelegateToolBatchRun {
            agent_id: run.agent_id,
            run_id: run.run_id,
            summary,
            duration_ms: duration.map(|d| d.as_millis() as u64),
        });
    }

    Ok(runs)
}

fn map_adapter_error(err: DelegateToolError) -> FunctionCallError {
    match err {
        DelegateToolError::DelegateInProgress => FunctionCallError::RespondToModel(
            "another delegate is already running; wait for it to finish before delegating again"
                .to_string(),
        ),
        DelegateToolError::QueueFull => FunctionCallError::RespondToModel(
            "delegate queue is full; wait for background delegates to finish before delegating again"
                .to_string(),
        ),
        DelegateToolError::AgentNotFound(agent_id) => FunctionCallError::RespondToModel(format!(
            "delegate agent `{agent_id}` is not configured"
        )),
        DelegateToolError::SetupFailed(reason) => {
            FunctionCallError::RespondToModel(format!("failed to start delegate: {reason}"))
        }
    }
}

async fn monitor_detached_run(
    mut events: DelegateEventReceiver,
    session: Arc<Session>,
    run_id: String,
    agent_id: String,
) {
    let mut collected = String::new();

    while let Some(event) = events.recv().await {
        if event_run_id(&event) != run_id.as_str() {
            continue;
        }

        match event {
            DelegateToolEvent::Delta { chunk, .. } => {
                collected.push_str(&chunk);
            }
            DelegateToolEvent::Completed {
                output, duration, ..
            } => {
                let summary = output.or_else(|| {
                    if collected.trim().is_empty() {
                        None
                    } else {
                        Some(collected.clone())
                    }
                });
                session
                    .notifier()
                    .notify(&UserNotification::DetachedRunFinished {
                        agent_id: agent_id.clone(),
                        run_id: run_id.clone(),
                        conversation_id: Some(session.conversation_id().to_string()),
                        summary,
                        duration_ms: Some(duration.as_millis() as u64),
                        error: None,
                    });
                break;
            }
            DelegateToolEvent::Failed { error, .. } => {
                let summary = if collected.trim().is_empty() {
                    None
                } else {
                    Some(collected.clone())
                };
                session
                    .notifier()
                    .notify(&UserNotification::DetachedRunFinished {
                        agent_id: agent_id.clone(),
                        run_id: run_id.clone(),
                        conversation_id: Some(session.conversation_id().to_string()),
                        summary,
                        duration_ms: None,
                        error: Some(error),
                    });
                break;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::sync::mpsc;

    struct MockDelegateAdapter {
        sender: Mutex<Option<mpsc::UnboundedSender<DelegateToolEvent>>>,
        requests: Mutex<Vec<DelegateToolRequest>>,
    }

    impl MockDelegateAdapter {
        fn new() -> Self {
            Self {
                sender: Mutex::new(None),
                requests: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl DelegateToolAdapter for MockDelegateAdapter {
        async fn subscribe(&self) -> DelegateEventReceiver {
            let (tx, rx) = mpsc::unbounded_channel();
            *self.sender.lock().await = Some(tx);
            rx
        }

        async fn delegate(
            &self,
            request: DelegateToolRequest,
        ) -> Result<DelegateToolRun, DelegateToolError> {
            self.requests.lock().await.push(request.clone());
            let run_id = format!("run-{}", request.agent_id);
            if let Some(sender) = self.sender.lock().await.as_ref() {
                let _ = sender.send(DelegateToolEvent::Completed {
                    run_id: run_id.clone(),
                    agent_id: request.agent_id.clone(),
                    output: Some(format!("summary: {}", request.prompt)),
                    duration: Duration::from_millis(5),
                });
            }
            Ok(DelegateToolRun {
                run_id,
                agent_id: request.agent_id,
            })
        }
    }

    #[tokio::test]
    async fn handle_batch_executes_in_order() {
        let adapter = Arc::new(MockDelegateAdapter::new());
        let mut events = adapter.subscribe().await;

        let batch = vec![
            DelegateToolBatchArgs {
                agent_id: "alpha".into(),
                prompt: "one".into(),
                context: None,
                mode: None,
            },
            DelegateToolBatchArgs {
                agent_id: "bravo".into(),
                prompt: "two".into(),
                context: None,
                mode: None,
            },
        ];

        let runs =
            handle_batch_entries(adapter.as_ref(), &mut events, &ConversationId::new(), batch)
                .await
                .expect("batch runs");

        let requests = adapter.requests.lock().await.clone();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].agent_id, "alpha");
        assert_eq!(requests[1].agent_id, "bravo");

        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].agent_id, "alpha");
        assert_eq!(runs[1].agent_id, "bravo");
        assert!(runs.iter().all(|run| run.summary.is_some()));
    }
}
