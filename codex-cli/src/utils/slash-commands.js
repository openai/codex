/**
 * @typedef {{ command: string, description: string }} SlashCommand
 */

/**
 * List of available slash commands and their descriptions.
 * Used for autocompletion and help display.
 * @type {SlashCommand[]}
 */
export const SLASH_COMMANDS = [
  { command: "/clear", description: "Clear conversation history and free up context" },
  { command: "/compact", description: "Clear conversation history but keep a summary in context. Optional: /compact [instructions for summarization]" },
  { command: "/config", description: "Open config panel" },
  { command: "/cost", description: "Show the total cost and duration of the current session" },
  { command: "/pr-comments", description: "Get comments from a GitHub pull request" },
  { command: "/mcp", description: "Show MCP server connection status" },
  { command: "/doctor", description: "Checks the health of your Claude Code installation" },
  { command: "/history", description: "Open command history" },
  { command: "/help", description: "Show list of commands" },
  { command: "/model", description: "Open model selection panel" },
  { command: "/approval", description: "Open approval mode selection panel" },
  { command: "/clearhistory", description: "Clear command history" }
];