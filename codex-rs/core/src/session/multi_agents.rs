use crate::config::DEFAULT_MULTI_AGENT_V2_ROOT_AGENT_USAGE_HINT_TEXT;
use crate::config::DEFAULT_MULTI_AGENT_V2_SUBAGENT_USAGE_HINT_TEXT;
use crate::config::MultiAgentV2Config;
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

    match session_source {
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn { .. }) => {
            rendered_subagent_usage_hint_text(multi_agent_v2)
        }
        SessionSource::Cli
        | SessionSource::VSCode
        | SessionSource::Exec
        | SessionSource::Mcp
        | SessionSource::Custom(_)
        | SessionSource::Unknown => rendered_root_usage_hint_text(multi_agent_v2),
        SessionSource::Internal(_) | SessionSource::SubAgent(_) => None,
    }
}

pub(crate) fn rendered_root_usage_hint_text(multi_agent_v2: &MultiAgentV2Config) -> Option<String> {
    render_usage_hint_text(
        multi_agent_v2.root_agent_usage_hint_text.as_deref(),
        multi_agent_v2.max_concurrent_threads_per_session,
    )
}

pub(crate) fn rendered_subagent_usage_hint_text(
    multi_agent_v2: &MultiAgentV2Config,
) -> Option<String> {
    render_usage_hint_text(
        multi_agent_v2.subagent_usage_hint_text.as_deref(),
        multi_agent_v2.max_concurrent_threads_per_session,
    )
}

fn render_usage_hint_text(
    usage_hint_text: Option<&str>,
    max_concurrent_threads_per_session: usize,
) -> Option<String> {
    usage_hint_text.map(|usage_hint_text| {
        format_usage_hint_text(usage_hint_text, max_concurrent_threads_per_session)
    })
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
