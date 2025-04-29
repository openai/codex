import type { ModelInfo } from "./model-info";

export type SupportedGeminiModelId = keyof typeof geminiModelInfo;

export const geminiModelInfo = {
  // Gemini 2.5 Models
  "gemini-2.5-pro-exp-03-25": {
    label: "Gemini 2.5 Pro Experimental (03-25)",
    maxContextLength: 1048576,
  },
  "gemini-2.5-pro-preview-03-25": {
    label: "Gemini 2.5 Pro Preview (03-25)",
    maxContextLength: 1048576,
  },
  "gemini-2.5-flash-preview-04-17": {
    label: "Gemini 2.5 Flash Preview (04-17)",
    maxContextLength: 1048576,
  },

  // Gemini 2.0 Models
  "gemini-2.0-flash-001": {
    label: "Gemini 2.0 Flash",
    maxContextLength: 1048576,
  },
  "gemini-2.0-flash-lite-preview-02-05": {
    label: "Gemini 2.0 Flash Lite Preview (02-05)",
    maxContextLength: 1048576,
  },
  "gemini-2.0-pro-exp-02-05": {
    label: "Gemini 2.0 Pro Experimental (02-05)",
    maxContextLength: 2097152,
  },
  "gemini-2.0-flash-thinking-exp-01-21": {
    label: "Gemini 2.0 Flash Thinking Experimental (01-21)",
    maxContextLength: 1048576,
  },
  "gemini-2.0-flash-thinking-exp-1219": {
    label: "Gemini 2.0 Flash Thinking Experimental (1219)",
    maxContextLength: 32767,
  },
  "gemini-2.0-flash-exp": {
    label: "Gemini 2.0 Flash Experimental",
    maxContextLength: 1048576,
  },

  // Gemini 1.5 Models
  "gemini-1.5-pro": {
    label: "Gemini 1.5 Pro",
    maxContextLength: 2097152,
  },
  "gemini-1.5-flash": {
    label: "Gemini 1.5 Flash",
    maxContextLength: 1048576,
  },
  "gemini-1.5-pro-dev": {
    label: "Gemini 1.5 Pro Dev",
    maxContextLength: 2097152,
  },
  "gemini-1.5-flash-dev": {
    label: "Gemini 1.5 Flash Dev",
    maxContextLength: 1048576,
  },
  "gemini-1.5-flash-exp-0827": {
    label: "Gemini 1.5 Flash Experimental (0827)",
    maxContextLength: 1048576,
  },
  "gemini-1.5-flash-8b-exp-0827": {
    label: "Gemini 1.5 Flash 8B Experimental (0827)",
    maxContextLength: 1048576,
  },
  "gemini-1.5-pro-002": {
    label: "Gemini 1.5 Pro (002)",
    maxContextLength: 2097152,
  },
  "gemini-1.5-pro-exp-0827": {
    label: "Gemini 1.5 Pro Experimental (0827)",
    maxContextLength: 2097152,
  },

  // Gemini 1.0 Models
  "gemini-1.0-pro": {
    label: "Gemini 1.0 Pro",
    maxContextLength: 32000,
  },
  "gemini-1.0-pro-vision": {
    label: "Gemini 1.0 Pro Vision",
    maxContextLength: 16000,
  },

  // Experimental Models
  "gemini-exp-1206": {
    label: "Gemini Experimental (1206)",
    maxContextLength: 2097152,
  },
} as const satisfies Record<string, ModelInfo>;
