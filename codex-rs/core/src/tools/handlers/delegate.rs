use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::delegate_tool::DelegateEventReceiver;
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
use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
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

    ToolSpec::Function(ResponsesApiTool {
        name: "delegate_agent".to_string(),
        description: r#"Delegates work to a configured sub-agent.
Provide the agent id, a prompt, and optional context such as working directory overrides.
"#
        .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["agent_id".to_string(), "prompt".to_string()]),
            additional_properties: Some(false.into()),
        },
    })
});

#[derive(Debug, Deserialize)]
struct DelegateToolArgs {
    agent_id: String,
    prompt: String,
    #[serde(default)]
    context: Option<DelegateToolArgsContext>,
}

#[derive(Debug, Default, Deserialize)]
struct DelegateToolArgsContext {
    working_directory: Option<String>,
    #[serde(default)]
    hints: Vec<String>,
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
    agent_id: String,
    run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<u64>,
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

        let adapter = session.delegate_adapter().ok_or_else(|| {
            FunctionCallError::RespondToModel("delegate tool is not available".to_string())
        })?;

        let mut events = adapter.subscribe().await;

        let request = DelegateToolRequest {
            agent_id: args.agent_id.clone(),
            prompt: args.prompt.clone(),
            context: args.context.unwrap_or_default().into(),
        };

        let run = adapter.delegate(request).await.map_err(map_adapter_error)?;

        let (summary, duration) = wait_for_completion(&mut events, &run)
            .await
            .map_err(FunctionCallError::RespondToModel)?;

        let response = DelegateToolResponse {
            status: "ok",
            agent_id: run.agent_id,
            run_id: run.run_id,
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

fn map_adapter_error(err: DelegateToolError) -> FunctionCallError {
    match err {
        DelegateToolError::DelegateInProgress => FunctionCallError::RespondToModel(
            "another delegate is already running; wait for it to finish before delegating again"
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
