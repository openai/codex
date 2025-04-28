import type { ModelInfo } from "./model-info";

export type SupportedAnthropicModelId = keyof typeof anthropicModelInfo;

export const anthropicModelInfo = {
  "claude-3-opus-20240229": {
    label: "Claude 3 Opus (20240229)",
    maxContextLength: 200000,
  },
  "claude-3-sonnet-20240229": {
    label: "Claude 3 Sonnet (20240229)",
    maxContextLength: 200000,
  },
  "claude-3-haiku-20240307": {
    label: "Claude 3 Haiku (20240307)",
    maxContextLength: 200000,
  },
  "claude-3-opus": {
    label: "Claude 3 Opus",
    maxContextLength: 200000,
  },
  "claude-3-sonnet": {
    label: "Claude 3 Sonnet",
    maxContextLength: 200000,
  },
  "claude-3-haiku": {
    label: "Claude 3 Haiku",
    maxContextLength: 200000,
  },
  "claude-2.1": {
    label: "Claude 2.1",
    maxContextLength: 200000,
  },
  "claude-2.0": {
    label: "Claude 2.0",
    maxContextLength: 100000,
  },
  "claude-instant-1.2": {
    label: "Claude Instant 1.2",
    maxContextLength: 100000,
  },
} as const satisfies Record<string, ModelInfo>;
