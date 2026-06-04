use std::collections::HashMap;
use std::sync::Arc;

use anyhow::anyhow;
use codex_features::Feature;
use codex_hooks::AgentHookRequest;
use codex_hooks::AgentHookRunner;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;

use crate::codex_delegate::run_codex_thread_one_shot;
use crate::config::Config;
use crate::config::Constrained;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;

pub(crate) const AGENT_HOOK_MAX_MODEL_REQUESTS: usize = 50;
const AGENT_HOOK_SOURCE_NAME: &str = "agent_hook";
const AGENT_HOOK_BASE_INSTRUCTIONS: &str = r#"You evaluate a Codex agent hook.

Use the available tools when needed to investigate whether the hook input satisfies the hook author's instructions.

Return only JSON:
{"ok": true}
or
{"ok": false, "reason": "concise actionable reason"}

Use ok:false only when the hook criteria fail. Do not answer the user's task. Do not include Markdown or extra text."#;

pub(crate) fn is_agent_hook_source(session_source: &SessionSource) -> bool {
    matches!(session_source, SessionSource::SubAgent(source) if is_agent_hook_subagent_source(source))
}

pub(crate) fn is_agent_hook_subagent_source(source: &SubAgentSource) -> bool {
    matches!(source, SubAgentSource::Other(name) if name == AGENT_HOOK_SOURCE_NAME)
}

pub(crate) fn build_agent_hook_runner(
    parent_session: Arc<Session>,
    parent_turn: Arc<TurnContext>,
) -> AgentHookRunner {
    AgentHookRunner::new(move |request| {
        let parent_session = Arc::clone(&parent_session);
        let parent_turn = Arc::clone(&parent_turn);
        async move { run_agent_hook(parent_session, parent_turn, request).await }
    })
}

async fn run_agent_hook(
    parent_session: Arc<Session>,
    parent_turn: Arc<TurnContext>,
    request: AgentHookRequest,
) -> anyhow::Result<String> {
    let config =
        build_agent_hook_config(parent_turn.config.as_ref(), &parent_turn, &request.model)?;
    let cancellation_token = CancellationToken::new();
    let _cancel_on_drop = cancellation_token.clone().drop_guard();
    let codex = run_codex_thread_one_shot(
        config,
        parent_session.services.auth_manager.clone(),
        Arc::clone(&parent_session.services.models_manager),
        vec![UserInput::Text {
            text: request.prompt,
            text_elements: Vec::new(),
        }],
        parent_session,
        parent_turn,
        cancellation_token,
        SubAgentSource::Other(AGENT_HOOK_SOURCE_NAME.to_string()),
        Some(crate::hook_prompt::hook_output_schema()),
        /*initial_history*/ None,
    )
    .await?;

    let mut last_error = None;
    loop {
        match codex.next_event().await?.msg {
            EventMsg::TurnComplete(event) => {
                return event.last_agent_message.ok_or_else(|| {
                    anyhow!(last_error.unwrap_or_else(|| {
                        "agent hook completed without a final response".to_string()
                    }))
                });
            }
            EventMsg::TurnAborted(_) => return Err(anyhow!("agent hook was aborted")),
            EventMsg::Error(event) => last_error = Some(event.message),
            _ => {}
        }
    }
}

fn build_agent_hook_config(
    parent_config: &Config,
    parent_turn: &TurnContext,
    model: &str,
) -> anyhow::Result<Config> {
    let mut config = parent_config.clone();
    config.model = Some(model.to_string());
    config.base_instructions = Some(AGENT_HOOK_BASE_INSTRUCTIONS.to_string());
    config.user_instructions = None;
    config.developer_instructions = None;
    config.compact_prompt = None;
    config.guardian_policy_config = None;
    config.personality = None;
    config.project_doc_max_bytes = 0;
    config.include_permissions_instructions = false;
    config.include_apps_instructions = false;
    config.include_collaboration_mode_instructions = false;
    config.include_skill_instructions = false;
    config.include_environment_context = false;
    config.experimental_request_user_input_enabled = false;
    config.notify = None;
    config.ephemeral = true;
    config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);
    config
        .permissions
        .set_permission_profile(parent_turn.permission_profile())
        .map_err(|err| {
            anyhow!("agent hook could not inherit the active permission profile: {err}")
        })?;
    config.mcp_servers = Constrained::allow_only(HashMap::new());
    config.web_search_mode = Constrained::allow_only(WebSearchMode::Disabled);
    config.features = config
        .features
        .clone_with_features_disabled_for_internal_session([
            Feature::CodexHooks,
            Feature::MemoryTool,
            Feature::Chronicle,
            Feature::ChildAgentsMd,
            Feature::Collab,
            Feature::MultiAgentV2,
            Feature::SpawnCsv,
            Feature::Apps,
            Feature::EnableMcpApps,
            Feature::ToolSuggest,
            Feature::Plugins,
            Feature::BrowserUse,
            Feature::BrowserUseExternal,
            Feature::ComputerUse,
            Feature::RemotePlugin,
            Feature::PluginSharing,
            Feature::WebSearchRequest,
            Feature::WebSearchCached,
            Feature::StandaloneWebSearch,
            Feature::ImageGeneration,
            Feature::ImageGenExt,
            Feature::SkillMcpDependencyInstall,
            Feature::RequestPermissionsTool,
            Feature::ExecPermissionApprovals,
            Feature::DefaultModeRequestUserInput,
            Feature::GuardianApproval,
            Feature::Goals,
            Feature::ToolCallMcpElicitation,
            Feature::AuthElicitation,
            Feature::Artifact,
            Feature::WorkspaceDependencies,
        ]);
    Ok(config)
}

#[cfg(test)]
#[path = "hook_agent_tests.rs"]
mod tests;
