use crate::codex::TurnContext;
use crate::config::Config;
use crate::error::CodexErr;
use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use codex_protocol::ThreadId;
use serde::Deserialize;

pub struct CollabHandler;

pub(crate) const DEFAULT_WAIT_TIMEOUT_MS: i64 = 30_000;
pub(crate) const MAX_WAIT_TIMEOUT_MS: i64 = 300_000;

#[derive(Debug, Deserialize)]
struct SpawnAgentArgs {
    message: String,
}

#[derive(Debug, Deserialize)]
struct SendInputArgs {
    id: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct WaitArgs {
    id: String,
    timeout_ms: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CloseAgentArgs {
    id: String,
}

#[async_trait]
impl ToolHandler for CollabHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            tool_name,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "collab handler received unsupported payload".to_string(),
                ));
            }
        };

        match tool_name.as_str() {
            "spawn_agent" => handle_spawn_agent(session, turn, arguments).await,
            "send_input" => handle_send_input(session, arguments).await,
            "wait" => handle_wait(arguments).await,
            "close_agent" => handle_close_agent(arguments).await,
            other => Err(FunctionCallError::RespondToModel(format!(
                "unsupported collab tool {other}"
            ))),
        }
    }
}

async fn handle_spawn_agent(
    session: std::sync::Arc<crate::codex::Session>,
    turn: std::sync::Arc<TurnContext>,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    let args: SpawnAgentArgs = parse_arguments(&arguments)?;
    if args.message.trim().is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "Empty message can't be send to an agent".to_string(),
        ));
    }
    let config = build_agent_spawn_config(turn.as_ref())?;
    let result = session
        .services
        .agent_control
        .spawn_agent(config, args.message, true)
        .await
        .map_err(|err| FunctionCallError::Fatal(err.to_string()))?;

    Ok(ToolOutput::Function {
        content: format!("agent_id: {result}"),
        success: Some(true),
        content_items: None,
    })
}

async fn handle_send_input(
    session: std::sync::Arc<crate::codex::Session>,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    let args: SendInputArgs = parse_arguments(&arguments)?;
    let agent_id = agent_id(&args.id)?;
    if args.message.trim().is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "Empty message can't be send to an agent".to_string(),
        ));
    }
    let content = session
        .services
        .agent_control
        .send_prompt(agent_id, args.message)
        .await
        .map_err(|err| match err {
            CodexErr::ThreadNotFound(id) => {
                FunctionCallError::RespondToModel(format!("agent with id {id} not found"))
            }
            err => FunctionCallError::Fatal(err.to_string()),
        })?;

    Ok(ToolOutput::Function {
        content,
        success: Some(true),
        content_items: None,
    })
}

async fn handle_wait(arguments: String) -> Result<ToolOutput, FunctionCallError> {
    let args: WaitArgs = parse_arguments(&arguments)?;
    let _agent_id = agent_id(&args.id)?;

    let timeout_ms = args.timeout_ms.unwrap_or(DEFAULT_WAIT_TIMEOUT_MS);
    if timeout_ms <= 0 {
        return Err(FunctionCallError::RespondToModel(
            "timeout_ms must be greater than zero".to_string(),
        ));
    }
    let _timeout_ms = timeout_ms.min(MAX_WAIT_TIMEOUT_MS);
    // TODO(jif): implement agent wait once lifecycle tracking is wired up.
    Err(FunctionCallError::Fatal("wait not implemented".to_string()))
}

async fn handle_close_agent(arguments: String) -> Result<ToolOutput, FunctionCallError> {
    let args: CloseAgentArgs = parse_arguments(&arguments)?;
    let _agent_id = agent_id(&args.id)?;
    // TODO(jif): implement agent shutdown and return the final status.
    Err(FunctionCallError::Fatal(
        "close_agent not implemented".to_string(),
    ))
}

fn agent_id(id: &str) -> Result<ThreadId, FunctionCallError> {
    ThreadId::from_string(id)
        .map_err(|e| FunctionCallError::RespondToModel(format!("invalid agent id {id}: {e:?}")))
}

fn build_agent_spawn_config(turn: &TurnContext) -> Result<Config, FunctionCallError> {
    let base_config = turn.client.config();
    let mut config = (*base_config).clone();
    config.model = Some(turn.client.get_model());
    config.model_provider = turn.client.get_provider();
    config.model_reasoning_effort = turn.client.get_reasoning_effort();
    config.model_reasoning_summary = turn.client.get_reasoning_summary();
    config.developer_instructions = turn.developer_instructions.clone();
    config.base_instructions = turn.base_instructions.clone();
    config.compact_prompt = turn.compact_prompt.clone();
    config.user_instructions = turn.user_instructions.clone();
    config.shell_environment_policy = turn.shell_environment_policy.clone();
    config.codex_linux_sandbox_exe = turn.codex_linux_sandbox_exe.clone();
    config.cwd = turn.cwd.clone();
    config
        .approval_policy
        .set(turn.approval_policy)
        .map_err(|err| {
            FunctionCallError::RespondToModel(format!("approval_policy is invalid: {err}"))
        })?;
    config
        .sandbox_policy
        .set(turn.sandbox_policy.clone())
        .map_err(|err| {
            FunctionCallError::RespondToModel(format!("sandbox_policy is invalid: {err}"))
        })?;
    Ok(config)
}
