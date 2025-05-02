import type { FunctionTool } from "openai/resources/responses/responses.mjs";

export const shellToolDefinition: FunctionTool = {
  type: "function",
  name: "shell",
  description: "Runs a shell command, and returns its output.",
  strict: false,
  parameters: {
    type: "object",
    properties: {
      command: { type: "array", items: { type: "string" } },
      workdir: { type: "string" },
      timeout: { type: "number" },
    },
    required: ["command"],
    additionalProperties: false,
  },
};
