use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use codex_protocol::ConversationId;
use serde::Deserialize;

use crate::function_tool::FunctionCallError;
use crate::subsessions::SessionType;
use crate::subsessions::SubsessionError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct SubsessionsHandler;

#[derive(Deserialize)]
struct CreateSessionArgs {
    session_type: String,
    prompt: String,
}

#[derive(Deserialize)]
struct WaitSessionArgs {
    session_id: String,
    timeout_ms: Option<i32>,
}

#[derive(Deserialize)]
struct CancelSessionArgs {
    session_id: String,
}

#[async_trait]
impl ToolHandler for SubsessionsHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
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
                return Err(FunctionCallError::RespondToModel(format!(
                    "{tool_name} received unsupported payload"
                )));
            }
        };

        match tool_name.as_str() {
            "create_session" => {
                let args: CreateSessionArgs = parse_args(&arguments)?;
                handle_create_session(session, turn, args).await
            }
            "wait_session" => {
                let args: WaitSessionArgs = parse_args(&arguments)?;
                handle_wait_session(session, args).await
            }
            "cancel_session" => {
                let args: CancelSessionArgs = parse_args(&arguments)?;
                handle_cancel_session(session, args).await
            }
            _ => Err(FunctionCallError::RespondToModel(format!(
                "unsupported subsession tool {tool_name}"
            ))),
        }
    }
}

fn parse_args<T: for<'de> Deserialize<'de>>(arguments: &str) -> Result<T, FunctionCallError> {
    serde_json::from_str(arguments).map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to parse arguments: {err}"))
    })
}

async fn handle_create_session(
    session: Arc<crate::codex::Session>,
    turn: Arc<crate::codex::TurnContext>,
    args: CreateSessionArgs,
) -> Result<ToolOutput, FunctionCallError> {
    let session_type = args.session_type.parse::<SessionType>().map_err(|_| {
        FunctionCallError::RespondToModel(format!("unknown session_type {}", args.session_type))
    })?;

    let subsessions = Arc::clone(&session.services.subsessions);
    let conversation_id = subsessions
        .spawn_child(session, turn, session_type, args.prompt)
        .await
        .map_err(map_subsession_err)?;

    let payload = serde_json::json!({ "session_id": conversation_id.to_string() });
    Ok(ToolOutput::Function {
        content: payload.to_string(),
        success: Some(true),
    })
}

async fn handle_wait_session(
    session: Arc<crate::codex::Session>,
    args: WaitSessionArgs,
) -> Result<ToolOutput, FunctionCallError> {
    let conversation_id = parse_session_id(&args.session_id)?;
    let subsessions = Arc::clone(&session.services.subsessions);
    let timeout = args.timeout_ms.map(|value| {
        if value > 0 {
            Duration::from_millis(value as u64)
        } else {
            Duration::ZERO
        }
    });

    let result = subsessions
        .wait_child(&conversation_id, timeout)
        .await
        .map_err(map_subsession_err)?;

    let payload = serde_json::json!({ "result": result });
    Ok(ToolOutput::Function {
        content: payload.to_string(),
        success: Some(true),
    })
}

async fn handle_cancel_session(
    session: Arc<crate::codex::Session>,
    args: CancelSessionArgs,
) -> Result<ToolOutput, FunctionCallError> {
    let conversation_id = parse_session_id(&args.session_id)?;
    let subsessions = Arc::clone(&session.services.subsessions);
    let cancelled = subsessions
        .cancel_child(&conversation_id, Arc::clone(&session))
        .await
        .map_err(map_subsession_err)?;

    let payload = serde_json::json!({ "cancelled": cancelled });
    Ok(ToolOutput::Function {
        content: payload.to_string(),
        success: Some(true),
    })
}

fn parse_session_id(value: &str) -> Result<ConversationId, FunctionCallError> {
    ConversationId::from_string(value).map_err(|err| {
        FunctionCallError::RespondToModel(format!("invalid session_id {value}: {err}"))
    })
}

fn map_subsession_err(err: SubsessionError) -> FunctionCallError {
    FunctionCallError::RespondToModel(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_session_id() {
        let err = parse_session_id("not-a-uuid").expect_err("invalid id");
        let FunctionCallError::RespondToModel(message) = err else {
            panic!("expected respond error");
        };
        assert!(message.contains("invalid session_id"));
    }

    #[test]
    fn parses_valid_session_id() {
        let id = ConversationId::default();
        let parsed = parse_session_id(&id.to_string()).expect("parse id");
        assert_eq!(parsed.to_string(), id.to_string());
    }
}
