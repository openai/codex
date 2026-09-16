use crate::config::MultiAgentV2Config;
use crate::context::MultiAgentRoleInstructions;
use crate::session::step_context::StepContext;
use codex_prompts::ResolvedMessage;
use codex_prompts::ResolvedModelMessages;
use codex_prompts::ResolvedMultiAgentMessages;
use codex_protocol::config_types::MultiAgentMode;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;

#[derive(Clone, Debug, Default)]
pub(crate) struct ResolvedMultiAgentV2UsageHints {
    pub(crate) root: Option<MultiAgentRoleInstructions>,
    pub(crate) subagent: Option<MultiAgentRoleInstructions>,
}

pub(super) fn usage_hint_text(
    step_context: &StepContext,
    session_source: &SessionSource,
) -> Option<MultiAgentRoleInstructions> {
    let turn_context = step_context.turn.as_ref();
    if turn_context.multi_agent_version != MultiAgentVersion::V2 {
        return None;
    }

    let multi_agent_messages =
        ResolvedModelMessages::from_model(&step_context.settings.model_info).multi_agent();
    let snapshot = resolve_usage_hints(
        &turn_context.config.multi_agent_v2,
        multi_agent_messages,
        !turn_context.config.update_plan_enabled && turn_context.config.model_catalog.is_none(),
    );
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
    multi_agent_messages: ResolvedMultiAgentMessages<'_>,
    omit_update_plan_instructions: bool,
) -> ResolvedMultiAgentV2UsageHints {
    let resolve_role = |configured: Option<&str>, message: ResolvedMessage<'_>| {
        // Configured roles take precedence; empty configured or catalog roles suppress fallback.
        if let Some(configured) = configured {
            return (!configured.is_empty())
                .then(|| MultiAgentRoleInstructions::Configured(configured.to_owned()));
        }

        let base = message.text();
        if base.is_empty() {
            return None;
        }
        Some(MultiAgentRoleInstructions::Composed {
            base: base.to_owned(),
            marked: message.catalog_override().is_some(),
            omit_update_plan_instructions,
            max_concurrency: config.max_concurrent_threads_per_session,
            wait_agent_enabled: config.wait_agent_enabled,
            expose_model_overrides: config.expose_spawn_agent_model_overrides,
        })
    };

    ResolvedMultiAgentV2UsageHints {
        root: resolve_role(
            config.root_agent_usage_hint_text.as_deref(),
            multi_agent_messages.root,
        ),
        subagent: resolve_role(
            config.subagent_usage_hint_text.as_deref(),
            multi_agent_messages.subagent,
        ),
    }
}

pub(crate) fn effective_multi_agent_mode(step_context: &StepContext) -> Option<MultiAgentMode> {
    let turn_context = step_context.turn.as_ref();
    let settings = &step_context.settings;
    if turn_context.multi_agent_version != MultiAgentVersion::V2 {
        return None;
    }

    let multi_agent_messages =
        ResolvedModelMessages::from_model(&settings.model_info).multi_agent();
    let hint = turn_context
        .config
        .multi_agent_v2
        .multi_agent_mode_hint_text
        .as_deref()
        .or(multi_agent_messages.hint);
    let multi_agent_mode = match hint {
        Some(text) => MultiAgentMode::Custom(text.to_owned()),
        None => {
            let (message, builtin) =
                if settings.effective_reasoning_effort() == Some(ReasoningEffort::Ultra) {
                    (multi_agent_messages.proactive, MultiAgentMode::Proactive)
                } else {
                    (
                        multi_agent_messages.explicit,
                        MultiAgentMode::ExplicitRequestOnly,
                    )
                };
            match message {
                ResolvedMessage::Catalog(text) => MultiAgentMode::Custom(text.to_owned()),
                ResolvedMessage::Bundled(_) => builtin,
            }
        }
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
