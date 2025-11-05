/**
 * Codex Orchestrator RPC Protocol Types
 * Based on codex-rs/orchestrator/src/rpc.rs
 */

// ========== RPC Envelope Types ==========

export interface RpcRequest {
  id: string;
  idem_key?: string;
  method: string;
  params: Record<string, unknown>;
}

export interface RpcResponse {
  id: string;
  result?: unknown;
  error?: RpcError;
}

export interface RpcError {
  code: number;
  message: string;
  data?: unknown;
}

// Error codes
export const ERROR_PARSE = -32700;
export const ERROR_INVALID_REQUEST = -32600;
export const ERROR_METHOD_NOT_FOUND = -32601;
export const ERROR_INVALID_PARAMS = -32602;
export const ERROR_INTERNAL = -32603;
export const ERROR_CONFLICT = 409;
export const ERROR_BACKPRESSURE = 429;

// ========== Lock Methods ==========

export interface LockStatusRequest {
  path?: string;
}

export interface LockStatusResponse {
  locked: boolean;
  holder?: string;
  acquired_at?: string;
}

export interface LockAcquireRequest {
  path: string;
  force?: boolean;
}

export interface LockAcquireResponse {
  success: boolean;
  message?: string;
}

export interface LockReleaseRequest {
  path: string;
}

export interface LockReleaseResponse {
  success: boolean;
}

// ========== Status Methods ==========

export interface StatusGetRequest {}

export interface StatusGetResponse {
  server_version: string;
  uptime_seconds: number;
  queue_size: number;
  queue_capacity: number;
  active_agents: number;
  active_tasks: number;
  total_tokens_used: number;
  total_tokens_budget: number;
}

// ========== Filesystem Methods ==========

export interface FsReadRequest {
  path: string;
}

export interface FsReadResponse {
  content: string;
}

export interface FsWriteRequest {
  path: string;
  content: string;
  preimage_sha?: string;
}

export interface FsWriteResponse {
  success: boolean;
  new_sha: string;
}

export interface FsPatchRequest {
  unified_diff: string;
  base_commit: string;
}

export interface FsPatchResponse {
  success: boolean;
  applied_files: string[];
}

// ========== VCS Methods ==========

export interface VcsDiffRequest {}

export interface VcsDiffResponse {
  diff: string;
}

export interface VcsCommitRequest {
  message: string;
}

export interface VcsCommitResponse {
  success: boolean;
  commit_sha: string;
}

export interface VcsPushRequest {
  remote: string;
  branch: string;
}

export interface VcsPushResponse {
  success: boolean;
}

// ========== Agent Methods ==========

export interface AgentRegisterRequest {
  agent_id: string;
  agent_type: string;
}

export interface AgentRegisterResponse {
  success: boolean;
}

export interface AgentHeartbeatRequest {
  agent_id: string;
}

export interface AgentHeartbeatResponse {
  success: boolean;
}

export interface AgentListRequest {}

export interface AgentInfo {
  agent_id: string;
  agent_type: string;
  status: string;
  last_heartbeat: string;
}

export interface AgentListResponse {
  agents: AgentInfo[];
}

// ========== Task Methods ==========

export interface TaskSubmitRequest {
  task_id: string;
  agent_type: string;
  task_description: string;
  metadata?: Record<string, string>;
}

export interface TaskSubmitResponse {
  success: boolean;
  task_id: string;
}

export interface TaskCancelRequest {
  task_id: string;
}

export interface TaskCancelResponse {
  success: boolean;
}

// ========== Token Methods ==========

export interface TokensReportUsageRequest {
  agent_id: string;
  tokens_used: number;
}

export interface TokensReportUsageResponse {
  success: boolean;
  remaining_budget: number;
}

export interface TokensGetBudgetRequest {}

export interface TokensGetBudgetResponse {
  total_budget: number;
  used: number;
  remaining: number;
  warning_threshold: number;
}

// ========== Session Methods ==========

export interface SessionStartRequest {
  session_id: string;
  cwd: string;
}

export interface SessionStartResponse {
  success: boolean;
}

export interface SessionEndRequest {
  session_id: string;
}

export interface SessionEndResponse {
  success: boolean;
}

// ========== PubSub Methods ==========

export interface PubSubSubscribeRequest {
  topics: string[];
}

export interface PubSubSubscribeResponse {
  success: boolean;
}

export interface PubSubUnsubscribeRequest {
  topics: string[];
}

export interface PubSubUnsubscribeResponse {
  success: boolean;
}

// ========== Blueprint Methods ==========

export interface BlueprintCreateRequest {
  description: string;
  context?: Record<string, unknown>;
}

export interface BlueprintCreateResponse {
  success: boolean;
  blueprint_id: string;
}

export interface BlueprintApproveRequest {
  blueprint_id: string;
}

export interface BlueprintApproveResponse {
  success: boolean;
}

export interface BlueprintRejectRequest {
  blueprint_id: string;
  reason?: string;
}

export interface BlueprintRejectResponse {
  success: boolean;
}

export interface BlueprintExportRequest {
  blueprint_id: string;
  format?: 'markdown' | 'json' | 'both';
}

export interface BlueprintExportResponse {
  success: boolean;
  markdown_path?: string;
  json_path?: string;
}

export interface BlueprintSetModeRequest {
  blueprint_id: string;
  mode: 'single' | 'orchestrated' | 'competition';
}

export interface BlueprintSetModeResponse {
  success: boolean;
}

export interface BlueprintGetRequest {
  blueprint_id: string;
}

export interface BlueprintGetResponse {
  blueprint?: unknown;
}

// ========== Events ==========

export interface RpcEvent {
  topic: string;
  data: unknown;
  timestamp: string;
}

// Event topics
export const EVENT_LOCK_CHANGED = 'lock.changed';
export const EVENT_TOKENS_UPDATED = 'tokens.updated';
export const EVENT_AGENT_STATUS = 'agent.status';
export const EVENT_TASK_COMPLETED = 'task.completed';
export const EVENT_TASK_FAILED = 'task.failed';

