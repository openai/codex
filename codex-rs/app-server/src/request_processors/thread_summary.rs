use super::*;

pub(crate) async fn read_summary_from_rollout(
    path: &Path,
    fallback_provider: &str,
) -> std::io::Result<ConversationSummary> {
    let head = read_head_for_summary(path).await?;

    let Some(first) = head.first() else {
        return Err(IoError::other(format!(
            "rollout at {} is empty",
            path.display()
        )));
    };

    let session_meta_line =
        serde_json::from_value::<SessionMetaLine>(first.clone()).map_err(|_| {
            IoError::other(format!(
                "rollout at {} does not start with session metadata",
                path.display()
            ))
        })?;
    let SessionMetaLine {
        meta: session_meta,
        git,
    } = session_meta_line;
    let mut session_meta = session_meta;
    session_meta.source = with_thread_spawn_agent_metadata(
        session_meta.source.clone(),
        session_meta.agent_nickname.clone(),
        session_meta.agent_role.clone(),
    );

    let created_at = if session_meta.timestamp.is_empty() {
        None
    } else {
        Some(session_meta.timestamp.as_str())
    };
    let updated_at = read_updated_at(path, created_at).await;
    if let Some(summary) = extract_conversation_summary(
        path.to_path_buf(),
        &head,
        &session_meta,
        git.as_ref(),
        fallback_provider,
        updated_at.clone(),
    ) {
        return Ok(summary);
    }

    let timestamp = if session_meta.timestamp.is_empty() {
        None
    } else {
        Some(session_meta.timestamp.clone())
    };
    let model_provider = session_meta
        .model_provider
        .clone()
        .unwrap_or_else(|| fallback_provider.to_string());
    let git_info = git.as_ref().map(map_git_info);
    let updated_at = updated_at.or_else(|| timestamp.clone());

    Ok(ConversationSummary {
        conversation_id: session_meta.id,
        timestamp,
        updated_at,
        path: path.to_path_buf(),
        preview: String::new(),
        model_provider,
        cwd: session_meta.cwd,
        cli_version: session_meta.cli_version,
        source: session_meta.source,
        git_info,
    })
}

pub(crate) async fn read_rollout_items_from_rollout(
    path: &Path,
) -> std::io::Result<Vec<RolloutItem>> {
    let items = match RolloutRecorder::get_rollout_history(path).await? {
        InitialHistory::New | InitialHistory::Cleared => Vec::new(),
        InitialHistory::Forked(items) => items,
        InitialHistory::Resumed(resumed) => resumed.history,
    };

    Ok(items)
}

fn extract_conversation_summary(
    path: PathBuf,
    head: &[serde_json::Value],
    session_meta: &SessionMeta,
    git: Option<&CoreGitInfo>,
    fallback_provider: &str,
    updated_at: Option<String>,
) -> Option<ConversationSummary> {
    let preview = head
        .iter()
        .filter_map(|value| serde_json::from_value::<ResponseItem>(value.clone()).ok())
        .find_map(|item| match codex_core::parse_turn_item(&item) {
            Some(TurnItem::UserMessage(user)) => Some(user.message()),
            _ => None,
        })?;

    let preview = match preview.find(USER_MESSAGE_BEGIN) {
        Some(idx) => preview[idx + USER_MESSAGE_BEGIN.len()..].trim(),
        None => preview.as_str(),
    };

    let timestamp = if session_meta.timestamp.is_empty() {
        None
    } else {
        Some(session_meta.timestamp.clone())
    };
    let conversation_id = session_meta.id;
    let model_provider = session_meta
        .model_provider
        .clone()
        .unwrap_or_else(|| fallback_provider.to_string());
    let git_info = git.map(map_git_info);
    let updated_at = updated_at.or_else(|| timestamp.clone());

    Some(ConversationSummary {
        conversation_id,
        timestamp,
        updated_at,
        path,
        preview: preview.to_string(),
        model_provider,
        cwd: session_meta.cwd.clone(),
        cli_version: session_meta.cli_version.clone(),
        source: session_meta.source.clone(),
        git_info,
    })
}

