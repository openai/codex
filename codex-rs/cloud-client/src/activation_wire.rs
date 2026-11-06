//! Minimal protobuf envelopes for authoritative thread activation.

use bytes::Bytes;
use prost::Message;
use std::fmt;

#[derive(Clone, Message)]
pub(super) struct UpdateConfigRequest {
    #[prost(bool, optional, tag = "7")]
    pub exclude_turns: Option<bool>,
    #[prost(string, optional, tag = "18")]
    pub thread_id: Option<String>,
}

#[derive(Clone, Message)]
#[prost(skip_debug)]
pub(super) struct UpdateConfigResponse {
    #[prost(message, optional, tag = "1")]
    pub active_permission_profile: Option<ActivePermissionProfile>,
    #[prost(message, optional, tag = "2")]
    pub approval_policy: Option<ApprovalPolicy>,
    #[prost(int32, optional, tag = "3")]
    pub approvals_reviewer: Option<i32>,
    #[prost(message, optional, tag = "4")]
    pub collaboration_mode: Option<CollaborationMode>,
    #[prost(string, optional, tag = "5")]
    pub cwd: Option<String>,
    #[prost(message, optional, tag = "8")]
    pub instruction_sources: Option<StringList>,
    #[prost(string, optional, tag = "9")]
    pub items_backwards_cursor: Option<String>,
    #[prost(string, optional, tag = "10")]
    pub model: Option<String>,
    #[prost(string, optional, tag = "11")]
    pub model_provider: Option<String>,
    #[prost(string, optional, tag = "13")]
    pub reasoning_effort: Option<String>,
    #[prost(message, optional, tag = "14")]
    pub runtime_workspace_roots: Option<StringList>,
    #[prost(message, optional, tag = "15")]
    pub sandbox: Option<SandboxPolicy>,
    #[prost(message, optional, tag = "16")]
    pub service_tier: Option<NullableString>,
    #[prost(bytes = "bytes", optional, tag = "17")]
    pub thread: Option<Bytes>,
    #[prost(string, optional, tag = "18")]
    pub turns_backwards_cursor: Option<String>,
}

impl fmt::Debug for UpdateConfigResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UpdateConfigResponse([redacted])")
    }
}

#[derive(Clone, Message)]
pub(super) struct CollaborationMode {
    #[prost(int32, optional, tag = "1")]
    pub mode: Option<i32>,
    #[prost(message, optional, tag = "2")]
    pub settings: Option<Settings>,
}

#[derive(Clone, Message)]
#[prost(skip_debug)]
pub(super) struct Settings {
    #[prost(string, optional, tag = "1")]
    pub developer_instructions: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub model: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub reasoning_effort: Option<String>,
}

impl fmt::Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Settings([redacted])")
    }
}

#[derive(Clone, Message)]
pub(super) struct ActivePermissionProfile {
    #[prost(string, optional, tag = "1")]
    pub extends: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub id: Option<String>,
}

#[derive(Clone, Message)]
pub(super) struct ApprovalPolicy {
    #[prost(oneof = "approval_policy::Value", tags = "1, 2")]
    pub value: Option<approval_policy::Value>,
}
pub(super) mod approval_policy {
    #[derive(Clone, prost::Oneof)]
    pub enum Value {
        #[prost(int32, tag = "1")]
        Simple(i32),
        #[prost(message, tag = "2")]
        Granular(super::GranularApprovalWrapper),
    }
}
#[derive(Clone, Message)]
pub(super) struct GranularApprovalWrapper {
    #[prost(message, optional, tag = "1")]
    pub granular: Option<GranularApproval>,
}
#[derive(Clone, Message)]
pub(super) struct GranularApproval {
    #[prost(bool, optional, tag = "1")]
    pub mcp_elicitations: Option<bool>,
    #[prost(bool, optional, tag = "2")]
    pub request_permissions: Option<bool>,
    #[prost(bool, optional, tag = "3")]
    pub rules: Option<bool>,
    #[prost(bool, optional, tag = "4")]
    pub sandbox_approval: Option<bool>,
    #[prost(bool, optional, tag = "5")]
    pub skill_approval: Option<bool>,
}

