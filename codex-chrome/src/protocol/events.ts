/**
 * Event types ported from codex-rs/protocol/src/protocol.rs
 * Preserving exact event names and structures
 */

import { ReviewRequest } from './types';

/**
 * Complete EventMsg enumeration
 */
export type EventMsg =
  | { type: 'Error'; data: ErrorEvent }
  | { type: 'TaskStarted'; data: TaskStartedEvent }
  | { type: 'TaskComplete'; data: TaskCompleteEvent }
  | { type: 'TokenCount'; data: TokenCountEvent }
  | { type: 'AgentMessage'; data: AgentMessageEvent }
  | { type: 'UserMessage'; data: UserMessageEvent }
  | { type: 'AgentMessageDelta'; data: AgentMessageDeltaEvent }
  | { type: 'AgentReasoning'; data: AgentReasoningEvent }
  | { type: 'AgentReasoningDelta'; data: AgentReasoningDeltaEvent }
  | { type: 'AgentReasoningRawContent'; data: AgentReasoningRawContentEvent }
  | { type: 'AgentReasoningRawContentDelta'; data: AgentReasoningRawContentDeltaEvent }
  | { type: 'AgentReasoningSectionBreak'; data: AgentReasoningSectionBreakEvent }
  | { type: 'SessionConfigured'; data: SessionConfiguredEvent }
  | { type: 'McpToolCallBegin'; data: McpToolCallBeginEvent }
  | { type: 'McpToolCallEnd'; data: McpToolCallEndEvent }
  | { type: 'WebSearchBegin'; data: WebSearchBeginEvent }
  | { type: 'WebSearchEnd'; data: WebSearchEndEvent }
  | { type: 'ExecCommandBegin'; data: ExecCommandBeginEvent }
  | { type: 'ExecCommandOutputDelta'; data: ExecCommandOutputDeltaEvent }
  | { type: 'ExecCommandEnd'; data: ExecCommandEndEvent }
  | { type: 'ExecApprovalRequest'; data: ExecApprovalRequestEvent }
  | { type: 'ApplyPatchApprovalRequest'; data: ApplyPatchApprovalRequestEvent }
  | { type: 'BackgroundEvent'; data: BackgroundEventEvent }
  | { type: 'StreamError'; data: StreamErrorEvent }
  | { type: 'PatchApplyBegin'; data: PatchApplyBeginEvent }
  | { type: 'PatchApplyEnd'; data: PatchApplyEndEvent }
  | { type: 'TurnDiff'; data: TurnDiffEvent }
  | { type: 'GetHistoryEntryResponse'; data: GetHistoryEntryResponseEvent }
  | { type: 'McpListToolsResponse'; data: McpListToolsResponseEvent }
  | { type: 'ListCustomPromptsResponse'; data: ListCustomPromptsResponseEvent }
  | { type: 'PlanUpdate'; data: UpdatePlanArgs }
  | { type: 'TurnAborted'; data: TurnAbortedEvent }
  | { type: 'ShutdownComplete' }
  | { type: 'ConversationPath'; data: ConversationPathResponseEvent }
  | { type: 'EnteredReviewMode'; data: ReviewRequest }
  | { type: 'ExitedReviewMode'; data: ExitedReviewModeEvent };

// Individual event payload types

export interface ErrorEvent {
  message: string;
}

export interface TaskStartedEvent {
  model_context_window?: number;
}

export interface TaskCompleteEvent {
  last_agent_message?: string;
}

export interface TokenUsage {
  input_tokens: number;
  cached_input_tokens: number;
  output_tokens: number;
  reasoning_output_tokens: number;
  total_tokens: number;
}

export interface TokenUsageInfo {
  total_token_usage: TokenUsage;
  last_token_usage: TokenUsage;
  model_context_window?: number;
}

export interface TokenCountEvent {
  info?: TokenUsageInfo;
  rate_limits?: RateLimitSnapshotEvent;
}

export interface RateLimitSnapshotEvent {
  /** Percentage (0-100) of the primary window that has been consumed */
  primary_used_percent: number;
  /** Percentage (0-100) of the secondary window that has been consumed */
  secondary_used_percent: number;
  /** Size of the primary window relative to secondary (0-100) */
  primary_to_secondary_ratio_percent: number;
  /** Rolling window duration for the primary limit, in minutes */
  primary_window_minutes: number;
  /** Rolling window duration for the secondary limit, in minutes */
  secondary_window_minutes: number;
}

export interface AgentMessageEvent {
  message: string;
}

export interface UserMessageEvent {
  message: string;
}

export interface AgentMessageDeltaEvent {
  delta: string;
}

export interface AgentReasoningEvent {
  content: string;
}

export interface AgentReasoningDeltaEvent {
  delta: string;
}

export interface AgentReasoningRawContentEvent {
  content: string;
}

export interface AgentReasoningRawContentDeltaEvent {
  delta: string;
}

export interface AgentReasoningSectionBreakEvent {
  title?: string;
}

export interface SessionConfiguredEvent {
  session_id: string;
}

export interface McpToolCallBeginEvent {
  tool_name: string;
  params: any;
}

export interface McpToolCallEndEvent {
  tool_name: string;
  result?: any;
  error?: string;
}

export interface WebSearchBeginEvent {
  query: string;
}

export interface WebSearchEndEvent {
  query: string;
  results_count: number;
}

export interface ExecCommandBeginEvent {
  session_id: string;
  command: string;
  /** Added for Chrome extension context */
  tab_id?: number;
  url?: string;
}

export interface ExecCommandOutputDeltaEvent {
  session_id: string;
  output: string;
  stream: 'stdout' | 'stderr';
}

export interface ExecCommandEndEvent {
  session_id: string;
  exit_code: number;
  duration_ms?: number;
}

export interface ExecApprovalRequestEvent {
  id: string;
  command: string;
  explanation?: string;
}

export interface ApplyPatchApprovalRequestEvent {
  id: string;
  path: string;
  patch: string;
}

export interface BackgroundEventEvent {
  message: string;
  level?: 'info' | 'warning' | 'error';
}

export interface StreamErrorEvent {
  error: string;
  retrying: boolean;
  attempt?: number;
}

export interface PatchApplyBeginEvent {
  path: string;
  description?: string;
}

export interface PatchApplyEndEvent {
  path: string;
  success: boolean;
  error?: string;
}

export interface TurnDiffEvent {
  diff: string;
  files_changed: number;
}

export interface GetHistoryEntryResponseEvent {
  entry?: HistoryEntry;
  error?: string;
}

export interface HistoryEntry {
  timestamp: number;
  text: string;
  type: 'user' | 'agent';
}

export interface McpListToolsResponseEvent {
  tools: McpTool[];
}

export interface McpTool {
  name: string;
  description: string;
  parameters?: any;
}

export interface ListCustomPromptsResponseEvent {
  prompts: CustomPrompt[];
}

export interface CustomPrompt {
  name: string;
  content: string;
}

export interface UpdatePlanArgs {
  tasks: PlanTask[];
}

export interface PlanTask {
  id: string;
  description: string;
  status: 'pending' | 'in_progress' | 'completed';
}

export interface TurnAbortedEvent {
  reason: TurnAbortReason;
  submission_id?: string;
}

export type TurnAbortReason = 'user_interrupt' | 'automatic_abort' | 'error';

export interface ConversationPathResponseEvent {
  path: string;
  messages_count: number;
}

export interface ExitedReviewModeEvent {
  review_output?: ReviewOutputEvent;
}

export interface ReviewOutputEvent {
  approved: boolean;
  changes?: string;
  comments?: string;
}