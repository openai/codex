use std::path::PathBuf;
use std::str::FromStr;

use chrono::DateTime;
use chrono::Utc;
use codex_git_utils::GitSha;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::BaseInstructions;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GitInfo;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadMemoryMode;

use super::proto;
use crate::GitInfoPatch;
use crate::OptionalStringPatch;
use crate::SortDirection;
use crate::StoredThread;
use crate::StoredThreadHistory;
use crate::ThreadEventPersistenceMode;
use crate::ThreadMetadataPatch;
use crate::ThreadPersistenceMetadata;
use crate::ThreadSortKey;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

pub(super) fn remote_status_to_error(status: tonic::Status) -> ThreadStoreError {
    match status.code() {
        tonic::Code::InvalidArgument => ThreadStoreError::InvalidRequest {
            message: status.message().to_string(),
        },
        tonic::Code::AlreadyExists | tonic::Code::FailedPrecondition | tonic::Code::Aborted => {
            ThreadStoreError::Conflict {
                message: status.message().to_string(),
            }
        }
        _ => ThreadStoreError::Internal {
            message: format!("remote thread store request failed: {status}"),
        },
    }
}

pub(super) fn remote_status_to_thread_error(
    status: tonic::Status,
    thread_id: ThreadId,
) -> ThreadStoreError {
    if status.code() == tonic::Code::NotFound {
        return ThreadStoreError::ThreadNotFound { thread_id };
    }
    remote_status_to_error(status)
}

pub(super) fn proto_thread_id_request(thread_id: ThreadId) -> proto::ThreadIdRequest {
    proto::ThreadIdRequest {
        thread_id: thread_id.to_string(),
    }
}

pub(super) fn proto_sort_key(sort_key: ThreadSortKey) -> proto::ThreadSortKey {
    match sort_key {
        ThreadSortKey::CreatedAt => proto::ThreadSortKey::CreatedAt,
        ThreadSortKey::UpdatedAt => proto::ThreadSortKey::UpdatedAt,
    }
}

pub(super) fn proto_sort_direction(sort_direction: SortDirection) -> proto::SortDirection {
    match sort_direction {
        SortDirection::Asc => proto::SortDirection::Asc,
        SortDirection::Desc => proto::SortDirection::Desc,
    }
}

pub(super) fn proto_event_persistence_mode(
    mode: ThreadEventPersistenceMode,
) -> proto::ThreadEventPersistenceMode {
    match mode {
        ThreadEventPersistenceMode::Limited => proto::ThreadEventPersistenceMode::Limited,
        ThreadEventPersistenceMode::Extended => proto::ThreadEventPersistenceMode::Extended,
    }
}

pub(super) fn proto_session_source(source: &SessionSource) -> proto::SessionSource {
    match source {
        SessionSource::Cli => proto_source(proto::SessionSourceKind::Cli),
        SessionSource::VSCode => proto_source(proto::SessionSourceKind::Vscode),
        SessionSource::Exec => proto_source(proto::SessionSourceKind::Exec),
        SessionSource::Mcp => proto_source(proto::SessionSourceKind::AppServer),
        SessionSource::Custom(custom) => proto::SessionSource {
            kind: proto::SessionSourceKind::Custom.into(),
            custom: Some(custom.clone()),
            ..Default::default()
        },
        SessionSource::SubAgent(SubAgentSource::Review) => {
            proto_source(proto::SessionSourceKind::SubAgentReview)
        }
        SessionSource::SubAgent(SubAgentSource::Compact) => {
            proto_source(proto::SessionSourceKind::SubAgentCompact)
        }
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            depth,
            agent_path,
            agent_nickname,
            agent_role,
        }) => proto::SessionSource {
            kind: proto::SessionSourceKind::SubAgentThreadSpawn.into(),
            sub_agent_parent_thread_id: Some(parent_thread_id.to_string()),
            sub_agent_depth: Some(*depth),
            sub_agent_path: agent_path.as_ref().map(|path| path.as_str().to_string()),
            sub_agent_nickname: agent_nickname.clone(),
            sub_agent_role: agent_role.clone(),
            ..Default::default()
        },
        SessionSource::SubAgent(SubAgentSource::MemoryConsolidation) => {
            proto_source(proto::SessionSourceKind::SubAgentMemoryConsolidation)
        }
        SessionSource::SubAgent(SubAgentSource::Other(other)) => proto::SessionSource {
            kind: proto::SessionSourceKind::SubAgentOther.into(),
            sub_agent_other: Some(other.clone()),
            ..Default::default()
        },
        SessionSource::Internal(_) => proto_source(proto::SessionSourceKind::Unknown),
        SessionSource::Unknown => proto_source(proto::SessionSourceKind::Unknown),
    }
}