#[derive(Clone, Message)]
pub(super) struct SandboxPolicy {
    #[prost(oneof = "sandbox_policy::Value", tags = "1, 2, 3, 4")]
    pub value: Option<sandbox_policy::Value>,
}
pub(super) mod sandbox_policy {
    #[derive(Clone, prost::Oneof)]
    pub enum Value {
        #[prost(message, tag = "1")]
        DangerFullAccess(super::Empty),
        #[prost(message, tag = "2")]
        ReadOnly(super::ReadOnlySandbox),
        #[prost(message, tag = "3")]
        ExternalSandbox(super::ExternalSandbox),
        #[prost(message, tag = "4")]
        WorkspaceWrite(super::WorkspaceWriteSandbox),
    }
}
#[derive(Clone, Message)]
pub(super) struct Empty {}
#[derive(Clone, Message)]
pub(super) struct ReadOnlySandbox {
    #[prost(bool, optional, tag = "1")]
    pub network_access: Option<bool>,
}
#[derive(Clone, Message)]
pub(super) struct ExternalSandbox {
    #[prost(int32, optional, tag = "1")]
    pub network_access: Option<i32>,
}
#[derive(Clone, Message)]
pub(super) struct WorkspaceWriteSandbox {
    #[prost(bool, optional, tag = "1")]
    pub exclude_slash_tmp: Option<bool>,
    #[prost(bool, optional, tag = "2")]
    pub exclude_tmpdir_env_var: Option<bool>,
    #[prost(bool, optional, tag = "3")]
    pub network_access: Option<bool>,
    #[prost(message, optional, tag = "4")]
    pub writable_roots: Option<StringList>,
}
#[derive(Clone, Message)]
pub(super) struct StringList {
    #[prost(string, repeated, tag = "1")]
    pub values: Vec<String>,
}
#[derive(Clone, Message)]
pub(super) struct NullableString {
    #[prost(oneof = "nullable_string::Value", tags = "1, 2")]
    pub value: Option<nullable_string::Value>,
}
pub(super) mod nullable_string {
    #[derive(Clone, prost::Oneof)]
    pub enum Value {
        #[prost(string, tag = "1")]
        String(String),
        #[prost(message, tag = "2")]
        Null(super::Empty),
    }
}

#[derive(Clone, Message)]
#[prost(skip_debug)]
pub(super) struct Thread {
    #[prost(string, optional, tag = "1")]
    pub agent_nickname: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub agent_role: Option<String>,
    #[prost(bool, optional, tag = "3")]
    pub can_accept_direct_input: Option<bool>,
    #[prost(string, optional, tag = "4")]
    pub cli_version: Option<String>,
    #[prost(int64, optional, tag = "5")]
    pub created_at: Option<i64>,
    #[prost(string, optional, tag = "6")]
    pub cwd: Option<String>,
    #[prost(bool, optional, tag = "7")]
    pub daybreak_enabled: Option<bool>,
    #[prost(message, optional, tag = "8")]
    pub environments: Option<Environments>,
    #[prost(bool, optional, tag = "9")]
    pub ephemeral: Option<bool>,
    #[prost(string, optional, tag = "11")]
    pub forked_from_id: Option<String>,
    #[prost(message, optional, tag = "12")]
    pub git_info: Option<GitInfo>,
    #[prost(int32, optional, tag = "13")]
    pub history_mode: Option<i32>,
    #[prost(string, optional, tag = "14")]
    pub id: Option<String>,
    #[prost(string, optional, tag = "15")]
    pub model: Option<String>,
    #[prost(string, optional, tag = "16")]
    pub model_provider: Option<String>,
    #[prost(string, optional, tag = "17")]
    pub name: Option<String>,
    #[prost(string, optional, tag = "18")]
    pub originator: Option<String>,
    #[prost(string, optional, tag = "19")]
    pub parent_thread_id: Option<String>,
    #[prost(string, optional, tag = "20")]
    pub path: Option<String>,
    #[prost(string, optional, tag = "21")]
    pub preview: Option<String>,
    #[prost(string, optional, tag = "22")]
    pub project_id: Option<String>,
    #[prost(string, optional, tag = "23")]
    pub reasoning_effort: Option<String>,
    #[prost(int64, optional, tag = "24")]
    pub recency_at: Option<i64>,
    #[prost(string, optional, tag = "27")]
    pub session_id: Option<String>,
    #[prost(message, optional, tag = "28")]
    pub source: Option<SessionSource>,
    #[prost(message, optional, tag = "29")]
    pub status: Option<ThreadStatus>,
    #[prost(string, optional, tag = "30")]
    pub thread_source: Option<String>,
    #[prost(int64, optional, tag = "32")]
    pub updated_at: Option<i64>,
}

