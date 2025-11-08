/// Orchestrator RPC protocol definitions
///
/// Single-Writer Queue architecture with idempotency
use serde::Deserialize;
/// Orchestrator RPC protocol definitions
///
/// Single-Writer Queue architecture with idempotency
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// RPC Request envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    /// Request ID (for response matching)
    pub id: String,
    /// Idempotency key (10 min TTL)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idem_key: Option<String>,
    /// Method name
    pub method: String,
    /// Method parameters (JSON object)
    #[serde(default)]
    pub params: serde_json::Value,
}

/// RPC Response envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    /// Request ID (matches request)
    pub id: String,
    /// Success result (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

/// RPC Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional error data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

// ========== Error Codes ==========
pub const ERROR_PARSE: i32 = -32700;
pub const ERROR_INVALID_REQUEST: i32 = -32600;
pub const ERROR_METHOD_NOT_FOUND: i32 = -32601;
pub const ERROR_INVALID_PARAMS: i32 = -32602;
pub const ERROR_INTERNAL: i32 = -32603;

// Custom error codes
pub const ERROR_CONFLICT: i32 = 409; // Lock conflict
pub const ERROR_BACKPRESSURE: i32 = 429; // Queue full

// ========== RPC Methods (v1.0) ==========

// ---------- Lock Methods ----------

/// lock.status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockStatusRequest {
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockStatusResponse {
    pub locked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub holder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acquired_at: Option<String>,
}

