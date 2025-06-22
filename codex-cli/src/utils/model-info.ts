export type ModelInfo = {
  /** The human-readable label for this model */
  label: string;
  /** The max context window size for this model */
  maxContextLength: number;
};

export type SupportedModelId = keyof typeof openAiModelInfo;
export const openAiModelInfo = {
  "o1-pro-2025-03-19": {
    label: "o1 Pro (2025-03-19)",
    maxContextLength: 200000,
  },
  "o3": {
    label: "o3",
    maxContextLength: 200000,
  },
  "o3-2025-04-16": {
    label: "o3 (2025-04-16)",
    maxContextLength: 200000,
  },
  "codex-mini-latest": {
    label: "codex-mini-latest",
    maxContextLength: 200000,
  },
  "o4-mini": {
    label: "o4 Mini",
    maxContextLength: 200000,
  },
  "gpt-4.1-nano": {
    label: "GPT-4.1 Nano",
    maxContextLength: 1000000,
  },
  "gpt-4.1-nano-2025-04-14": {
    label: "GPT-4.1 Nano (2025-04-14)",
    maxContextLength: 1000000,
  },
  "o4-mini-2025-04-16": {
    label: "o4 Mini (2025-04-16)",
    maxContextLength: 200000,
  },
  "gpt-4": {
    label: "GPT-4",
    maxContextLength: 8192,
  },
  "o1-preview-2024-09-12": {
    label: "o1 Preview (2024-09-12)",
    maxContextLength: 128000,
  },
  "gpt-4.1-mini": {
    label: "GPT-4.1 Mini",
    maxContextLength: 1000000,
  },
  "gpt-3.5-turbo-instruct-0914": {
    label: "GPT-3.5 Turbo Instruct (0914)",
    maxContextLength: 4096,
  },
  "gpt-4o-mini-search-preview": {
    label: "GPT-4o Mini Search Preview",
    maxContextLength: 128000,
  },
  "gpt-4.1-mini-2025-04-14": {
    label: "GPT-4.1 Mini (2025-04-14)",
    maxContextLength: 1000000,
  },
  "chatgpt-4o-latest": {
    label: "ChatGPT-4o Latest",
    maxContextLength: 128000,
  },
  "gpt-3.5-turbo-1106": {
    label: "GPT-3.5 Turbo (1106)",
    maxContextLength: 16385,
  },
  "gpt-4o-search-preview": {
    label: "GPT-4o Search Preview",
    maxContextLength: 128000,
  },
  "gpt-4-turbo": {
    label: "GPT-4 Turbo",
    maxContextLength: 128000,
  },
  "gpt-4o-realtime-preview-2024-12-17": {
    label: "GPT-4o Realtime Preview (2024-12-17)",
    maxContextLength: 128000,
  },
  "gpt-3.5-turbo-instruct": {
    label: "GPT-3.5 Turbo Instruct",
    maxContextLength: 4096,
  },
  "gpt-3.5-turbo": {
    label: "GPT-3.5 Turbo",
    maxContextLength: 16385,
  },
  "gpt-4-turbo-preview": {
    label: "GPT-4 Turbo Preview",
    maxContextLength: 128000,
  },
  "gpt-4o-mini-search-preview-2025-03-11": {
    label: "GPT-4o Mini Search Preview (2025-03-11)",
    maxContextLength: 128000,
  },
  "gpt-4-0125-preview": {
    label: "GPT-4 (0125) Preview",
    maxContextLength: 128000,
  },
  "gpt-4o-2024-11-20": {
    label: "GPT-4o (2024-11-20)",
    maxContextLength: 128000,
  },
  "o3-mini": {
    label: "o3 Mini",
    maxContextLength: 200000,
  },
  "gpt-4o-2024-05-13": {
    label: "GPT-4o (2024-05-13)",
    maxContextLength: 128000,
  },
  "gpt-4-turbo-2024-04-09": {
    label: "GPT-4 Turbo (2024-04-09)",
    maxContextLength: 128000,
  },
  "gpt-3.5-turbo-16k": {
    label: "GPT-3.5 Turbo 16k",
    maxContextLength: 16385,
  },
  "o3-mini-2025-01-31": {
    label: "o3 Mini (2025-01-31)",
    maxContextLength: 200000,
  },
  "o1-preview": {
    label: "o1 Preview",
    maxContextLength: 128000,
  },
  "o1-2024-12-17": {
    label: "o1 (2024-12-17)",
    maxContextLength: 128000,
  },
  "gpt-4-0613": {
    label: "GPT-4 (0613)",
    maxContextLength: 8192,
  },
  "o1": {
    label: "o1",
    maxContextLength: 128000,
  },
  "o1-pro": {
    label: "o1 Pro",
    maxContextLength: 200000,
  },
  "gpt-4.5-preview": {
    label: "GPT-4.5 Preview",
    maxContextLength: 128000,
  },
  "gpt-4.5-preview-2025-02-27": {
    label: "GPT-4.5 Preview (2025-02-27)",
    maxContextLength: 128000,
  },
  "gpt-4o-search-preview-2025-03-11": {
    label: "GPT-4o Search Preview (2025-03-11)",
    maxContextLength: 128000,
  },
  "gpt-4o": {
    label: "GPT-4o",
    maxContextLength: 128000,
  },
  "gpt-4o-mini": {
    label: "GPT-4o Mini",
    maxContextLength: 128000,
  },
  "gpt-4o-2024-08-06": {
    label: "GPT-4o (2024-08-06)",
    maxContextLength: 128000,
  },
  "gpt-4.1": {
    label: "GPT-4.1",
    maxContextLength: 1000000,
  },
  "gpt-4.1-2025-04-14": {
    label: "GPT-4.1 (2025-04-14)",
    maxContextLength: 1000000,
  },
  "gpt-4o-mini-2024-07-18": {
    label: "GPT-4o Mini (2024-07-18)",
    maxContextLength: 128000,
  },
  "o1-mini": {
    label: "o1 Mini",
    maxContextLength: 128000,
  },
  "gpt-3.5-turbo-0125": {
    label: "GPT-3.5 Turbo (0125)",
    maxContextLength: 16385,
  },
  "o1-mini-2024-09-12": {
    label: "o1 Mini (2024-09-12)",
    maxContextLength: 128000,
  },
  "gpt-4-1106-preview": {
    label: "GPT-4 (1106) Preview",
    maxContextLength: 128000,
  },
} as const satisfies Record<string, ModelInfo>;