fn proto_source(kind: proto::SessionSourceKind) -> proto::SessionSource {
    proto::SessionSource {
        kind: kind.into(),
        ..Default::default()
    }
}

pub(super) fn serialize_json<T: serde::Serialize>(
    value: &T,
    field_name: &str,
) -> ThreadStoreResult<String> {
    serde_json::to_string(value).map_err(|err| ThreadStoreError::InvalidRequest {
        message: format!("failed to serialize {field_name} for remote thread store: {err}"),
    })
}

fn deserialize_json<T: serde::de::DeserializeOwned>(
    json: &str,
    field_name: &str,
) -> ThreadStoreResult<T> {
    serde_json::from_str(json).map_err(|err| ThreadStoreError::InvalidRequest {
        message: format!("remote thread store returned invalid {field_name}: {err}"),
    })
}

pub(super) fn serialize_json_vec<T: serde::Serialize>(
    values: &[T],
    field_name: &str,
) -> ThreadStoreResult<Vec<String>> {
    values
        .iter()
        .map(|value| serialize_json(value, field_name))
        .collect()
}

fn deserialize_json_vec<T: serde::de::DeserializeOwned>(
    values: &[String],
    field_name: &str,
) -> ThreadStoreResult<Vec<T>> {
    values
        .iter()
        .map(|value| deserialize_json(value, field_name))
        .collect()
}

pub(super) fn base_instructions_json(
    base_instructions: &BaseInstructions,
) -> ThreadStoreResult<String> {
    serialize_json(base_instructions, "base_instructions")
}

pub(super) fn dynamic_tools_json(
    dynamic_tools: &[DynamicToolSpec],
) -> ThreadStoreResult<Vec<String>> {
    serialize_json_vec(dynamic_tools, "dynamic_tool")
}

pub(super) fn thread_persistence_metadata_json(
    metadata: &ThreadPersistenceMetadata,
) -> ThreadStoreResult<String> {
    serialize_json(metadata, "thread_persistence_metadata")
}

pub(super) fn rollout_items_json(items: &[RolloutItem]) -> ThreadStoreResult<Vec<String>> {
    serialize_json_vec(items, "rollout_item")
}

pub(super) fn stored_thread_history_from_proto(
    history: proto::StoredThreadHistory,
) -> ThreadStoreResult<StoredThreadHistory> {
    let thread_id = ThreadId::from_string(&history.thread_id).map_err(|err| {
        ThreadStoreError::InvalidRequest {
            message: format!("remote thread store returned invalid history thread_id: {err}"),
        }
    })?;
    Ok(StoredThreadHistory {
        thread_id,
        items: deserialize_json_vec(&history.items_json, "rollout_item")?,
    })
}

pub(super) fn proto_metadata_patch(patch: ThreadMetadataPatch) -> proto::ThreadMetadataPatch {
    proto::ThreadMetadataPatch {
        name: patch.name,
        memory_mode: patch.memory_mode.map(proto_memory_mode).map(Into::into),
        git_info: patch.git_info.map(proto_git_info_patch),
    }
}

fn proto_memory_mode(memory_mode: ThreadMemoryMode) -> proto::ThreadMemoryMode {
    match memory_mode {
        ThreadMemoryMode::Enabled => proto::ThreadMemoryMode::Enabled,
        ThreadMemoryMode::Disabled => proto::ThreadMemoryMode::Disabled,
    }
}

fn proto_git_info_patch(patch: GitInfoPatch) -> proto::GitInfoPatch {
    proto::GitInfoPatch {
        sha: Some(proto_optional_string_patch(patch.sha)),
        branch: Some(proto_optional_string_patch(patch.branch)),
        origin_url: Some(proto_optional_string_patch(patch.origin_url)),
    }
}

