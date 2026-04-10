use std::ffi::OsStr;
use std::fmt;
use std::path::Path;
use std::str::FromStr;

use chrono::DateTime;
use chrono::Utc;
use codex_git_utils::GitSha;
use codex_protocol::ThreadId;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GitInfo;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TokenUsage;
use codex_rollout::Cursor as RolloutCursor;
use codex_rollout::EventPersistenceMode;
use codex_rollout::parse_cursor;
use codex_state::DirectionalThreadSpawnEdgeStatus;
use codex_state::ThreadMetadata;

use crate::StoredThread;
use crate::StoredThreadHistory;
use crate::ThreadEventPersistenceMode;
use crate::ThreadSortKey;
use crate::ThreadSpawnEdgeStatus;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

impl From<ThreadEventPersistenceMode> for EventPersistenceMode {
    fn from(value: ThreadEventPersistenceMode) -> Self {
        match value {
            ThreadEventPersistenceMode::Limited => Self::Limited,
            ThreadEventPersistenceMode::Extended => Self::Extended,
        }
    }
}

pub(crate) fn metadata_from_items(
    path: &Path,
    items: &[RolloutItem],
    default_provider: &str,
    archived: bool,
) -> ThreadStoreResult<ThreadMetadata> {
    let Some(builder) = codex_rollout::builder_from_items(items, path) else {
        return Err(ThreadStoreError::InvalidRequest {
            message: format!("rollout {} is missing session metadata", path.display()),
        });
    };
    let mut metadata = builder.build(default_provider);
    for item in items {
        codex_state::apply_rollout_item(&mut metadata, item, default_provider);
    }
    if let Some(updated_at) = file_modified_time_utc(path) {
        metadata.updated_at = updated_at;
        if archived && metadata.archived_at.is_none() {
            metadata.archived_at = Some(updated_at);
        }
    }
    Ok(metadata)
}

pub(crate) fn stored_thread_from_metadata(
    metadata: ThreadMetadata,
    name: Option<String>,
    memory_mode: Option<String>,
    history: Option<StoredThreadHistory>,
) -> StoredThread {
    let thread_id = metadata.id;
    let git_info = git_info_from_metadata(&metadata);
    StoredThread {
        thread_id,
        forked_from_id: None,
        owner: Default::default(),
        preview: metadata.title.clone(),
        name,
        model_provider: metadata.model_provider,
        model: metadata.model,
        service_tier: None,
        reasoning_effort: metadata.reasoning_effort,
        created_at: metadata.created_at,
        updated_at: metadata.updated_at,
        archived_at: metadata.archived_at,
        cwd: metadata.cwd,
        cli_version: metadata.cli_version,
        source: parse_session_source(metadata.source.as_str()),
        agent_nickname: metadata.agent_nickname,
        agent_role: metadata.agent_role,
        agent_path: metadata.agent_path,
        git_info,
        approval_mode: parse_approval_mode(metadata.approval_mode.as_str()),
        sandbox_policy: parse_sandbox_policy(metadata.sandbox_policy.as_str()),
        token_usage: (metadata.tokens_used > 0).then_some(TokenUsage {
            total_tokens: metadata.tokens_used,
            ..TokenUsage::default()
        }),
        first_user_message: metadata.first_user_message,
        memory_mode,
        history,
    }
}

fn git_info_from_metadata(metadata: &ThreadMetadata) -> Option<GitInfo> {
    if metadata.git_sha.is_none()
        && metadata.git_branch.is_none()
        && metadata.git_origin_url.is_none()
    {
        return None;
    }
    Some(GitInfo {
        commit_hash: metadata.git_sha.as_deref().map(GitSha::new),
        branch: metadata.git_branch.clone(),
        repository_url: metadata.git_origin_url.clone(),
    })
}

fn parse_session_source(source: &str) -> SessionSource {
    serde_json::from_str::<SessionSource>(source)
        .or_else(|_| serde_json::from_value(serde_json::Value::String(source.to_string())))
        .or_else(|_| {
            SessionSource::from_startup_arg(source)
                .map_err(|_| serde_json::Error::io(std::io::Error::other("invalid session source")))
        })
        .unwrap_or(SessionSource::Unknown)
}

