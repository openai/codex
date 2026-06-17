use crate::config::DEFAULT_MULTI_AGENT_V2_NO_SPAWN_HINT_TEXT;
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
    usage_hint_text_for(
        turn_context.multi_agent_version,
        &turn_context.config.multi_agent_v2,
        session_source,
    )
}

pub(super) fn usage_hint_text_for<'a>(
    multi_agent_version: MultiAgentVersion,
    multi_agent_v2: &'a MultiAgentV2Config,
    session_source: &SessionSource,
) -> Option<&'a str> {
    if multi_agent_version != MultiAgentVersion::V2 {
        return None;
    }

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

    Some(stable_usage_hint_text(usage_hint_text))
}

pub(super) fn spawn_policy_is_supported(
    turn_context: &TurnContext,
    session_source: &SessionSource,
) -> bool {
    usage_hint_text(turn_context, session_source).is_some()
}

pub(crate) fn stable_usage_hint_text(usage_hint_text: &str) -> &str {
    usage_hint_text
        .strip_suffix(DEFAULT_MULTI_AGENT_V2_NO_SPAWN_HINT_TEXT)
        .and_then(|usage_hint_text| usage_hint_text.strip_suffix("\n\n"))
        .unwrap_or(usage_hint_text)
}

pub(crate) fn spawn_policy_for_turn(
    turn_context: &TurnContext,
    session_source: &SessionSource,
) -> Option<MultiAgentMode> {
    spawn_policy_is_supported(turn_context, session_source).then_some(turn_context.multi_agent_mode)
}
