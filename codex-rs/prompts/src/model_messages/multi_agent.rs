//! Resolves multi-agent role bases and mode text while preserving catalog provenance.
//! Consumers own role assembly, suppression, and effort-dependent mode selection.

use super::ResolvedMessage;
use codex_protocol::openai_models::MultiAgentMessages;

const DEFAULT_MULTI_AGENT_V2_ROOT_AGENT_USAGE_HINT_TEXT: &str = r#"You are `/root`, the primary agent in a team of agents collaborating to fulfill the user's goals.

At the start of your turn, you are the active agent.
You can spawn sub-agents to handle subtasks, and those sub-agents can spawn their own sub-agents.
All agents in the team, including the agents that you can assign tasks to, are equally intelligent and capable, and have access to the same set of tools.

You can use `spawn_agent` to create a new agent, `followup_task` to give an existing agent a new task and trigger a turn, and `send_message` to pass a message to a running agent without triggering a turn.
Child agents can also spawn their own sub-agents.
You can decide how much context you want to propagate to your sub-agents with the `fork_turns` parameter.

You will receive messages in the analysis channel in the form:
```
Message Type: MESSAGE | FINAL_ANSWER
Task name: <recipient>
Sender: <author>
Payload:
<payload text>
```
They may be addressed as to=/root
"#;
const DEFAULT_MULTI_AGENT_V2_SUBAGENT_USAGE_HINT_TEXT: &str = r#"You are an agent in a team of agents collaborating to complete a task.

You can spawn sub-agents to handle subtasks, and those sub-agents can spawn their own sub-agents. All agents in the team, including the agents that you can assign tasks to, are equally intelligent and capable, and have access to the same set of tools.

You can use `spawn_agent` to create a new agent, `followup_task` to give an existing agent a new task and trigger a turn, and `send_message` to pass a message to a running agent.
Child agents can also spawn their own sub-agents.

When you provide a response in the final channel, that content is immediately delivered back to your parent agent.

You will receive messages in the analysis channel in the form:
```
Message Type: NEW_TASK | MESSAGE | FINAL_ANSWER
Task name: <recipient>
Sender: <author>
Payload:
<payload text>
```
You may also see them addressed as to=/root/..., which indicates your identity is /root/...
"#;
const EXPLICIT_REQUEST_ONLY_MULTI_AGENT_MODE_TEXT: &str = "Any earlier instruction enabling proactive multi-agent delegation no longer applies. Do not spawn sub-agents unless the user or applicable AGENTS.md/skill instructions explicitly ask for sub-agents, delegation, or parallel agent work.";
const PROACTIVE_MULTI_AGENT_MODE_TEXT: &str = "Proactive multi-agent delegation is active. Any earlier developer instruction requiring an explicit user request before spawning sub-agents no longer applies. This mode remains active until a later multi-agent mode developer message changes it. User requests override this hint.\n\nIf at any point you can parallelize work by delegating tasks to another agent (no matter if you are root or subagent), you should do so using collaboration tools if it could save time or improve quality.";

/// Model-only role bases and mode alternatives for runtime selection.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedMultiAgentMessages<'a> {
    pub root: ResolvedMessage<'a>,
    pub subagent: ResolvedMessage<'a>,
    pub explicit: ResolvedMessage<'a>,
    pub proactive: ResolvedMessage<'a>,
    /// A supplied hint overrides both effort-specific modes, including when empty.
    pub hint: Option<&'a str>,
}

impl<'a> ResolvedMultiAgentMessages<'a> {
    pub(crate) fn new(messages: Option<&'a MultiAgentMessages>) -> Self {
        let role = messages.and_then(|messages| messages.role.as_ref());
        let mode = messages.and_then(|messages| messages.mode.as_ref());
        Self {
            root: ResolvedMessage::new(
                role.and_then(|role| role.root.as_deref()),
                DEFAULT_MULTI_AGENT_V2_ROOT_AGENT_USAGE_HINT_TEXT,
            ),
            subagent: ResolvedMessage::new(
                role.and_then(|role| role.subagent.as_deref()),
                DEFAULT_MULTI_AGENT_V2_SUBAGENT_USAGE_HINT_TEXT,
            ),
            explicit: ResolvedMessage::new(
                mode.and_then(|mode| mode.explicit.as_deref()),
                EXPLICIT_REQUEST_ONLY_MULTI_AGENT_MODE_TEXT,
            ),
            proactive: ResolvedMessage::new(
                mode.and_then(|mode| mode.proactive.as_deref()),
                PROACTIVE_MULTI_AGENT_MODE_TEXT,
            ),
            hint: mode.and_then(|mode| mode.hint_text.as_deref()),
        }
    }
}