fn parse_approval_mode(value: &str) -> AskForApproval {
    serde_json::from_str::<AskForApproval>(value)
        .or_else(|_| serde_json::from_value(serde_json::Value::String(value.to_string())))
        .unwrap_or(AskForApproval::OnRequest)
}

fn parse_sandbox_policy(value: &str) -> SandboxPolicy {
    SandboxPolicy::from_str(value).unwrap_or_else(|_| SandboxPolicy::new_read_only_policy())
}

pub(crate) fn memory_mode_from_items(items: &[RolloutItem]) -> Option<String> {
    items.iter().find_map(|item| match item {
        RolloutItem::SessionMeta(meta) => meta.meta.memory_mode.clone(),
        RolloutItem::ResponseItem(_)
        | RolloutItem::Compacted(_)
        | RolloutItem::TurnContext(_)
        | RolloutItem::EventMsg(_) => None,
    })
}

pub(crate) fn dynamic_tools_from_items(items: &[RolloutItem]) -> Option<Vec<DynamicToolSpec>> {
    items.iter().find_map(|item| match item {
        RolloutItem::SessionMeta(meta) => meta.meta.dynamic_tools.clone(),
        RolloutItem::ResponseItem(_)
        | RolloutItem::Compacted(_)
        | RolloutItem::TurnContext(_)
        | RolloutItem::EventMsg(_) => None,
    })
}

pub(crate) fn parse_cursor_param(cursor: Option<&str>) -> ThreadStoreResult<Option<RolloutCursor>> {
    cursor
        .map(|cursor| {
            parse_cursor(cursor).ok_or_else(|| ThreadStoreError::InvalidRequest {
                message: format!("invalid cursor: {cursor}"),
            })
        })
        .transpose()
}

pub(crate) fn serialize_cursor(
    cursor: Option<&RolloutCursor>,
) -> ThreadStoreResult<Option<String>> {
    cursor
        .map(|cursor| {
            serde_json::to_value(cursor)
                .ok()
                .and_then(|value| value.as_str().map(str::to_string))
                .ok_or_else(|| ThreadStoreError::Internal {
                    message: "failed to serialize rollout cursor".to_string(),
                })
        })
        .transpose()
}

pub(crate) fn rollout_sort_key(sort_key: ThreadSortKey) -> codex_rollout::ThreadSortKey {
    match sort_key {
        ThreadSortKey::CreatedAt => codex_rollout::ThreadSortKey::CreatedAt,
        ThreadSortKey::UpdatedAt => codex_rollout::ThreadSortKey::UpdatedAt,
    }
}

pub(crate) fn source_to_state_string(source: &SessionSource) -> String {
    match serde_json::to_value(source) {
        Ok(serde_json::Value::String(source)) => source,
        Ok(value) => value.to_string(),
        Err(_) => String::new(),
    }
}

pub(crate) fn edge_status_to_state(
    status: ThreadSpawnEdgeStatus,
) -> DirectionalThreadSpawnEdgeStatus {
    match status {
        ThreadSpawnEdgeStatus::Open => DirectionalThreadSpawnEdgeStatus::Open,
        ThreadSpawnEdgeStatus::Closed => DirectionalThreadSpawnEdgeStatus::Closed,
    }
}

pub(crate) fn checked_rollout_file_name(
    path: &Path,
    thread_id: ThreadId,
) -> ThreadStoreResult<std::ffi::OsString> {
    let Some(file_name) = path.file_name().map(OsStr::to_owned) else {
        return Err(ThreadStoreError::InvalidRequest {
            message: format!("rollout path `{}` missing file name", path.display()),
        });
    };
    let required_suffix = format!("{thread_id}.jsonl");
    if !file_name
        .to_string_lossy()
        .ends_with(required_suffix.as_str())
    {
        return Err(ThreadStoreError::InvalidRequest {
            message: format!(
                "rollout path `{}` does not match thread id {thread_id}",
                path.display()
            ),
        });
    }
    Ok(file_name)
}

fn file_modified_time_utc(path: &Path) -> Option<DateTime<Utc>> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(DateTime::<Utc>::from(modified))
}

pub(crate) fn io_error(err: std::io::Error) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: err.to_string(),
    }
}

pub(crate) fn display_error(err: impl fmt::Display) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: err.to_string(),
    }
}
