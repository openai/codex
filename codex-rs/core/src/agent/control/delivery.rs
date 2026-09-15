//! Delivers V2 messages without exposing local loading and eviction to callers.
//!
//! Target checks precede reload, and queue-only messages retain their non-waking semantics.

use super::AgentControl;
use crate::TurnStartOptions;
use crate::agent::child_config::build_agent_resume_config;
use crate::agent_communication::AgentCommunicationContext;
use crate::agent_communication::AgentCommunicationKind;
use crate::context::ContextualUserFragment;
use crate::context::InterAgentMessage;
use crate::context::InterAgentMessageType;
use crate::session::turn_context::TurnContext;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::protocol::InterAgentCommunication;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum MessageDeliveryMode {
    QueueOnly,
    TriggerTurn,
}

/// Keeps model-provided encrypted content distinct from text that needs a context wrapper.
pub(crate) enum AgentMessage {
    Plaintext(String),
    Encrypted(String),
}

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

/// Separates request validation from agent runtime failures so adapters retain their error text.
#[derive(Debug)]
pub(crate) enum MessageDeliveryError {
    InvalidRequest(String),
    Agent(CodexErr),
}

impl AgentControl {
    /// Checks and delivers to a resolved target, restoring an evicted runtime when necessary.
    ///
    /// The caller resolves tool-facing names separately so it can attribute failures and
    /// interruptions to the target before delivery starts.
    pub(crate) async fn deliver_message(
        &self,
        caller: ThreadId,
        turn: &TurnContext,
        target: ThreadId,
        message: AgentMessage,
        mode: MessageDeliveryMode,
    ) -> Result<AgentPath, MessageDeliveryError> {
        let receiver_agent = self
            .ensure_agent_known(target)
            .map_err(MessageDeliveryError::Agent)?;
        if mode == MessageDeliveryMode::TriggerTurn
            && receiver_agent
                .agent_path
                .as_ref()
                .is_some_and(AgentPath::is_root)
        {
            return Err(MessageDeliveryError::InvalidRequest(
                "Follow-up tasks can't target the root agent".to_string(),
            ));
        }
        let receiver_agent_path = receiver_agent.agent_path.clone().ok_or_else(|| {
            MessageDeliveryError::InvalidRequest(
                "target agent is missing an agent_path".to_string(),
            )
        })?;
        let resume_config =
            build_agent_resume_config(turn).map_err(MessageDeliveryError::InvalidRequest)?;
        self.ensure_v2_agent_loaded(resume_config, target, /*parent*/ None)
            .await
            .map_err(MessageDeliveryError::Agent)?;
        let author = turn
            .session_source
            .get_agent_path()
            .unwrap_or_else(AgentPath::root);
        let communication = message.into_communication(author, receiver_agent_path.clone(), mode);
        let kind = match mode {
            MessageDeliveryMode::QueueOnly => AgentCommunicationKind::Message,
            MessageDeliveryMode::TriggerTurn => AgentCommunicationKind::Followup,
        };
        let context = AgentCommunicationContext::new(kind, caller);
        let parent_turn_id =
            matches!(mode, MessageDeliveryMode::TriggerTurn).then(|| turn.sub_id.clone());
        self.send_inter_agent_communication(
            target,
            communication,
            context,
            TurnStartOptions {
                parent_turn_id,
                root_turn_id: turn.turn_metadata_state.root_turn_id(),
                turn_trigger: turn.turn_metadata_state.current_turn_trigger(),
                cyber_access_program: turn.cyber_access_program,
                ..Default::default()
            },
        )
        .await
        .map_err(MessageDeliveryError::Agent)?;
        Ok(receiver_agent_path)
    }
}
