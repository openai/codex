use super::*;
use codex_protocol::config_types::MultiAgentMode;
use codex_utils_path_uri::LegacyAppPathString;

pub(super) fn with_thread_spawn_agent_metadata(
    source: codex_protocol::protocol::SessionSource,
    agent_nickname: Option<String>,
    agent_role: Option<String>,
) -> codex_protocol::protocol::SessionSource {
    if agent_nickname.is_none() && agent_role.is_none() {
        return source;
    }

    match source {
        codex_protocol::protocol::SessionSource::SubAgent(
            codex_protocol::protocol::SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth,
                agent_path,
                agent_nickname: existing_agent_nickname,
                agent_role: existing_agent_role,
            },
        ) => codex_protocol::protocol::SessionSource::SubAgent(
            codex_protocol::protocol::SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth,
                agent_path,
                agent_nickname: agent_nickname.or(existing_agent_nickname),
                agent_role: agent_role.or(existing_agent_role),
            },
        ),
        _ => source,
    }
}

pub(crate) fn thread_response_active_permission_profile(
    active_permission_profile: Option<codex_protocol::models::ActivePermissionProfile>,
) -> Option<codex_app_server_protocol::ActivePermissionProfile> {
    active_permission_profile.map(Into::into)
}

pub(super) fn thread_cwd_for_api(
    cwd: &AbsolutePathBuf,
) -> Result<LegacyAppPathString, JSONRPCErrorError> {
    cwd.to_str()
        .map(LegacyAppPathString::from_string)
        .ok_or_else(|| internal_error("cannot encode thread cwd: path contains invalid UTF-8"))
}

pub(crate) fn thread_settings_from_config_snapshot(
    config_snapshot: &ThreadConfigSnapshot,
) -> Result<ThreadSettings, JSONRPCErrorError> {
    Ok(ThreadSettings {
        disabled_plugin_ids: config_snapshot.disabled_plugin_ids.clone(),
        cwd: thread_cwd_for_api(config_snapshot.cwd())?,
        environments: Some(
            config_snapshot
                .environment_selections()
                .iter()
                .map(Into::into)
                .collect(),
        ),
        approval_policy: config_snapshot.approval_policy.into(),
        approvals_reviewer: config_snapshot.approvals_reviewer.into(),
        sandbox_policy: config_snapshot.sandbox_policy().into(),
        active_permission_profile: thread_response_active_permission_profile(
            config_snapshot.active_permission_profile.clone(),
        ),
        model: config_snapshot.model.clone(),
        model_provider: config_snapshot.model_provider_id.clone(),
        service_tier: config_snapshot.service_tier.clone(),
        effort: config_snapshot.reasoning_effort.clone(),
        summary: config_snapshot.reasoning_summary,
        collaboration_mode: config_snapshot.collaboration_mode.clone(),
        multi_agent_mode: MultiAgentMode::ExplicitRequestOnly,
        personality: config_snapshot.personality,
    })
}

pub(super) fn thread_started_notification(mut thread: Thread) -> ThreadStartedNotification {
    thread.turns.clear();
    ThreadStartedNotification { thread }
}
