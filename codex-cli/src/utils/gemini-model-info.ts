import type { ModelInfo } from "./model-info";

export type SupportedGeminiModelId = keyof typeof geminiModelInfo;

export const geminiModelInfo = {
  "gemini-1.0-pro": {
    label: "Gemini 1.0 Pro",
    maxContextLength: 32000,
  },
  "gemini-1.0-pro-vision": {
    label: "Gemini 1.0 Pro Vision",
    maxContextLength: 16000,
  },
  "gemini-1.5-pro": {
    label: "Gemini 1.5 Pro",
    maxContextLength: 1000000,
  },
  "gemini-1.5-flash": {
    label: "Gemini 1.5 Flash",
    maxContextLength: 1000000,
  },
  "gemini-1.5-pro-dev": {
    label: "Gemini 1.5 Pro Dev",
    maxContextLength: 1000000,
  },
  "gemini-1.5-flash-dev": {
    label: "Gemini 1.5 Flash Dev",
    maxContextLength: 1000000,
  },
} as const satisfies Record<string, ModelInfo>;
