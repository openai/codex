use crate::config::DEFAULT_MULTI_AGENT_V2_ROOT_AGENT_USAGE_HINT_TEXT;
use crate::config::DEFAULT_MULTI_AGENT_V2_SUBAGENT_USAGE_HINT_TEXT;
use crate::session::turn_context::TurnContext;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;

pub(super) fn usage_hint_text(
    turn_context: &TurnContext,
    session_source: &SessionSource,
) -> Option<String> {
    if turn_context.multi_agent_version != MultiAgentVersion::V2 {
        return None;
    }

    let multi_agent_v2 = &turn_context.config.multi_agent_v2;
    if !multi_agent_v2.usage_hint_enabled {
        return None;
    }

    let usage_hint_text = match session_source {
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
    }?;

    Some(format_usage_hint_text(
        usage_hint_text,
        multi_agent_v2.max_concurrent_threads_per_session,
    ))
}

fn format_usage_hint_text(
    usage_hint_text: &str,
    max_concurrent_threads_per_session: usize,
) -> String {
    // Keep the built-in hint text aligned with the resolved limit instead of
    // baking a stale number into the default strings.
    if usage_hint_text == DEFAULT_MULTI_AGENT_V2_ROOT_AGENT_USAGE_HINT_TEXT
        || usage_hint_text == DEFAULT_MULTI_AGENT_V2_SUBAGENT_USAGE_HINT_TEXT
    {
        return format!(
            "{usage_hint_text}\nThere are {max_concurrent_threads_per_session} available concurrency slots, meaning that up to {max_concurrent_threads_per_session} agents can be active at once, including you."
        );
    }

    usage_hint_text.to_string()
}
