export type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };

export interface RpcRequest {
  id: number;
  method: string;
  params?: JsonValue;
}

export interface RpcNotification {
  method: string;
  params?: JsonValue;
}

export interface RpcSuccess {
  id: number;
  result: JsonValue;
}

export interface RpcError {
  id: number;
  error: {
    code: number;
    message: string;
    data?: JsonValue;
  };
}

export type IncomingRpc =
  | RpcSuccess
  | RpcError
  | ({ id: number; method: string; params?: JsonValue } & Record<string, JsonValue>)
  | ({ method: string; params?: JsonValue } & Record<string, JsonValue>);

export type ConnectionStatus = "connecting" | "connected" | "disconnected" | "error" | "missing-token";

export interface ThreadSummary {
  id: string;
  title: string;
  preview: string;
  searchText: string;
  updatedAt?: number;
}

export type TimelineKind =
  | "system"
  | "user"
  | "assistant"
  | "reasoning"
  | "command"
  | "file-change"
  | "approval"
  | "notification";

export interface TimelineEntry {
  key: string;
  kind: TimelineKind;
  label: string;
  text: string;
  status?: string;
  meta?: string;
  requestId?: number;
  method?: string;
  createdAt: number;
}

export interface RawLogEntry {
  id: number;
  direction: "in" | "out" | "info" | "error";
  at: number;
  text: string;
}

export interface AppState {
  connection: ConnectionStatus;
  statusText: string;
  initialized: boolean;
  threadId: string | null;
  threads: ThreadSummary[];
  threadsLoaded: boolean;
  threadsNextCursor: string | null;
  threadsLoadingMore: boolean;
  timeline: TimelineEntry[];
  itemToTimeline: Record<string, string>;
  activeApprovals: Record<number, true>;
  logs: RawLogEntry[];
  logPanelOpen: boolean;
  sidebarOpen: boolean;
  streamActive: boolean;
  theme: "dark" | "light";
}

export type AppAction =
  | { type: "set_connection"; connection: ConnectionStatus; statusText: string }
  | { type: "set_initialized"; initialized: boolean }
  | { type: "set_thread"; threadId: string | null }
  | { type: "set_threads"; threads: ThreadSummary[]; nextCursor: string | null }
  | { type: "append_threads"; threads: ThreadSummary[]; nextCursor: string | null }
  | { type: "set_threads_loading_more"; loading: boolean }
  | { type: "clear_timeline" }
  | { type: "upsert_entry"; entry: TimelineEntry }
  | { type: "append_delta"; itemId: string; delta: string }
  | { type: "set_item_key"; itemId: string; key: string }
  | { type: "activate_approval"; requestId: number }
  | { type: "complete_approval"; requestId: number; status: string }
  | { type: "set_log_panel"; open: boolean }
  | { type: "append_log"; log: RawLogEntry }
  | { type: "set_sidebar"; open: boolean }
  | { type: "set_stream_active"; active: boolean }
  | { type: "set_theme"; theme: "dark" | "light" };