impl fmt::Debug for Thread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Thread([redacted])")
    }
}

#[derive(Clone, Message)]
#[prost(skip_debug)]
pub(super) struct GitInfo {
    #[prost(string, optional, tag = "1")]
    pub branch: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub origin_url: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub sha: Option<String>,
}

impl fmt::Debug for GitInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GitInfo([redacted])")
    }
}

#[derive(Clone, Message)]
pub(super) struct Environments {
    #[prost(message, repeated, tag = "1")]
    pub values: Vec<Environment>,
}
#[derive(Clone, Message)]
pub(super) struct Environment {
    #[prost(string, optional, tag = "1")]
    pub cwd: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub environment_id: Option<String>,
    #[prost(message, optional, tag = "3")]
    pub runtime_workspace_roots: Option<StringList>,
}
#[derive(Clone, Message)]
pub(super) struct SessionSource {
    #[prost(oneof = "session_source::Value", tags = "1, 2, 3")]
    pub value: Option<session_source::Value>,
}
pub(super) mod session_source {
    #[derive(Clone, prost::Oneof)]
    pub enum Value {
        #[prost(int32, tag = "1")]
        Simple(i32),
        #[prost(message, tag = "2")]
        Custom(super::CustomSource),
        #[prost(message, tag = "3")]
        SubAgent(super::SubAgentSourceWrapper),
    }
}
#[derive(Clone, Message)]
pub(super) struct SubAgentSourceWrapper {
    #[prost(message, optional, tag = "1")]
    pub sub_agent: Option<SubAgentSource>,
}
#[derive(Clone, Message)]
pub(super) struct SubAgentSource {
    #[prost(oneof = "sub_agent_source::Value", tags = "1, 2, 3")]
    pub value: Option<sub_agent_source::Value>,
}
pub(super) mod sub_agent_source {
    #[derive(Clone, prost::Oneof)]
    pub enum Value {
        #[prost(int32, tag = "1")]
        Simple(i32),
        #[prost(message, tag = "2")]
        ThreadSpawn(super::ThreadSpawnWrapper),
        #[prost(message, tag = "3")]
        Other(super::OtherSubAgentSource),
    }
}
#[derive(Clone, Message)]
pub(super) struct OtherSubAgentSource {
    #[prost(string, optional, tag = "1")]
    pub other: Option<String>,
}
#[derive(Clone, Message)]
pub(super) struct ThreadSpawnWrapper {
    #[prost(message, optional, tag = "1")]
    pub thread_spawn: Option<ThreadSpawnSource>,
}
#[derive(Clone, Message)]
pub(super) struct ThreadSpawnSource {
    #[prost(string, optional, tag = "1")]
    pub agent_nickname: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub agent_path: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub agent_role: Option<String>,
    #[prost(int64, optional, tag = "4")]
    pub depth: Option<i64>,
    #[prost(string, optional, tag = "5")]
    pub parent_thread_id: Option<String>,
}
#[derive(Clone, Message)]
pub(super) struct CustomSource {
    #[prost(string, optional, tag = "1")]
    pub custom: Option<String>,
}
#[derive(Clone, Message)]
pub(super) struct ThreadStatus {
    #[prost(oneof = "thread_status::Value", tags = "1, 2, 3, 4")]
    pub value: Option<thread_status::Value>,
}
pub(super) mod thread_status {
    #[derive(Clone, prost::Oneof)]
    pub enum Value {
        #[prost(message, tag = "1")]
        NotLoaded(super::Empty),
        #[prost(message, tag = "2")]
        Idle(super::Empty),
        #[prost(message, tag = "3")]
        SystemError(super::Empty),
        #[prost(message, tag = "4")]
        Active(super::ActiveStatus),
    }
}
#[derive(Clone, Message)]
pub(super) struct ActiveStatus {
    #[prost(message, optional, tag = "1")]
    pub active_flags: Option<IntList>,
}
#[derive(Clone, Message)]
pub(super) struct IntList {
    #[prost(int32, repeated, tag = "1")]
    pub values: Vec<i32>,
}
