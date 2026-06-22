use crate::config::MultiAgentV2Config;
use crate::session::turn_context::TurnContext;
use codex_protocol::config_types::MultiAgentMode;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;

pub(super) fn usage_hint_text<'a>(
    turn_context: &'a TurnContext,
    session_source: &SessionSource,
) -> Option<&'a str> {
    if turn_context.multi_agent_version != MultiAgentVersion::V2 {
        return None;
    }

    let multi_agent_v2 = &turn_context.config.multi_agent_v2;
    if !multi_agent_v2.usage_hint_enabled {
        return None;
    }

    configured_usage_hint_text_for_source(multi_agent_v2, session_source)
}

fn configured_usage_hint_text_for_source<'a>(
    multi_agent_v2: &'a MultiAgentV2Config,
    session_source: &SessionSource,
) -> Option<&'a str> {
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

fn multi_agent_mode_is_applicable(
    multi_agent_version: MultiAgentVersion,
    multi_agent_v2: &MultiAgentV2Config,
    session_source: &SessionSource,
) -> bool {
    multi_agent_version == MultiAgentVersion::V2
        && multi_agent_v2.usage_hint_enabled
        && configured_usage_hint_text_for_source(multi_agent_v2, session_source).is_some()
}

pub(crate) fn effective_multi_agent_mode(
    multi_agent_version: MultiAgentVersion,
    multi_agent_v2: &MultiAgentV2Config,
    session_source: &SessionSource,
    requested_multi_agent_mode: Option<MultiAgentMode>,
    multi_agent_mode_enabled: bool,
) -> Option<MultiAgentMode> {
    if !multi_agent_mode_is_applicable(multi_agent_version, multi_agent_v2, session_source) {
        return None;
    }

    Some(if multi_agent_mode_enabled {
        requested_multi_agent_mode.unwrap_or_default()
    } else {
        MultiAgentMode::ExplicitRequestOnly
    })
}
