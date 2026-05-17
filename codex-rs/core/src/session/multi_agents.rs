use crate::session::turn_context::TurnContext;
use codex_features::Feature;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;

const DEFERRED_MULTI_AGENT_V1_USAGE_HINT: &str = r#"
Only use sub-agents if and only if the user explicitly asks for sub-agents, delegation, or parallel agent work.
Requests for depth, thoroughness, research, investigation, or detailed codebase analysis do not count as permission to spawn.

### When to delegate vs. do the subtask yourself
- First, quickly analyze the overall user task and form a succinct high-level plan. Identify which tasks are immediate blockers on the critical path, and which tasks are sidecar tasks that are needed but can run in parallel without blocking the next local step. As part of that plan, explicitly decide what immediate task you should do locally right now. Do this planning step before delegating to agents so you do not hand off the immediate blocking task to a submodel and then waste time waiting on it.
- Use a subagent when a subtask is easy enough for it to handle and can run in parallel with your local work. Prefer delegating concrete, bounded sidecar tasks that materially advance the main task without blocking your immediate next local step.
- Do not delegate urgent blocking work when your immediate next step depends on that result. If the very next action is blocked on that task, the main rollout should usually do it locally to keep the critical path moving.
- Keep work local when the subtask is too difficult to delegate well and when it is tightly coupled, urgent, or likely to block your immediate next local step."#;

pub(super) fn usage_hint_text<'a>(
    turn_context: &'a TurnContext,
    session_source: &SessionSource,
) -> Option<&'a str> {
    if !turn_context.features.enabled(Feature::MultiAgentV2) {
        return deferred_multi_agent_v1_usage_hint_text(turn_context);
    }

    let multi_agent_v2 = &turn_context.config.multi_agent_v2;
    match session_source {
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn { .. }) => {
            multi_agent_v2.subagent_usage_hint_text.as_deref()
        }
        SessionSource::Cli
        | SessionSource::VSCode
        | SessionSource::Exec
        | SessionSource::Mcp
        | SessionSource::Custom(_)
        | SessionSource::Unknown => multi_agent_v2.root_agent_usage_hint_text.as_deref(),
        SessionSource::Internal(_) | SessionSource::SubAgent(_) => None,
    }
}

fn deferred_multi_agent_v1_usage_hint_text(turn_context: &TurnContext) -> Option<&str> {
    let tools_config = &turn_context.tools_config;
    if !(tools_config.collab_tools
        && !tools_config.multi_agent_v2
        && tools_config.search_tool
        && tools_config.namespace_tools
        && tools_config.spawn_agent_usage_hint)
    {
        return None;
    }

    tools_config
        .spawn_agent_usage_hint_text
        .as_deref()
        .or(Some(DEFERRED_MULTI_AGENT_V1_USAGE_HINT))
}
