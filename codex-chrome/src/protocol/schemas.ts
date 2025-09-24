/**
 * Zod schemas for runtime validation of protocol types
 */

import { z } from 'zod';

// Input item schemas
const TextInputItemSchema = z.object({
  type: z.literal('text'),
  text: z.string(),
});

const ImageInputItemSchema = z.object({
  type: z.literal('image'),
  image_url: z.string(),
});

const ClipboardInputItemSchema = z.object({
  type: z.literal('clipboard'),
  content: z.string().optional(),
});

const ContextInputItemSchema = z.object({
  type: z.literal('context'),
  path: z.string().optional(),
});

export const InputItemSchema = z.discriminatedUnion('type', [
  TextInputItemSchema,
  ImageInputItemSchema,
  ClipboardInputItemSchema,
  ContextInputItemSchema,
]);

// Review decision schema
export const ReviewDecisionSchema = z.enum(['approve', 'reject', 'request_change']);

// Reasoning config schemas
export const ReasoningEffortConfigSchema = z.object({
  effort: z.enum(['low', 'medium', 'high']),
});

export const ReasoningSummaryConfigSchema = z.object({
  enabled: z.boolean(),
});

// Approval policy schema
export const AskForApprovalSchema = z.enum(['untrusted', 'on-failure', 'on-request', 'never']);

// Sandbox policy schemas
const DangerFullAccessSchema = z.object({
  mode: z.literal('danger-full-access'),
});

const ReadOnlySchema = z.object({
  mode: z.literal('read-only'),
});

const WorkspaceWriteSchema = z.object({
  mode: z.literal('workspace-write'),
  writable_roots: z.array(z.string()).optional(),
  network_access: z.boolean().optional(),
  exclude_tmpdir_env_var: z.boolean().optional(),
  exclude_slash_tmp: z.boolean().optional(),
});

export const SandboxPolicySchema = z.discriminatedUnion('mode', [
  DangerFullAccessSchema,
  ReadOnlySchema,
  WorkspaceWriteSchema,
]);

// Review request schema
export const ReviewRequestSchema = z.object({
  id: z.string(),
  content: z.string(),
  type: z.enum(['code', 'document', 'general']).optional(),
});

// Op schemas
const InterruptOpSchema = z.object({
  type: z.literal('Interrupt'),
});

const UserInputOpSchema = z.object({
  type: z.literal('UserInput'),
  items: z.array(InputItemSchema),
});

const UserTurnOpSchema = z.object({
  type: z.literal('UserTurn'),
  items: z.array(InputItemSchema),
  cwd: z.string(),
  approval_policy: AskForApprovalSchema,
  sandbox_policy: SandboxPolicySchema,
  model: z.string(),
  effort: ReasoningEffortConfigSchema.optional(),
  summary: ReasoningSummaryConfigSchema,
});

const OverrideTurnContextOpSchema = z.object({
  type: z.literal('OverrideTurnContext'),
  cwd: z.string().optional(),
  approval_policy: AskForApprovalSchema.optional(),
  sandbox_policy: SandboxPolicySchema.optional(),
  model: z.string().optional(),
  effort: z.union([ReasoningEffortConfigSchema, z.null()]).optional(),
  summary: ReasoningSummaryConfigSchema.optional(),
});

const ExecApprovalOpSchema = z.object({
  type: z.literal('ExecApproval'),
  id: z.string(),
  decision: ReviewDecisionSchema,
});

const PatchApprovalOpSchema = z.object({
  type: z.literal('PatchApproval'),
  id: z.string(),
  decision: ReviewDecisionSchema,
});

const AddToHistoryOpSchema = z.object({
  type: z.literal('AddToHistory'),
  text: z.string(),
});

const GetHistoryEntryRequestOpSchema = z.object({
  type: z.literal('GetHistoryEntryRequest'),
  offset: z.number(),
  log_id: z.number(),
});

const GetPathOpSchema = z.object({
  type: z.literal('GetPath'),
});

const ListMcpToolsOpSchema = z.object({
  type: z.literal('ListMcpTools'),
});

const ListCustomPromptsOpSchema = z.object({
  type: z.literal('ListCustomPrompts'),
});

const CompactOpSchema = z.object({
  type: z.literal('Compact'),
});

const ReviewOpSchema = z.object({
  type: z.literal('Review'),
  review_request: ReviewRequestSchema,
});

const ShutdownOpSchema = z.object({
  type: z.literal('Shutdown'),
});

export const OpSchema = z.discriminatedUnion('type', [
  InterruptOpSchema,
  UserInputOpSchema,
  UserTurnOpSchema,
  OverrideTurnContextOpSchema,
  ExecApprovalOpSchema,
  PatchApprovalOpSchema,
  AddToHistoryOpSchema,
  GetHistoryEntryRequestOpSchema,
  GetPathOpSchema,
  ListMcpToolsOpSchema,
  ListCustomPromptsOpSchema,
  CompactOpSchema,
  ReviewOpSchema,
  ShutdownOpSchema,
]);

// Submission schema
export const SubmissionSchema = z.object({
  id: z.string(),
  op: OpSchema,
});

// Basic Event schema (EventMsg will be expanded later)
export const EventSchema = z.object({
  id: z.string(),
  msg: z.object({
    type: z.string(),
    data: z.any().optional(),
  }),
});

/**
 * Validate a submission
 */
export function validateSubmission(data: unknown): data is import('./types').Submission {
  return SubmissionSchema.safeParse(data).success;
}

/**
 * Parse and validate a submission
 */
export function parseSubmission(data: unknown): import('./types').Submission {
  return SubmissionSchema.parse(data);
}

/**
 * Validate an event
 */
export function validateEvent(data: unknown): data is import('./types').Event {
  return EventSchema.safeParse(data).success;
}

/**
 * Parse and validate an event
 */
export function parseEvent(data: unknown): import('./types').Event {
  return EventSchema.parse(data);
}