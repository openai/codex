export type FieldKind = "text" | "textarea" | "select";

export interface FieldOption {
  value: string;
  label: string;
}

export interface ActionFieldDefinition {
  id: string;
  label: string;
  kind: FieldKind;
  placeholder?: string;
  helperText?: string;
  required: boolean;
  defaultValue?: string;
  options?: FieldOption[];
}

export interface ActionMetadata {
  id: string;
  label: string;
  description: string;
  category: string;
  ctaLabel: string;
  fields: ActionFieldDefinition[];
}

export type ExecutionStatus = "completed" | "failed";

export interface ExecutionResponse {
  id: string;
  actionId: string;
  command: string[];
  executedAt: string;
  durationMs: number;
  status: ExecutionStatus;
  exitCode: number | null;
  stdout: string;
  stderr: string;
}

export interface ExecuteActionPayload {
  values: Record<string, string>;
}

export interface ExecutionHistoryEntry {
  action: ActionMetadata;
  request: ExecuteActionPayload;
  response: ExecutionResponse;
}
