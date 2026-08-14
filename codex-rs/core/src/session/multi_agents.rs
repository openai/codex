use crate::config::MultiAgentV2Config;
use crate::context::MultiAgentRoleInstructions;
use crate::session::turn_context::TurnContext;
use codex_protocol::config_types::MultiAgentMode;
use codex_protocol::openai_models::MultiAgentRoleMessages;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;

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
const DEFAULT_MULTI_AGENT_V2_MODEL_OVERRIDE_USAGE_HINT_TEXT: &str = "Full-history forks (`fork_turns` omitted or `\"all\"`) inherit the parent model and reasoning effort and do not accept overrides. Only set `model` or `reasoning_effort` when explicitly requested by the user, applicable `AGENTS.md` instructions, or skill instructions; when doing so, set `fork_turns` to `\"none\"` or a positive integer string.";
const DEFAULT_MULTI_AGENT_V2_WAIT_AGENT_USAGE_HINT_TEXT: &str =
    "When calling `wait_agent`, prefer longer waits (minutes) to avoid busy polling.";
const DEFAULT_MULTI_AGENT_V2_SHARED_USAGE_HINT_TEXT: &str = r#"Note that collaboration tools cannot be called from inside `functions.exec`. Call `spawn_agent`, `send_message`, `followup_task`, `wait_agent`, `interrupt_agent`, and `list_agents` only as direct tool calls using the recipient shown in their tool definitions, such as `to=functions.collaboration.spawn_agent`, since they are intentionally absent from the `functions.exec` `tools.*` namespace. Available tools in `functions.exec` are explicitly described with a `tools` namespace in the developer message.

All agents share the same directory. In detail:
- All agents have access to the same container and filesystem as you.
- All agents use the same current working directory.
- As a result, edits made by one agent are immediately visible to all other agents.
"#;

#[derive(Clone, Debug, Default)]
pub(crate) struct ResolvedMultiAgentV2UsageHints {
    pub(crate) root: Option<MultiAgentRoleInstructions>,
    pub(crate) subagent: Option<MultiAgentRoleInstructions>,
}

pub(super) fn usage_hint_text(
    turn_context: &TurnContext,
    session_source: &SessionSource,
) -> Option<MultiAgentRoleInstructions> {
    if turn_context.multi_agent_version != MultiAgentVersion::V2 {
        return None;
    }

    let catalog = turn_context
        .model_info
        .model_messages
        .as_ref()
        .and_then(|messages| messages.multi_agent.as_ref())
        .and_then(|messages| messages.role.as_ref());
    let snapshot = resolve_usage_hints(&turn_context.config.multi_agent_v2, catalog);
    match session_source {
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn { .. }) => snapshot.subagent,
        SessionSource::Cli
        | SessionSource::VSCode
        | SessionSource::Exec
        | SessionSource::Mcp
        | SessionSource::Custom(_)
        | SessionSource::Unknown => snapshot.root,
        SessionSource::Internal(_) | SessionSource::SubAgent(_) => None,
    }
}

pub(crate) fn resolve_usage_hints(
    config: &MultiAgentV2Config,
    catalog: Option<&MultiAgentRoleMessages>,
) -> ResolvedMultiAgentV2UsageHints {
    let resolve_role = |configured: Option<&str>, catalog: Option<&str>, bundled: &str| {
        // Configured roles take precedence; empty configured or catalog roles suppress fallback.
        if let Some(configured) = configured {
            return (!configured.is_empty())
                .then(|| MultiAgentRoleInstructions::unmarked(configured));
        }

        let base = catalog.unwrap_or(bundled);
        if base.is_empty() {
            return None;
        }

        let max_concurrency = config.max_concurrent_threads_per_session;
        let wait_agent_guidance = if config.wait_agent_enabled {
            format!("{DEFAULT_MULTI_AGENT_V2_WAIT_AGENT_USAGE_HINT_TEXT}\n\n")
        } else {
            String::new()
        };
        let mut text = format!(
            "{base}\n{DEFAULT_MULTI_AGENT_V2_SHARED_USAGE_HINT_TEXT}\n{wait_agent_guidance}There are {max_concurrency} available concurrency slots, meaning that up to {max_concurrency} agents can be active at once, including you."
        );
        if config.expose_spawn_agent_model_overrides {
            text.push_str("\n\n");
            text.push_str(DEFAULT_MULTI_AGENT_V2_MODEL_OVERRIDE_USAGE_HINT_TEXT);
        }

        Some(if catalog.is_some() {
            MultiAgentRoleInstructions::catalog(text)
        } else {
            MultiAgentRoleInstructions::unmarked(text)
        })
    };

    ResolvedMultiAgentV2UsageHints {
        root: resolve_role(
            config.root_agent_usage_hint_text.as_deref(),
            catalog.and_then(|messages| messages.root.as_deref()),
            DEFAULT_MULTI_AGENT_V2_ROOT_AGENT_USAGE_HINT_TEXT,
        ),
        subagent: resolve_role(
            config.subagent_usage_hint_text.as_deref(),
            catalog.and_then(|messages| messages.subagent.as_deref()),
            DEFAULT_MULTI_AGENT_V2_SUBAGENT_USAGE_HINT_TEXT,
        ),
    }
}

pub(crate) fn effective_multi_agent_mode(turn_context: &TurnContext) -> Option<MultiAgentMode> {
    if turn_context.multi_agent_version != MultiAgentVersion::V2 {
        return None;
    }

    let catalog_mode = turn_context
        .model_info
        .model_messages
        .as_ref()
        .and_then(|messages| messages.multi_agent.as_ref())
        .and_then(|messages| messages.mode.as_ref());
    let mode_hint_text = turn_context
        .config
        .multi_agent_v2
        .multi_agent_mode_hint_text
        .as_deref()
        .or_else(|| catalog_mode.and_then(|mode| mode.hint_text.as_deref()));

    // A configured or catalog hint, including an empty string, defines a custom policy instead
    // of an effort-derived built-in policy.
    let multi_agent_mode = match mode_hint_text {
        Some(hint_text) => MultiAgentMode::Custom(hint_text.to_string()),
        None => match turn_context.effective_reasoning_effort() {
            Some(ReasoningEffort::Ultra) => MultiAgentMode::Proactive,
            _ => catalog_mode
                .and_then(|messages| messages.explicit.clone())
                .map(MultiAgentMode::Custom)
                .unwrap_or(MultiAgentMode::ExplicitRequestOnly),
        },
    };

    match &turn_context.session_source {
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn { .. })
        | SessionSource::Cli
        | SessionSource::VSCode
        | SessionSource::Exec
        | SessionSource::Mcp
        | SessionSource::Custom(_)
        | SessionSource::Unknown => Some(multi_agent_mode),
        SessionSource::Internal(_) | SessionSource::SubAgent(_) => None,
    }
}
