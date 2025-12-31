use async_trait::async_trait;
use serde::Deserialize;
use std::time::Instant;

use crate::function_tool::FunctionCallError;
use crate::protocol::EventMsg;
use crate::protocol::SessionSource;
use crate::protocol::SubAgentRunBeginEvent;
use crate::protocol::SubAgentRunEndEvent;
use crate::subagent_runner::run_subagent_one_shot_with_definition;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct RunSubagentHandler;

#[derive(Deserialize)]
struct RunSubagentArgs {
    name: String,
    prompt: String,
}

fn valid_subagent_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[async_trait]
impl ToolHandler for RunSubagentHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            call_id,
            turn,
            session,
            payload,
            cancellation_token,
            ..
        } = invocation;

        if matches!(turn.client.get_session_source(), SessionSource::SubAgent(_)) {
            return Err(FunctionCallError::RespondToModel(
                "run_subagent is not available from within a subagent session".to_string(),
            ));
        }

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "run_subagent handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: RunSubagentArgs = serde_json::from_str(&arguments).map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "failed to parse function arguments: {err:?}"
            ))
        })?;

        let RunSubagentArgs { name, prompt } = args;
        if !valid_subagent_name(&name) {
            return Err(FunctionCallError::RespondToModel(
                "invalid subagent name".to_string(),
            ));
        }

        let prompt = prompt.trim().to_string();
        if prompt.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "prompt must be non-empty".to_string(),
            ));
        }

        let started = Instant::now();

        let definition = crate::subagents::resolve_subagent_definition_with_sources(
            &turn.cwd,
            &turn.codex_home,
            &turn.agents_sources,
            &name,
        )
        .await
        .map_err(|e| FunctionCallError::RespondToModel(e.to_string()))?;

        session
            .send_event(
                turn.as_ref(),
                EventMsg::SubAgentRunBegin(SubAgentRunBeginEvent {
                    call_id: call_id.clone(),
                    name: definition.name.clone(),
                    description: definition.description.clone(),
                    color: definition.color.clone(),
                    prompt: prompt.clone(),
                }),
            )
            .await;

        session
            .notify_background_event(
                turn.as_ref(),
                format!("Subagent @{} を実行中…", definition.name),
            )
            .await;

        let subagent_cancellation_token = cancellation_token.child_token();
        let result = run_subagent_one_shot_with_definition(
            &session,
            &turn,
            definition.prompt.clone(),
            definition.source.clone(),
            prompt,
            &subagent_cancellation_token,
        )
        .await;

        let duration_ms = i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX);
        session
            .send_event(
                turn.as_ref(),
                EventMsg::SubAgentRunEnd(SubAgentRunEndEvent {
                    call_id: call_id.clone(),
                    duration_ms,
                    success: result.is_ok(),
                }),
            )
            .await;

        if result.is_ok() {
            session
                .notify_background_event(
                    turn.as_ref(),
                    format!("Subagent @{} 完了（{}ms）", definition.name, duration_ms),
                )
                .await;
        }

        let output = result.map_err(|e| FunctionCallError::RespondToModel(e.to_string()))?;
        Ok(ToolOutput::Function {
            content: format!("Subagent: @{}\n\n{output}", definition.name),
            content_items: None,
            success: Some(true),
        })
    }
}