fn proto_optional_string_patch(patch: OptionalStringPatch) -> proto::OptionalStringPatch {
    match patch {
        None => proto::OptionalStringPatch {
            kind: proto::OptionalStringPatchKind::Unset.into(),
            value: None,
        },
        Some(None) => proto::OptionalStringPatch {
            kind: proto::OptionalStringPatchKind::Clear.into(),
            value: None,
        },
        Some(Some(value)) => proto::OptionalStringPatch {
            kind: proto::OptionalStringPatchKind::Set.into(),
            value: Some(value),
        },
    }
}

pub(super) fn stored_thread_from_proto(
    thread: proto::StoredThread,
) -> ThreadStoreResult<StoredThread> {
    // Keep this mapping boring: the proto mirrors StoredThread for remote-readable
    // summary fields, except for Rust domain types that cross gRPC as stable scalar
    // values. Local-only fields such as rollout_path intentionally stay local.
    let source = thread
        .source
        .as_ref()
        .map(session_source_from_proto)
        .transpose()?
        .unwrap_or(SessionSource::Unknown);
    let thread_id = ThreadId::from_string(&thread.thread_id).map_err(|err| {
        ThreadStoreError::InvalidRequest {
            message: format!("remote thread store returned invalid thread_id: {err}"),
        }
    })?;
    let forked_from_id = thread
        .forked_from_id
        .as_deref()
        .map(ThreadId::from_string)
        .transpose()
        .map_err(|err| ThreadStoreError::InvalidRequest {
            message: format!("remote thread store returned invalid forked_from_id: {err}"),
        })?;

    Ok(StoredThread {
        thread_id,
        rollout_path: thread.rollout_path.map(PathBuf::from),
        forked_from_id,
        preview: thread.preview,
        name: thread.name,
        model_provider: thread.model_provider,
        model: thread.model,
        reasoning_effort: thread
            .reasoning_effort
            .as_deref()
            .map(parse_reasoning_effort)
            .transpose()?,
        created_at: datetime_from_unix(thread.created_at)?,
        updated_at: datetime_from_unix(thread.updated_at)?,
        archived_at: thread.archived_at.map(datetime_from_unix).transpose()?,
        cwd: PathBuf::from(thread.cwd),
        cli_version: thread.cli_version,
        source,
        agent_nickname: thread.agent_nickname,
        agent_role: thread.agent_role,
        agent_path: thread.agent_path,
        git_info: thread.git_info.map(git_info_from_proto),
        approval_mode: thread
            .approval_mode_json
            .as_deref()
            .map(|json| deserialize_json(json, "approval_mode"))
            .transpose()?
            .unwrap_or(AskForApproval::OnRequest),
        sandbox_policy: thread
            .sandbox_policy_json
            .as_deref()
            .map(|json| deserialize_json(json, "sandbox_policy"))
            .transpose()?
            .unwrap_or_else(SandboxPolicy::new_read_only_policy),
        token_usage: thread
            .token_usage_json
            .as_deref()
            .map(|json| deserialize_json(json, "token_usage"))
            .transpose()?,
        first_user_message: thread.first_user_message,
        history: thread
            .history
            .map(stored_thread_history_from_proto)
            .transpose()?,
    })
}

#[cfg(test)]
pub(super) fn stored_thread_to_proto(thread: StoredThread) -> proto::StoredThread {
    proto::StoredThread {
        thread_id: thread.thread_id.to_string(),
        forked_from_id: thread.forked_from_id.map(|thread_id| thread_id.to_string()),
        preview: thread.preview,
        name: thread.name,
        model_provider: thread.model_provider,
        model: thread.model,
        created_at: thread.created_at.timestamp(),
        updated_at: thread.updated_at.timestamp(),
        archived_at: thread.archived_at.map(|timestamp| timestamp.timestamp()),
        cwd: thread.cwd.to_string_lossy().into_owned(),
        cli_version: thread.cli_version,
        source: Some(proto_session_source(&thread.source)),
        git_info: thread.git_info.map(git_info_to_proto),
        agent_nickname: thread.agent_nickname,
        agent_role: thread.agent_role,
        agent_path: thread.agent_path,
        reasoning_effort: thread.reasoning_effort.map(|effort| effort.to_string()),
        first_user_message: thread.first_user_message,
        rollout_path: thread
            .rollout_path
            .map(|path| path.to_string_lossy().into_owned()),
        approval_mode_json: Some(serialize_json(&thread.approval_mode, "approval_mode").unwrap()),
        sandbox_policy_json: Some(
            serialize_json(&thread.sandbox_policy, "sandbox_policy").unwrap(),
        ),
        token_usage_json: thread
            .token_usage
            .as_ref()
            .map(|usage| serialize_json(usage, "token_usage").unwrap()),
        history: thread.history.map(stored_thread_history_to_proto),
    }
}

