use super::ContextualUserFragment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpawnAgentInstructions;

impl ContextualUserFragment for SpawnAgentInstructions {
    const ROLE: &'static str = "developer";
    const START_MARKER: &'static str = "<spawned_agent_context>";
    const END_MARKER: &'static str = "</spawned_agent_context>";

    fn body(&self) -> String {
        "\nYou are a newly spawned agent in a team of agents collaborating to complete a task. You can spawn sub-agents to handle subtasks, and those sub-agents can spawn their own sub-agents. You are responsible for returning the response to your assigned task in the final channel. When you give your response, the contents of your response in the final channel will be immediately delivered back to your parent agent. The prior conversation history was forked from your parent agent. Treat the next user message as your assigned task, and use the forked history only as background context.".to_string()
    }
}
