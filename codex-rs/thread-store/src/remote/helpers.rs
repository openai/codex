use std::path::PathBuf;
use std::str::FromStr;

use chrono::DateTime;
use chrono::Utc;
use codex_git_utils::GitSha;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GitInfo;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;

use super::proto;
use crate::StoredThread;
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

pub(super) fn proto_sort_key(sort_key: ThreadSortKey) -> proto::ThreadSortKey {
    match sort_key {
        ThreadSortKey::CreatedAt => proto::ThreadSortKey::CreatedAt,
        ThreadSortKey::UpdatedAt => proto::ThreadSortKey::UpdatedAt,
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
        SessionSource::Unknown => proto_source(proto::SessionSourceKind::Unknown),
    }
}

fn proto_source(kind: proto::SessionSourceKind) -> proto::SessionSource {
    proto::SessionSource {
        kind: kind.into(),
        ..Default::default()
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
        rollout_path: None,
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
        approval_mode: AskForApproval::OnRequest,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        token_usage: None,
        first_user_message: thread.first_user_message,
        history: None,
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
