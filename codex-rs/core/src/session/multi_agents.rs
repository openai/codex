use crate::session::turn_context::TurnContext;
use codex_features::Feature;
use codex_protocol::protocol::SessionSource;

pub(super) fn usage_hint_text<'a>(
    turn_context: &'a TurnContext,
    session_source: &SessionSource,
) -> Option<&'a str> {
    if !turn_context.features.enabled(Feature::MultiAgentV2) {
        return None;
    }

    let multi_agent_v2 = &turn_context.config.multi_agent_v2;
    match session_source {
        SessionSource::SubAgent(_) => multi_agent_v2.subagent_usage_hint_text.as_deref(),
        _ => multi_agent_v2.root_agent_usage_hint_text.as_deref(),
    }
}
