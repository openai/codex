//! Delivers captured agent input without exposing local loading and eviction to callers.
//!
//! Target checks precede reload, and queue-only messages retain their non-waking semantics.

use super::LocalAgentControl;
use crate::agent::api::AgentInput;
use crate::agent::api::DeliveryReceipt;
use crate::agent::api::SendRequest;
use crate::agent::types::AgentMessage;
use crate::agent::types::MessageDeliveryMode;
use crate::agent_communication::AgentCommunicationContext;
use crate::agent_communication::AgentCommunicationKind;
use crate::context::ContextualUserFragment;
use crate::context::InterAgentMessage;
use crate::context::InterAgentMessageType;
use codex_protocol::AgentPath;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::InterAgentCommunication;

impl AgentMessage {
    pub(crate) fn into_communication(
        self,
        author: AgentPath,
        recipient: AgentPath,
        mode: MessageDeliveryMode,
    ) -> InterAgentCommunication {
        let trigger_turn = mode == MessageDeliveryMode::TriggerTurn;
        match self {
            Self::Encrypted(message) => InterAgentCommunication::new_encrypted(
                author,
                recipient,
                Vec::new(),
                message,
                trigger_turn,
            ),
            Self::Plaintext(message) => {
                let message_type = match mode {
                    MessageDeliveryMode::QueueOnly => InterAgentMessageType::Message,
                    MessageDeliveryMode::TriggerTurn => InterAgentMessageType::NewTask,
                };
                let content = InterAgentMessage::new(
                    message_type,
                    recipient.clone(),
                    author.clone(),
                    message,
                )
                .render();
                InterAgentCommunication::new(author, recipient, Vec::new(), content, trigger_turn)
            }
        }
    }
}

impl LocalAgentControl {
    /// Resolves and delivers captured input, restoring an evicted runtime when necessary.
    pub(crate) async fn send(&self, request: SendRequest) -> CodexResult<DeliveryReceipt> {
        let SendRequest {
            caller,
            target,
            resume_config,
            input,
            mut start_options,
        } = request;
        let target = self.resolve_target(caller, &target)?;
        let (metadata, submission_id) = match input {
            AgentInput::UserInput(input) => {
                let receiver = self.get_agent_metadata(target);
                if receiver.is_some() {
                    self.ensure_v2_agent_loaded(resume_config, target, /*parent*/ None)
                        .await?;
                }
                let submission_id = self.send_input(target, input, start_options).await?;
                (receiver.unwrap_or_default(), submission_id)
            }
            AgentInput::Message { message, mode } => {
                let receiver = self.ensure_agent_known(target)?;
                let author = self
                    .ensure_agent_known(caller)?
                    .agent_path
                    .unwrap_or_else(AgentPath::root);
                if mode == MessageDeliveryMode::TriggerTurn
                    && receiver.agent_path.as_ref().is_some_and(AgentPath::is_root)
                {
                    return Err(CodexErr::UnsupportedOperation(
                        "Follow-up tasks can't target the root agent".to_string(),
                    ));
                }
                let receiver_path = receiver.agent_path.clone().ok_or_else(|| {
                    CodexErr::UnsupportedOperation(
                        "target agent is missing an agent_path".to_string(),
                    )
                })?;
                self.ensure_v2_agent_loaded(resume_config, target, /*parent*/ None)
                    .await?;
                let communication = message.into_communication(author, receiver_path, mode);
                let kind = match mode {
                    MessageDeliveryMode::QueueOnly => {
                        start_options.parent_turn_id = None;
                        AgentCommunicationKind::Message
                    }
                    MessageDeliveryMode::TriggerTurn => AgentCommunicationKind::Followup,
                };
                let submission_id = self
                    .send_inter_agent_communication(
                        target,
                        communication,
                        AgentCommunicationContext::new(kind, caller),
                        start_options,
                    )
                    .await?;
                (receiver, submission_id)
            }
        };
        Ok(DeliveryReceipt {
            thread_id: target,
            metadata,
            submission_id,
        })
    }
}
