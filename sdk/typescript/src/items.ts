// based on item types from codex-rs/exec/src/exec_events.rs

export type CommandExecutionStatus = "in_progress" | "completed" | "failed";

export type CommandExecutionItem = {
  id: string;
  item_type: "command_execution";
  command: string;
  aggregated_output: string;
  exit_code?: number;
  status: CommandExecutionStatus;
};

export type PatchChangeKind = "add" | "delete" | "update";

export type FileUpdateChange = {
  path: string;
  kind: PatchChangeKind;
};

export type PatchApplyStatus = "completed" | "failed";

export type FileChangeItem = {
  id: string;
  item_type: "file_change";
  changes: FileUpdateChange[];
  status: PatchApplyStatus;
};

export type McpToolCallStatus = "in_progress" | "completed" | "failed";

export type McpToolCallItem = {
  id: string;
  item_type: "mcp_tool_call";
  server: string;
  tool: string;
  status: McpToolCallStatus;
};

export type AssistantMessageItem = {
  id: string;
  item_type: "assistant_message";
  text: string;
};

export type ReasoningItem = {
  id: string;
  item_type: "reasoning";
  text: string;
};

export type WebSearchItem = {
  id: string;
  item_type: "web_search";
  query: string;
};

export type ErrorItem = {
  id: string;
  item_type: "error";
  message: string;
};

export type TodoItem = {
  text: string;
  completed: boolean;
};

export type TodoListItem = {
  id: string;
  item_type: "todo_list";
  items: TodoItem[];
};

export type SessionItem = {
  id: string;
  item_type: "session";
  session_id: string;
};

export type ThreadItem =
  | AssistantMessageItem
  | ReasoningItem
  | CommandExecutionItem
  | FileChangeItem
  | McpToolCallItem
  | WebSearchItem
  | TodoListItem
  | ErrorItem;
