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
  { command: "/clearhistory", description: "Clear command history" },
  { command: "/compact", description: "Clear conversation history but keep a summary in context. Optional: /compact [instructions for summarization]" },
  { command: "/history", description: "Open command history" },
  { command: "/help", description: "Show list of commands" },
  { command: "/model", description: "Open model selection panel" },
  { command: "/approval", description: "Open approval mode selection panel" },
];