#[cfg(test)]
fn stored_thread_history_to_proto(history: StoredThreadHistory) -> proto::StoredThreadHistory {
    proto::StoredThreadHistory {
        thread_id: history.thread_id.to_string(),
        items_json: rollout_items_json(&history.items).unwrap(),
    }
}

fn datetime_from_unix(timestamp: i64) -> ThreadStoreResult<DateTime<Utc>> {
    DateTime::from_timestamp(timestamp, 0).ok_or_else(|| ThreadStoreError::InvalidRequest {
        message: format!("remote thread store returned invalid timestamp: {timestamp}"),
    })
}

fn session_source_from_proto(source: &proto::SessionSource) -> ThreadStoreResult<SessionSource> {
    let kind = proto::SessionSourceKind::try_from(source.kind).unwrap_or_default();
    Ok(match kind {
        proto::SessionSourceKind::Unknown => SessionSource::Unknown,
        proto::SessionSourceKind::Cli => SessionSource::Cli,
        proto::SessionSourceKind::Vscode => SessionSource::VSCode,
        proto::SessionSourceKind::Exec => SessionSource::Exec,
        proto::SessionSourceKind::AppServer => SessionSource::Mcp,
        proto::SessionSourceKind::Custom => {
            SessionSource::Custom(source.custom.clone().unwrap_or_default())
        }
        proto::SessionSourceKind::SubAgentReview => SessionSource::SubAgent(SubAgentSource::Review),
        proto::SessionSourceKind::SubAgentCompact => {
            SessionSource::SubAgent(SubAgentSource::Compact)
        }
        proto::SessionSourceKind::SubAgentThreadSpawn => {
            let parent_thread_id = source
                .sub_agent_parent_thread_id
                .as_deref()
                .map(ThreadId::from_string)
                .transpose()
                .map_err(|err| ThreadStoreError::InvalidRequest {
                    message: format!(
                        "remote thread store returned invalid sub-agent parent thread id: {err}"
                    ),
                })?
                .ok_or_else(|| ThreadStoreError::InvalidRequest {
                    message: "remote thread store omitted sub-agent parent thread id".to_string(),
                })?;
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth: source.sub_agent_depth.unwrap_or_default(),
                agent_path: source
                    .sub_agent_path
                    .clone()
                    .map(AgentPath::from_string)
                    .transpose()
                    .map_err(|message| ThreadStoreError::InvalidRequest { message })?,
                agent_nickname: source.sub_agent_nickname.clone(),
                agent_role: source.sub_agent_role.clone(),
            })
        }
        proto::SessionSourceKind::SubAgentMemoryConsolidation => {
            SessionSource::SubAgent(SubAgentSource::MemoryConsolidation)
        }
        proto::SessionSourceKind::SubAgentOther => SessionSource::SubAgent(SubAgentSource::Other(
            source.sub_agent_other.clone().unwrap_or_default(),
        )),
    })
}

fn git_info_from_proto(info: proto::GitInfo) -> GitInfo {
    GitInfo {
        commit_hash: info.sha.as_deref().map(GitSha::new),
        branch: info.branch,
        repository_url: info.origin_url,
    }
}

#[cfg(test)]
fn git_info_to_proto(info: GitInfo) -> proto::GitInfo {
    proto::GitInfo {
        sha: info.commit_hash.map(|sha| sha.0),
        branch: info.branch,
        origin_url: info.repository_url,
    }
}

fn parse_reasoning_effort(value: &str) -> ThreadStoreResult<ReasoningEffort> {
    ReasoningEffort::from_str(value).map_err(|message| ThreadStoreError::InvalidRequest {
        message: format!("remote thread store returned {message}"),
    })
}