/// lock.acquire
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockAcquireRequest {
    pub path: PathBuf,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockAcquireResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// lock.release
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockReleaseRequest {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockReleaseResponse {
    pub success: bool,
}

// ---------- Status Methods ----------

/// status.get
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusGetRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusGetResponse {
    pub server_version: String,
    pub uptime_seconds: u64,
    pub queue_size: usize,
    pub queue_capacity: usize,
    pub active_agents: usize,
    pub active_tasks: usize,
    pub total_tokens_used: u64,
    pub total_tokens_budget: u64,
}

// ---------- Filesystem Methods ----------

/// fs.read
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsReadRequest {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsReadResponse {
    pub content: String,
}

/// fs.write
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsWriteRequest {
    pub path: PathBuf,
    pub content: String,
    /// Preimage SHA256 (for conflict detection)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preimage_sha: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsWriteResponse {
    pub success: bool,
    pub new_sha: String,
}

/// fs.patch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsPatchRequest {
    pub unified_diff: String,
    pub base_commit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsPatchResponse {
    pub success: bool,
    pub applied_files: Vec<PathBuf>,
}

// ---------- VCS Methods ----------

/// vcs.diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VcsDiffRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VcsDiffResponse {
    pub diff: String,
}

/// vcs.commit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VcsCommitRequest {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VcsCommitResponse {
    pub success: bool,
    pub commit_sha: String,
}

/// vcs.push
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VcsPushRequest {
    pub remote: String,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VcsPushResponse {
    pub success: bool,
}

// ---------- Agent Methods ----------

/// agent.register
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegisterRequest {
    pub agent_id: String,
    pub agent_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegisterResponse {
    pub success: bool,
}

/// agent.heartbeat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHeartbeatRequest {
    pub agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHeartbeatResponse {
    pub success: bool,
}

/// agent.list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentListRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentListResponse {
    pub agents: Vec<AgentInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub agent_id: String,
    pub agent_type: String,
    pub status: String,
    pub last_heartbeat: String,
}

// ---------- Task Methods ----------

/// task.submit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSubmitRequest {
    pub task_id: String,
    pub agent_type: String,
    pub task_description: String,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSubmitResponse {
    pub success: bool,
    pub task_id: String,
}

/// task.cancel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCancelRequest {
    pub task_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCancelResponse {
    pub success: bool,
}

// ---------- Token Methods ----------

/// tokens.reportUsage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokensReportUsageRequest {
    pub agent_id: String,
    pub tokens_used: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokensReportUsageResponse {
    pub success: bool,
    pub remaining_budget: u64,
}

/// tokens.getBudget
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokensGetBudgetRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokensGetBudgetResponse {
    pub total_budget: u64,
    pub used: u64,
    pub remaining: u64,
    pub warning_threshold: u64,
}

// ---------- Session Methods ----------

/// session.start
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStartRequest {
    pub session_id: String,
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStartResponse {
    pub success: bool,
}

/// session.end
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEndRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEndResponse {
    pub success: bool,
}

// ---------- PubSub Methods ----------

/// pubsub.subscribe
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubSubSubscribeRequest {
    pub topics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubSubSubscribeResponse {
    pub success: bool,
}

/// pubsub.unsubscribe
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubSubUnsubscribeRequest {
    pub topics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubSubUnsubscribeResponse {
    pub success: bool,
}

// ---------- Blueprint Methods ----------

/// blueprint.create
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintCreateRequest {
    pub goal: String,
    pub title: String,
    pub mode: String, // "single" | "orchestrated" | "competition"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget: Option<BlueprintBudget>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintBudget {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_step: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_cap: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimate_min: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cap_min: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintCreateResponse {
    pub success: bool,
    pub blueprint_id: String,
}

/// blueprint.get
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintGetRequest {
    pub blueprint_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintGetResponse {
    pub blueprint: serde_json::Value, // Full BlueprintBlock as JSON
}

/// blueprint.update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintUpdateRequest {
    pub blueprint_id: String,
    pub changes: serde_json::Value, // Partial blueprint updates
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintUpdateResponse {
    pub success: bool,
    pub blueprint_id: String, // May be new ID if scope changed
}

/// blueprint.approve
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintApproveRequest {
    pub blueprint_id: String,
    pub approver: String,
    pub approver_role: String, // "user" | "reviewer" | "maintainer" | "admin"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintApproveResponse {
    pub success: bool,
}

/// blueprint.reject
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintRejectRequest {
    pub blueprint_id: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejector: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintRejectResponse {
    pub success: bool,
}

/// blueprint.export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintExportRequest {
    pub blueprint_id: String,
    pub format: String, // "md" | "json" | "both"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintExportResponse {
    pub success: bool,
    pub markdown_path: Option<String>,
    pub json_path: Option<String>,
}

/// blueprint.setMode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintSetModeRequest {
    pub mode: String, // "single" | "orchestrated" | "competition"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintSetModeResponse {
    pub success: bool,
}

/// blueprint.addResearch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintAddResearchRequest {
    pub blueprint_id: String,
    pub research: BlueprintResearch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintResearch {
    pub query: String,
    pub depth: u8,
    pub strategy: String,
    pub sources: Vec<BlueprintResearchSource>,
    pub synthesis: String,
    pub confidence: f64,
    pub needs_approval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintResearchSource {
    pub title: String,
    pub url: String,
    pub date: String,
    pub key_finding: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintAddResearchResponse {
    pub success: bool,
}

// ========== Events ==========

/// RPC Event (published to subscribers)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcEvent {
    /// Event topic
    pub topic: String,
    /// Event data (JSON object)
    pub data: serde_json::Value,
    /// Timestamp (RFC3339)
    pub timestamp: String,
}

// Common event topics
pub const EVENT_LOCK_CHANGED: &str = "lock.changed";
pub const EVENT_TOKENS_UPDATED: &str = "tokens.updated";
pub const EVENT_AGENT_STATUS: &str = "agent.status";
pub const EVENT_TASK_COMPLETED: &str = "task.completed";
pub const EVENT_TASK_FAILED: &str = "task.failed";

// Blueprint event topics
pub const EVENT_BLUEPRINT_CREATED: &str = "blueprint.created";
pub const EVENT_BLUEPRINT_UPDATED: &str = "blueprint.updated";
pub const EVENT_BLUEPRINT_APPROVED: &str = "blueprint.approved";
pub const EVENT_BLUEPRINT_REJECTED: &str = "blueprint.rejected";
pub const EVENT_BLUEPRINT_EXPORTED: &str = "blueprint.exported";