fn map_git_info(git_info: &CoreGitInfo) -> ConversationGitInfo {
    ConversationGitInfo {
        sha: git_info.commit_hash.as_ref().map(|sha| sha.0.clone()),
        branch: git_info.branch.clone(),
        origin_url: git_info.repository_url.clone(),
    }
}

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

pub(super) fn thread_response_active_permission_profile(
    active_permission_profile: Option<codex_protocol::models::ActivePermissionProfile>,
) -> Option<codex_app_server_protocol::ActivePermissionProfile> {
    active_permission_profile.map(Into::into)
}

pub(super) fn apply_permission_profile_selection_to_config_overrides(
    overrides: &mut ConfigOverrides,
    permissions: Option<PermissionProfileSelectionParams>,
) {
    let Some(PermissionProfileSelectionParams::Profile { id, modifications }) = permissions else {
        return;
    };
    overrides.default_permissions = Some(id);
    overrides
        .additional_writable_roots
        .extend(modifications.unwrap_or_default().into_iter().map(
            |modification| match modification {
                PermissionProfileModificationParams::AdditionalWritableRoot { path } => {
                    path.to_path_buf()
                }
            },
        ));
}

pub(super) fn thread_response_sandbox_policy(
    permission_profile: &codex_protocol::models::PermissionProfile,
    cwd: &Path,
) -> codex_app_server_protocol::SandboxPolicy {
    let file_system_policy = permission_profile.file_system_sandbox_policy();
    let sandbox_policy = codex_sandboxing::compatibility_sandbox_policy_for_permission_profile(
        permission_profile,
        &file_system_policy,
        permission_profile.network_sandbox_policy(),
        cwd,
    );
    sandbox_policy.into()
}

fn parse_datetime(timestamp: Option<&str>) -> Option<DateTime<Utc>> {
    timestamp.and_then(|ts| {
        chrono::DateTime::parse_from_rfc3339(ts)
            .ok()
            .map(|dt| dt.with_timezone(&chrono::Utc))
    })
}

async fn read_updated_at(path: &Path, created_at: Option<&str>) -> Option<String> {
    let updated_at = tokio::fs::metadata(path)
        .await
        .ok()
        .and_then(|meta| meta.modified().ok())
        .map(|modified| {
            let updated_at: DateTime<Utc> = modified.into();
            updated_at.to_rfc3339_opts(SecondsFormat::Millis, true)
        });
    updated_at.or_else(|| created_at.map(str::to_string))
}

pub(super) fn thread_started_notification(mut thread: Thread) -> ThreadStartedNotification {
    thread.turns.clear();
    ThreadStartedNotification { thread }
}

pub(crate) fn summary_to_thread(
    summary: ConversationSummary,
    fallback_cwd: &AbsolutePathBuf,
) -> Thread {
    let ConversationSummary {
        conversation_id,
        path,
        preview,
        timestamp,
        updated_at,
        model_provider,
        cwd,
        cli_version,
        source,
        git_info,
    } = summary;

    let created_at = parse_datetime(timestamp.as_deref());
    let updated_at = parse_datetime(updated_at.as_deref()).or(created_at);
    let git_info = git_info.map(|info| ApiGitInfo {
        sha: info.sha,
        branch: info.branch,
        origin_url: info.origin_url,
    });
    let cwd =
        AbsolutePathBuf::relative_to_current_dir(path_utils::normalize_for_native_workdir(cwd))
            .unwrap_or_else(|err| {
                warn!(
                    path = %path.display(),
                    "failed to normalize thread cwd while summarizing thread: {err}"
                );
                fallback_cwd.clone()
            });

    Thread {
        id: conversation_id.to_string(),
        forked_from_id: None,
        preview,
        ephemeral: false,
        model_provider,
        created_at: created_at.map(|dt| dt.timestamp()).unwrap_or(0),
        updated_at: updated_at.map(|dt| dt.timestamp()).unwrap_or(0),
        status: ThreadStatus::NotLoaded,
        path: Some(path),
        cwd,
        cli_version,
        agent_nickname: source.get_nickname(),
        agent_role: source.get_agent_role(),
        source: source.into(),
        git_info,
        name: None,
        turns: Vec::new(),
    }
}

#[cfg(test)]
#[path = "thread_summary_tests.rs"]
mod thread_summary_tests;
