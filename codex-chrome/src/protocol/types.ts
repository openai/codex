/**
 * Core protocol types ported from codex-rs/protocol/src/protocol.rs
 * Preserving exact type names and structure from Rust
 */

// Constants from protocol
export const USER_INSTRUCTIONS_OPEN_TAG = '<user_instructions>';
export const USER_INSTRUCTIONS_CLOSE_TAG = '</user_instructions>';
export const ENVIRONMENT_CONTEXT_OPEN_TAG = '<environment_context>';
export const ENVIRONMENT_CONTEXT_CLOSE_TAG = '</environment_context>';
export const USER_MESSAGE_BEGIN = '## My request for Codex:';

/**
 * Submission Queue Entry - requests from user
 */
export interface Submission {
  /** Unique id for this Submission to correlate with Events */
  id: string;
  /** Payload */
  op: Op;
}

/**
 * Submission operation
 */
export type Op =
  | { type: 'Interrupt' }
  | {
      type: 'UserInput';
      /** User input items */
      items: InputItem[];
    }
  | {
      type: 'UserTurn';
      /** User input items */
      items: InputItem[];
      /** cwd to use with the SandboxPolicy */
      cwd: string;
      /** Policy to use for command approval */
      approval_policy: AskForApproval;
      /** Policy to use for tool calls */
      sandbox_policy: SandboxPolicy;
      /** Must be a valid model slug */
      model: string;
      /** Will only be honored if the model is configured to use reasoning */
      effort?: ReasoningEffortConfig;
      /** Will only be honored if the model is configured to use reasoning */
      summary: ReasoningSummaryConfig;
    }
  | {
      type: 'OverrideTurnContext';
      /** Updated cwd for sandbox/tool calls */
      cwd?: string;
      /** Updated command approval policy */
      approval_policy?: AskForApproval;
      /** Updated sandbox policy for tool calls */
      sandbox_policy?: SandboxPolicy;
      /** Updated model slug */
      model?: string;
      /** Updated reasoning effort */
      effort?: ReasoningEffortConfig | null;
      /** Updated reasoning summary preference */
      summary?: ReasoningSummaryConfig;
    }
  | {
      type: 'ExecApproval';
      /** The id of the submission we are approving */
      id: string;
      /** The user's decision in response to the request */
      decision: ReviewDecision;
    }
  | {
      type: 'PatchApproval';
      /** The id of the submission we are approving */
      id: string;
      /** The user's decision in response to the request */
      decision: ReviewDecision;
    }
  | {
      type: 'AddToHistory';
      /** The message text to be stored */
      text: string;
    }
  | {
      type: 'GetHistoryEntryRequest';
      offset: number;
      log_id: number;
    }
  | { type: 'GetPath' }
  | { type: 'ListMcpTools' }
  | { type: 'ListCustomPrompts' }
  | { type: 'Compact' }
  | {
      type: 'Review';
      review_request: ReviewRequest;
    }
  | { type: 'Shutdown' };

/**
 * Determines the conditions under which the user is consulted to approve
 * running the command proposed by Codex.
 */
export type AskForApproval =
  | 'untrusted'    // UnlessTrusted in Rust
  | 'on-failure'   // OnFailure
  | 'on-request'   // OnRequest (default)
  | 'never';       // Never

/**
 * Determines execution restrictions for model shell commands.
 * Adapted for browser context
 */
export type SandboxPolicy =
  | { mode: 'danger-full-access' }
  | { mode: 'read-only' }
  | {
      mode: 'workspace-write';
      /** Additional folders that should be writable (adapted for browser storage) */
      writable_roots?: string[];
      /** When true, network access is allowed */
      network_access?: boolean;
      exclude_tmpdir_env_var?: boolean;
      exclude_slash_tmp?: boolean;
    };

/**
 * User input types
 */
export type InputItem =
  | {
      type: 'text';
      text: string;
    }
  | {
      type: 'image';
      /** Pre-encoded data: URI image */
      image_url: string;
    }
  | {
      type: 'clipboard';
      /** Only available in browser context */
      content?: string;
    }
  | {
      type: 'context';
      /** Path or identifier for context */
      path?: string;
    };

/**
 * Review decision types
 */
export type ReviewDecision = 'approve' | 'reject' | 'request_change';

/**
 * Reasoning configuration
 */
export interface ReasoningEffortConfig {
  effort: 'low' | 'medium' | 'high';
}

export interface ReasoningSummaryConfig {
  enabled: boolean;
}

/**
 * Review request structure
 */
export interface ReviewRequest {
  id: string;
  content: string;
  type?: 'code' | 'document' | 'general';
}

/**
 * Event Queue Entry - responses from agent
 */
export interface Event {
  /** Unique id for this Event */
  id: string;
  /** Event message */
  msg: EventMsg;
}

// Re-export EventMsg from events.ts
export { EventMsg } from './events';