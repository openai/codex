//! Authoritative cloud thread settings returned by deliberate native activation.

use crate::Client;
use crate::Error;
use crate::MutationError;
use crate::activation_wire as wire;
use bytes::Bytes;
use prost::Message;
use serde::Serialize;
use std::fmt;

#[cfg(test)]
#[path = "activation_tests.rs"]
mod tests;

/// Resolved settings and metadata returned after loading a cloud thread.
#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadActivation {
    pub thread: ThreadSnapshot,
    pub collaboration_mode: Option<CollaborationMode>,
    pub model: String,
    pub model_provider: String,
    pub service_tier: Option<String>,
    pub cwd: String,
    pub instruction_sources: Vec<String>,
    pub runtime_workspace_roots: Vec<String>,
    pub approval_policy: ApprovalPolicy,
    pub approvals_reviewer: ApprovalsReviewer,
    pub sandbox: SandboxPolicy,
    pub active_permission_profile: Option<ActivePermissionProfile>,
    pub reasoning_effort: Option<String>,
    pub turns_backwards_cursor: Option<String>,
    pub items_backwards_cursor: Option<String>,
}

/// Effective collaboration mode and its backend-resolved settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "mode", content = "settings", rename_all = "lowercase")]
pub enum CollaborationMode {
    Plan(CollaborationSettings),
    Default(CollaborationSettings),
}

/// The resume protocol uses snake_case for these nested settings.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct CollaborationSettings {
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub developer_instructions: Option<String>,
}

