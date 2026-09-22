//! Converts captured spawn input into the existing local startup operation.
//! Agent messages keep their canonical paths and spawn attribution.

use super::LocalAgentControl;
use super::spawn::SpawnInitialInput;
use crate::agent::api::AgentInput;
use crate::agent::api::SpawnRequest;
use crate::agent::types::LiveAgent;
use crate::agent::types::MessageDeliveryMode;
use crate::agent_communication::AgentCommunicationContext;
use crate::agent_communication::AgentCommunicationKind;
use crate::codex_thread::ThreadConfigSnapshot;
use codex_protocol::AgentPath;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;

impl LocalAgentControl {
    pub(crate) async fn spawn(
        &self,
        request: SpawnRequest,
    ) -> CodexResult<(LiveAgent, ThreadConfigSnapshot)> {
        let SpawnRequest {
            caller,
            config,
            input,
            source,
            options,
        } = request;
        let input = match input {
            AgentInput::UserInput(input) => SpawnInitialInput::UserInput(input),
            AgentInput::Message { message, mode } => {
                if mode != MessageDeliveryMode::TriggerTurn {
                    return Err(CodexErr::InvalidRequest(
                        "spawn input must start the child turn".to_string(),
                    ));
                }
                let recipient = source.get_agent_path().ok_or_else(|| {
                    CodexErr::InvalidRequest(
                        "spawned agent is missing a canonical task name".to_string(),
                    )
                })?;
                let author = recipient
                    .as_str()
                    .rsplit_once('/')
                    .and_then(|(parent, _)| AgentPath::try_from(parent).ok())
                    .ok_or_else(|| {
                        CodexErr::InvalidRequest("spawn input needs a child path".to_string())
                    })?;
                SpawnInitialInput::InterAgentCommunication(
                    message.into_communication(author, recipient, mode),
                    AgentCommunicationContext::new(AgentCommunicationKind::Spawn, caller),
                )
            }
        };
        Box::pin(self.spawn_agent_internal(config, input, Some(source), options)).await
    }
}
