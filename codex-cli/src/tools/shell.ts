/**
 * Definition of the built-in shell tool for Codex server.
 */
export const tools = [
  {
    type: "function",
    name: "shell",
    description: "Runs a shell command, and returns its output.",
    strict: false,
    parameters: {
      type: "object",
      properties: {
        command: { type: "array", items: { type: "string" } },
        workdir: {
          type: "string",
          description: "The working directory for the command.",
        },
        timeout: {
          type: "number",
          description: "The maximum time to wait for the command to complete in milliseconds.",
        },
      },
      required: ["command"],
      additionalProperties: false,
    },
  }
];