use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::codex_delegate;
use crate::error::CodexErr;
use crate::error::Result;
use crate::features::Feature;
use crate::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;

pub(crate) async fn run_subagent_one_shot_with_definition(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    definition_prompt: String,
    source: SubAgentSource,
    prompt: String,
    cancellation_token: &CancellationToken,
) -> Result<String> {
    let mut sub_config = (*turn_context.client.config()).clone();
    sub_config
        .sandbox_policy
        .set(SandboxPolicy::new_read_only_policy())
        .map_err(|err| CodexErr::InvalidRequest(err.to_string()))?;
    sub_config.features.disable(Feature::ApplyPatchFreeform);
    sub_config.developer_instructions = Some(definition_prompt);

    let input = vec![UserInput::Text { text: prompt }];

    let io = codex_delegate::run_codex_conversation_one_shot(
        sub_config,
        Arc::clone(&sess.services.auth_manager),
        Arc::clone(&sess.services.models_manager),
        input,
        Arc::clone(sess),
        Arc::clone(turn_context),
        cancellation_token.child_token(),
        SessionSource::SubAgent(source),
        None,
    )
    .await?;

    let mut output_text: Option<String> = None;
    while let Ok(event) = io.rx_event.recv().await {
        match event.msg {
            crate::protocol::EventMsg::TaskComplete(task_complete) => {
                output_text = task_complete.last_agent_message;
                break;
            }
            crate::protocol::EventMsg::TurnAborted(_) => return Err(CodexErr::TurnAborted),
            crate::protocol::EventMsg::Error(err) => {
                return Err(CodexErr::InvalidRequest(err.message));
            }
            _ => {}
        }
    }

    output_text.ok_or_else(|| CodexErr::Fatal("subagent finished without output".to_string()))
}
