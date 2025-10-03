// based on item types from codex-rs/exec/src/exec_events.rs

/** The status of a command execution. */
export type CommandExecutionStatus = "in_progress" | "completed" | "failed";

/** A command executed by the agent. */
export type CommandExecutionItem = {
  id: string;
  type: "command_execution";
  /** The command line executed by the agent. */
  command: string;
  /** Aggregated stdout and stderr captured while the command was running. */
  aggregated_output: string;
  /** Set when the command exits; omitted while still running. */
  exit_code?: number;
  /** Current status of the command execution. */
  status: CommandExecutionStatus;
};

/** Indicates the type of the file change. */
export type PatchChangeKind = "add" | "delete" | "update";

/** A set of file changes by the agent. */
export type FileUpdateChange = {
  path: string;
  kind: PatchChangeKind;
};

/** The status of a file change. */
export type PatchApplyStatus = "completed" | "failed";

/** A set of file changes by the agent. Emitted once the patch succeeds or fails. */
export type FileChangeItem = {
  id: string;
  type: "file_change";
  /** Individual file changes that comprise the patch. */
  changes: FileUpdateChange[];
  /** Whether the patch ultimately succeeded or failed. */
  status: PatchApplyStatus;
};

/** The status of an MCP tool call. */
export type McpToolCallStatus = "in_progress" | "completed" | "failed";

/**
 * Represents a call to an MCP tool. The item starts when the invocation is dispatched
 * and completes when the MCP server reports success or failure.
 */
export type McpToolCallItem = {
  id: string;
  type: "mcp_tool_call";
  /** Name of the MCP server handling the request. */
  server: string;
  /** The tool invoked on the MCP server. */
  tool: string;
  /** Current status of the tool invocation. */
  status: McpToolCallStatus;
};

/** Response from the agent. Either natural-language text or JSON when structured output is requested. */
export type AgentMessageItem = {
  id: string;
  type: "agent_message";
  /** Either natural-language text or JSON when structured output is requested. */
  text: string;
};

/** Agent's reasoning summary. */
export type ReasoningItem = {
  id: string;
  type: "reasoning";
  text: string;
};

/** Captures a web search request. Completes when results are returned to the agent. */
export type WebSearchItem = {
  id: string;
  type: "web_search";
  query: string;
};

/** Describes a non-fatal error surfaced as an item. */
export type ErrorItem = {
  id: string;
  type: "error";
  message: string;
};

/** An item in the agent's to-do list. */
export type TodoItem = {
  text: string;
  completed: boolean;
};

/**
 * Tracks the agent's running to-do list. Starts when the plan is issued, updates as steps change,
 * and completes when the turn ends.
 */
export type TodoListItem = {
  id: string;
  type: "todo_list";
  items: TodoItem[];
};

export type SessionItem = {
  id: string;
  type: "session";
  session_id: string;
};

/** Canonical union of thread items and their type-specific payloads. */
export type ThreadItem =
  | AgentMessageItem
  | ReasoningItem
  | CommandExecutionItem
  | FileChangeItem
  | McpToolCallItem
  | WebSearchItem
  | TodoListItem
  | ErrorItem;