/// Metadata needed to reconstruct a turn-free app-server thread snapshot.
#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSnapshot {
    pub id: String,
    pub session_id: String,
    pub forked_from_id: Option<String>,
    pub parent_thread_id: Option<String>,
    pub preview: String,
    pub ephemeral: bool,
    pub daybreak_enabled: Option<bool>,
    pub project_id: Option<String>,
    pub history_mode: ThreadHistoryMode,
    pub model_provider: String,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub recency_at: Option<i64>,
    pub status: ThreadStatus,
    pub path: Option<String>,
    pub cwd: String,
    pub cli_version: String,
    pub originator: Option<String>,
    pub source: SessionSource,
    pub can_accept_direct_input: Option<bool>,
    pub thread_source: Option<String>,
    pub agent_nickname: Option<String>,
    pub agent_role: Option<String>,
    pub name: Option<String>,
    pub environments: Option<Vec<ThreadEnvironment>>,
    /// Activation deliberately excludes turns; callers page history separately.
    pub turns: [(); 0],
    #[serde(skip)]
    raw_protobuf: Bytes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThreadHistoryMode {
    Legacy,
    Paginated,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadEnvironment {
    pub environment_id: String,
    pub cwd: String,
    pub runtime_workspace_roots: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionSource {
    Cli,
    #[serde(rename = "vscode")]
    VsCode,
    Exec,
    AppServer,
    Unknown,
    Custom(String),
    SubAgent(SubAgentSource),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SubAgentSource {
    Review,
    Compact,
    MemoryConsolidation,
    ThreadSpawn {
        parent_thread_id: String,
        depth: i32,
        agent_path: Option<String>,
        agent_nickname: Option<String>,
        agent_role: Option<String>,
    },
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ThreadStatus {
    NotLoaded,
    Idle,
    SystemError,
    Active { active_flags: Vec<ThreadActiveFlag> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ThreadActiveFlag {
    WaitingOnApproval,
    WaitingOnUserInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalPolicy {
    #[serde(rename = "untrusted")]
    UnlessTrusted,
    OnRequest,
    Never,
    Granular {
        sandbox_approval: bool,
        rules: bool,
        skill_approval: bool,
        request_permissions: bool,
        mcp_elicitations: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ApprovalsReviewer {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "auto_review")]
    AutoReview,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivePermissionProfile {
    pub id: String,
    pub extends: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NetworkAccess {
    Restricted,
    Enabled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SandboxPolicy {
    DangerFullAccess,
    ReadOnly {
        network_access: bool,
    },
    ExternalSandbox {
        network_access: NetworkAccess,
    },
    WorkspaceWrite {
        writable_roots: Vec<String>,
        network_access: bool,
        exclude_tmpdir_env_var: bool,
        exclude_slash_tmp: bool,
    },
}

impl fmt::Debug for CollaborationSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CollaborationSettings([redacted])")
    }
}
impl fmt::Debug for ThreadActivation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ThreadActivation([redacted])")
    }
}
impl fmt::Debug for ThreadSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ThreadSnapshot([redacted])")
    }
}

impl ThreadSnapshot {
    /// Complete bounded native Thread payload, including fields this client does not interpret.
    pub fn protobuf(&self) -> &Bytes {
        &self.raw_protobuf
    }
}

impl Client {
    /// Loads a thread and returns its resolved settings without hydrating turns.
    ///
    /// This is a mutation-capable RPC and is never retried. A timeout or disconnect
    /// can leave the thread loaded even though the caller did not receive settings.
    /// Local rejection sends nothing; errors after dispatch leave the outcome unknown.
    pub async fn activate(&self, thread_id: &str) -> Result<ThreadActivation, MutationError> {
        if thread_id.trim().is_empty() {
            return Err(MutationError::InvalidRequest);
        }
        let bytes = self
            .mutate(
                wire::UpdateConfigRequest {
                    exclude_turns: Some(true),
                    thread_id: Some(thread_id.to_owned()),
                },
                "/oaiproto.codex_app_server_backend.ThreadService/UpdateConfig",
            )
            .await?;
        let response = wire::UpdateConfigResponse::decode(bytes)
            .map_err(|_| MutationError::OutcomeUnknown(Error::InvalidResponse))?;
        let settings =
            ThreadActivation::from_settings(response).map_err(MutationError::OutcomeUnknown)?;
        if settings.thread.id != thread_id {
            return Err(MutationError::OutcomeUnknown(Error::InvalidResponse));
        }
        Ok(settings)
    }
}

impl ThreadActivation {
    pub(crate) fn from_settings(response: wire::UpdateConfigResponse) -> Result<Self, Error> {
        let raw_thread = required(response.thread)?;
        let thread = ThreadSnapshot::decode(raw_thread)?;
        if thread.session_id.is_empty() || thread.model_provider.is_empty() {
            return Err(Error::InvalidResponse);
        }
        Ok(Self {
            thread,
            collaboration_mode: response
                .collaboration_mode
                .map(parse_collaboration_mode)
                .transpose()?,
            model: nonempty(response.model)?,
            model_provider: nonempty(response.model_provider)?,
            service_tier: response.service_tier.and_then(|value| match value.value {
                Some(wire::nullable_string::Value::String(value)) => Some(value),
                Some(wire::nullable_string::Value::Null(_)) | None => None,
            }),
            cwd: nonempty(response.cwd)?,
            instruction_sources: response
                .instruction_sources
                .map_or_else(Vec::new, |sources| sources.values),
            runtime_workspace_roots: response
                .runtime_workspace_roots
                .map_or_else(Vec::new, |roots| roots.values),
            approval_policy: parse_approval(required(response.approval_policy)?)?,
            approvals_reviewer: parse_reviewer(required(response.approvals_reviewer)?)?,
            sandbox: parse_sandbox(required(response.sandbox)?)?,
            active_permission_profile: response
                .active_permission_profile
                .map(|profile| -> Result<_, Error> {
                    Ok(ActivePermissionProfile {
                        id: nonempty(profile.id)?,
                        extends: profile.extends,
                    })
                })
                .transpose()?,
            reasoning_effort: response.reasoning_effort,
            turns_backwards_cursor: response.turns_backwards_cursor,
            items_backwards_cursor: response.items_backwards_cursor,
        })
    }
}

impl ThreadSnapshot {
    pub(crate) fn decode(raw_protobuf: Bytes) -> Result<Self, Error> {
        let thread =
            wire::Thread::decode(raw_protobuf.clone()).map_err(|_| Error::InvalidResponse)?;
        Self::try_from_wire(thread, raw_protobuf)
    }

    fn try_from_wire(thread: wire::Thread, raw_protobuf: Bytes) -> Result<Self, Error> {
        Ok(Self {
            id: nonempty(thread.id)?,
            session_id: required(thread.session_id)?,
            forked_from_id: thread.forked_from_id,
            parent_thread_id: thread.parent_thread_id,
            preview: required(thread.preview)?,
            ephemeral: required(thread.ephemeral)?,
            daybreak_enabled: thread.daybreak_enabled,
            project_id: thread.project_id,
            history_mode: parse_history_mode(thread.history_mode)?,
            model_provider: required(thread.model_provider)?,
            model: thread.model,
            reasoning_effort: thread.reasoning_effort,
            created_at: required(thread.created_at)?,
            updated_at: required(thread.updated_at)?,
            recency_at: thread.recency_at,
            status: parse_status(required(thread.status)?)?,
            path: thread.path,
            cwd: nonempty(thread.cwd)?,
            cli_version: required(thread.cli_version)?,
            originator: thread.originator,
            source: parse_source(required(thread.source)?)?,
            can_accept_direct_input: thread.can_accept_direct_input,
            thread_source: thread.thread_source,
            agent_nickname: thread.agent_nickname,
            agent_role: thread.agent_role,
            name: thread.name,
            environments: thread
                .environments
                .map(|environments| {
                    environments
                        .values
                        .into_iter()
                        .map(|environment| -> Result<_, Error> {
                            Ok(ThreadEnvironment {
                                environment_id: nonempty(environment.environment_id)?,
                                cwd: nonempty(environment.cwd)?,
                                runtime_workspace_roots: required(
                                    environment.runtime_workspace_roots,
                                )?
                                .values,
                            })
                        })
                        .collect()
                })
                .transpose()?,
            turns: [],
            raw_protobuf,
        })
    }
}

fn parse_collaboration_mode(value: wire::CollaborationMode) -> Result<CollaborationMode, Error> {
    let settings = required(value.settings)?;
    let settings = CollaborationSettings {
        model: nonempty(settings.model)?,
        reasoning_effort: settings.reasoning_effort,
        developer_instructions: settings.developer_instructions,
    };
    match required(value.mode)? {
        1 => Ok(CollaborationMode::Plan(settings)),
        2 => Ok(CollaborationMode::Default(settings)),
        _ => Err(Error::UnsupportedResponse("collaboration mode")),
    }
}

fn parse_history_mode(value: Option<i32>) -> Result<ThreadHistoryMode, Error> {
    match value {
        None | Some(1) => Ok(ThreadHistoryMode::Legacy),
        Some(2) => Ok(ThreadHistoryMode::Paginated),
        Some(_) => Err(Error::UnsupportedResponse("thread history mode")),
    }
}

fn parse_approval(value: wire::ApprovalPolicy) -> Result<ApprovalPolicy, Error> {
    match required(value.value)? {
        wire::approval_policy::Value::Simple(1) => Ok(ApprovalPolicy::UnlessTrusted),
        wire::approval_policy::Value::Simple(2) => Ok(ApprovalPolicy::OnRequest),
        wire::approval_policy::Value::Simple(3) => Ok(ApprovalPolicy::Never),
        wire::approval_policy::Value::Simple(_) => {
            Err(Error::UnsupportedResponse("approval policy"))
        }
        wire::approval_policy::Value::Granular(wrapper) => {
            let granular = required(wrapper.granular)?;
            Ok(ApprovalPolicy::Granular {
                sandbox_approval: required(granular.sandbox_approval)?,
                rules: required(granular.rules)?,
                skill_approval: granular.skill_approval.unwrap_or(false),
                request_permissions: granular.request_permissions.unwrap_or(false),
                mcp_elicitations: required(granular.mcp_elicitations)?,
            })
        }
    }
}

fn parse_reviewer(value: i32) -> Result<ApprovalsReviewer, Error> {
    match value {
        1 => Ok(ApprovalsReviewer::User),
        2 | 3 => Ok(ApprovalsReviewer::AutoReview),
        _ => Err(Error::UnsupportedResponse("approvals reviewer")),
    }
}

fn parse_sandbox(value: wire::SandboxPolicy) -> Result<SandboxPolicy, Error> {
    match required(value.value)? {
        wire::sandbox_policy::Value::DangerFullAccess(_) => Ok(SandboxPolicy::DangerFullAccess),
        wire::sandbox_policy::Value::ReadOnly(policy) => Ok(SandboxPolicy::ReadOnly {
            network_access: policy.network_access.unwrap_or(false),
        }),
        wire::sandbox_policy::Value::ExternalSandbox(policy) => {
            Ok(SandboxPolicy::ExternalSandbox {
                network_access: match policy.network_access {
                    None | Some(1) => NetworkAccess::Restricted,
                    Some(2) => NetworkAccess::Enabled,
                    Some(_) => return Err(Error::UnsupportedResponse("external network policy")),
                },
            })
        }
        wire::sandbox_policy::Value::WorkspaceWrite(policy) => Ok(SandboxPolicy::WorkspaceWrite {
            writable_roots: policy
                .writable_roots
                .map_or_else(Vec::new, |roots| roots.values),
            network_access: policy.network_access.unwrap_or(false),
            exclude_tmpdir_env_var: policy.exclude_tmpdir_env_var.unwrap_or(false),
            exclude_slash_tmp: policy.exclude_slash_tmp.unwrap_or(false),
        }),
    }
}

fn parse_source(value: wire::SessionSource) -> Result<SessionSource, Error> {
    match required(value.value)? {
        wire::session_source::Value::Simple(1) => Ok(SessionSource::Cli),
        wire::session_source::Value::Simple(2) => Ok(SessionSource::VsCode),
        wire::session_source::Value::Simple(3) => Ok(SessionSource::Exec),
        wire::session_source::Value::Simple(4) => Ok(SessionSource::AppServer),
        wire::session_source::Value::Simple(0) => Err(Error::UnsupportedResponse("session source")),
        wire::session_source::Value::Simple(_) => Ok(SessionSource::Unknown),
        wire::session_source::Value::Custom(source) => {
            Ok(SessionSource::Custom(nonempty(source.custom)?))
        }
        wire::session_source::Value::SubAgent(wrapper) => Ok(SessionSource::SubAgent(
            parse_subagent_source(required(wrapper.sub_agent)?)?,
        )),
    }
}

fn parse_subagent_source(value: wire::SubAgentSource) -> Result<SubAgentSource, Error> {
    match required(value.value)? {
        wire::sub_agent_source::Value::Simple(1) => Ok(SubAgentSource::Review),
        wire::sub_agent_source::Value::Simple(2) => Ok(SubAgentSource::Compact),
        wire::sub_agent_source::Value::Simple(3) => Ok(SubAgentSource::MemoryConsolidation),
        wire::sub_agent_source::Value::Simple(_) => {
            Err(Error::UnsupportedResponse("sub-agent source"))
        }
        wire::sub_agent_source::Value::ThreadSpawn(wrapper) => {
            let source = required(wrapper.thread_spawn)?;
            Ok(SubAgentSource::ThreadSpawn {
                parent_thread_id: nonempty(source.parent_thread_id)?,
                depth: i32::try_from(required(source.depth)?)
                    .map_err(|_| Error::InvalidResponse)?,
                agent_path: source.agent_path,
                agent_nickname: source.agent_nickname,
                agent_role: source.agent_role,
            })
        }
        wire::sub_agent_source::Value::Other(source) => {
            Ok(SubAgentSource::Other(nonempty(source.other)?))
        }
    }
}

pub(super) fn parse_status(value: wire::ThreadStatus) -> Result<ThreadStatus, Error> {
    match required(value.value)? {
        wire::thread_status::Value::NotLoaded(_) => Ok(ThreadStatus::NotLoaded),
        wire::thread_status::Value::Idle(_) => Ok(ThreadStatus::Idle),
        wire::thread_status::Value::SystemError(_) => Ok(ThreadStatus::SystemError),
        wire::thread_status::Value::Active(active) => Ok(ThreadStatus::Active {
            active_flags: required(active.active_flags)?
                .values
                .into_iter()
                .map(|flag| match flag {
                    1 => Ok(ThreadActiveFlag::WaitingOnApproval),
                    2 => Ok(ThreadActiveFlag::WaitingOnUserInput),
                    _ => Err(Error::UnsupportedResponse("thread active flag")),
                })
                .collect::<Result<_, _>>()?,
        }),
    }
}

fn required<T>(value: Option<T>) -> Result<T, Error> {
    value.ok_or(Error::InvalidResponse)
}

fn nonempty(value: Option<String>) -> Result<String, Error> {
    let value = required(value)?;
    if value.is_empty() {
        Err(Error::InvalidResponse)
    } else {
        Ok(value)
    }
}
