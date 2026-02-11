import { useCallback, useEffect, useReducer, useRef } from "react";
import { ChatFeed } from "./components/ChatFeed";
import { Composer } from "./components/Composer";
import { DevLogPanel } from "./components/DevLogPanel";
import { Sidebar } from "./components/Sidebar";
import { TopBar } from "./components/TopBar";
import { normalizePreview, nowClock } from "./lib/time";
import { appReducer, initialState } from "./state/reducer";
import { JsonValue, TimelineEntry, ThreadSummary } from "./types";

interface AppProps {
  initialPrompt: string;
  token: string;
  backend: string | null;
}

interface PendingRequest {
  method: string;
  reject: (error: Error) => void;
  resolve: (value: unknown) => void;
}

interface ThreadListPage {
  nextCursor: string | null;
  threads: ThreadSummary[];
}

function textContentFromUserInput(input: unknown): string {
  if (!Array.isArray(input)) {
    return "";
  }

  return input
    .filter((value) => typeof value === "object" && value !== null && (value as { type?: string }).type === "text")
    .map((value) => String((value as { text?: string }).text ?? ""))
    .join("");
}

function timelineFromItem(item: Record<string, unknown>): TimelineEntry {
  const itemId = String(item.id ?? "");
  const type = String(item.type ?? "unknown");

  if (type === "userMessage") {
    return {
      key: `item:${itemId}`,
      kind: "user",
      label: "User",
      text: textContentFromUserInput(item.content),
      status: itemId.length > 0 ? `item ${itemId}` : undefined,
      createdAt: Date.now(),
    };
  }

  if (type === "agentMessage") {
    return {
      key: `item:${itemId}`,
      kind: "assistant",
      label: "Assistant",
      text: String(item.text ?? ""),
      status: itemId.length > 0 ? `item ${itemId}` : undefined,
      createdAt: Date.now(),
    };
  }

  if (type === "reasoning") {
    const summary = Array.isArray(item.summary) ? item.summary.join("\n") : "";
    const content = Array.isArray(item.content) ? item.content.join("\n") : "";
    return {
      key: `item:${itemId}`,
      kind: "reasoning",
      label: "Reasoning",
      text: summary || content || "",
      status: itemId.length > 0 ? `item ${itemId}` : undefined,
      createdAt: Date.now(),
    };
  }

  if (type === "commandExecution") {
    const status = String(item.status ?? "inProgress");
    const command = String(item.command ?? "");
    const cwd = String(item.cwd ?? "");
    const output = String(item.aggregatedOutput ?? "");
    return {
      key: `item:${itemId}`,
      kind: "command",
      label: "Command",
      text: `cwd: ${cwd}\ncmd: ${command}\n\n${output}`,
      status,
      createdAt: Date.now(),
    };
  }

  if (type === "fileChange") {
    const status = String(item.status ?? "inProgress");
    const changes = Array.isArray(item.changes) ? item.changes : [];
    const diff = changes
      .map((change) => {
        if (typeof change !== "object" || change === null) {
          return "update";
        }
        const changeObj = change as { kind?: string; path?: string };
        return `${changeObj.kind ?? "update"} ${changeObj.path ?? ""}`.trim();
      })
      .join("\n");

    return {
      key: `item:${itemId}`,
      kind: "file-change",
      label: "File change",
      text: diff || "(no files listed)",
      status,
      createdAt: Date.now(),
    };
  }

  if (type === "webSearch") {
    return {
      key: `item:${itemId}`,
      kind: "notification",
      label: "Web search",
      text: String(item.query ?? ""),
      status: "search",
      createdAt: Date.now(),
    };
  }

  if (type === "plan") {
    return {
      key: `item:${itemId}`,
      kind: "notification",
      label: "Plan",
      text: String(item.text ?? ""),
      status: "draft",
      createdAt: Date.now(),
    };
  }

  return {
    key: `item:${itemId}`,
    kind: "notification",
    label: `Item:${type}`,
    text: JSON.stringify(item),
    createdAt: Date.now(),
  };
}

function normalizeThreads(raw: unknown): ThreadSummary[] {
  if (!Array.isArray(raw)) {
    return [];
  }

  return raw
    .map((item) => {
      if (typeof item !== "object" || item === null) {
        return null;
      }
      const thread = item as Record<string, unknown>;
      const id = String(thread.id ?? "");
      if (id.length === 0) {
        return null;
      }
      const preview = normalizePreview(String(thread.preview ?? ""));
      const title = preview.length > 0 ? preview : "Untitled session";
      const updatedAtRaw = thread.updatedAt ?? thread.updated_at;
      const updatedAt = typeof updatedAtRaw === "number" ? updatedAtRaw : undefined;
      return {
        id,
        title,
        preview,
        searchText: `${title} ${preview}`.toLowerCase(),
        updatedAt,
      };
    })
    .filter((item): item is ThreadSummary => item !== null)
    .sort((left, right) => (right.updatedAt ?? 0) - (left.updatedAt ?? 0));
}

function waitMs(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

export function App({ initialPrompt, token, backend }: AppProps) {
  const [state, dispatch] = useReducer(appReducer, initialState);

  const wsRef = useRef<WebSocket | null>(null);
  const nextRpcIdRef = useRef(1);
  const nextLogIdRef = useRef(1);
  const pendingRef = useRef<Map<number, PendingRequest>>(new Map());
  const threadIdRef = useRef<string | null>(null);
  const activeApprovalsRef = useRef<Record<number, true>>({});
  const initialPromptSentRef = useRef(false);

  useEffect(() => {
    threadIdRef.current = state.threadId;
  }, [state.threadId]);

  useEffect(() => {
    activeApprovalsRef.current = state.activeApprovals;
  }, [state.activeApprovals]);

  useEffect(() => {
    document.documentElement.dataset.theme = state.theme;
  }, [state.theme]);

  const appendLog = useCallback((direction: "in" | "out" | "info" | "error", value: unknown) => {
    const text = typeof value === "string" ? value : JSON.stringify(value);
    dispatch({
      type: "append_log",
      log: {
        id: nextLogIdRef.current++,
        direction,
        at: Date.now(),
        text,
      },
    });
  }, []);

  const appendSystem = useCallback(
    (text: string, status: "ok" | "warn" | "error" = "ok") => {
      dispatch({
        type: "upsert_entry",
        entry: {
          key: `system:${Date.now()}:${Math.random().toString(16).slice(2)}`,
          kind: status === "error" ? "notification" : "system",
          label: status === "error" ? "Error" : "System",
          text,
          status,
          meta: nowClock(),
          createdAt: Date.now(),
        },
      });
    },
    [],
  );

  const rpcSend = useCallback(
    (payload: Record<string, JsonValue>) => {
      const socket = wsRef.current;
      if (!socket || socket.readyState !== WebSocket.OPEN) {
        throw new Error("WebSocket not connected");
      }
      appendLog("out", payload);
      socket.send(JSON.stringify(payload));
    },
    [appendLog],
  );

  const rpcRequest = useCallback(
    (method: string, params: Record<string, JsonValue> | null = null) => {
      const id = nextRpcIdRef.current++;
      const payload: Record<string, JsonValue> = {
        id,
        method,
      };
      if (params !== null) {
        payload.params = params;
      }

      return new Promise<unknown>((resolve, reject) => {
        pendingRef.current.set(id, {
          resolve,
          reject,
          method,
        });

        try {
          rpcSend(payload);
        } catch (error) {
          pendingRef.current.delete(id);
          reject(error);
        }
      });
    },
    [rpcSend],
  );

  const rpcNotify = useCallback(
    (method: string, params: Record<string, JsonValue> | null = null) => {
      const payload: Record<string, JsonValue> = { method };
      if (params !== null) {
        payload.params = params;
      }
      rpcSend(payload);
    },
    [rpcSend],
  );

  const fetchThreadsPage = useCallback(
    async (cursor: string | null = null): Promise<ThreadListPage> => {
      const params: Record<string, JsonValue> = {
        limit: 30,
        modelProviders: [],
        sourceKinds: ["cli", "vscode", "appServer", "exec"],
        sortKey: "updated_at",
      };
      if (cursor) {
        params.cursor = cursor;
      }

      const result = (await rpcRequest("thread/list", params)) as {
        data?: unknown;
        nextCursor?: unknown;
        next_cursor?: unknown;
      };

      const nextCursorValue = result?.nextCursor ?? result?.next_cursor;
      return {
        threads: normalizeThreads(result?.data),
        nextCursor: typeof nextCursorValue === "string" && nextCursorValue.length > 0 ? nextCursorValue : null,
      };
    },
    [rpcRequest],
  );

  const refreshThreads = useCallback(async () => {
    const page = await fetchThreadsPage();
    dispatch({
      type: "set_threads",
      threads: page.threads,
      nextCursor: page.nextCursor,
    });
    return page.threads;
  }, [fetchThreadsPage]);

  const loadMoreThreads = useCallback(async () => {
    if (!state.threadsNextCursor || state.threadsLoadingMore) {
      return;
    }

    dispatch({ type: "set_threads_loading_more", loading: true });
    try {
      const page = await fetchThreadsPage(state.threadsNextCursor);
      dispatch({
        type: "append_threads",
        threads: page.threads,
        nextCursor: page.nextCursor,
      });
    } catch (error) {
      dispatch({ type: "set_threads_loading_more", loading: false });
      appendSystem(`thread/list failed: ${String((error as Error).message ?? error)}`, "warn");
    }
  }, [appendSystem, fetchThreadsPage, state.threadsLoadingMore, state.threadsNextCursor]);

  const refreshThreadsWithRetry = useCallback(async () => {
    let lastError: Error | null = null;
    for (const delayMs of [0, 200, 700, 1400]) {
      if (delayMs > 0) {
        await waitMs(delayMs);
      }
      try {
        await refreshThreads();
        return;
      } catch (error) {
        lastError = error as Error;
      }
    }

    throw lastError ?? new Error("thread/list failed");
  }, [refreshThreads]);

  const renderThreadHistory = useCallback((thread: unknown) => {
    if (typeof thread !== "object" || thread === null) {
      return;
    }

    const turns = (thread as { turns?: unknown }).turns;
    if (!Array.isArray(turns)) {
      return;
    }

    for (const turn of turns) {
      if (typeof turn !== "object" || turn === null) {
        continue;
      }
      const items = (turn as { items?: unknown }).items;
      if (!Array.isArray(items)) {
        continue;
      }
      for (const item of items) {
        if (typeof item !== "object" || item === null) {
          continue;
        }
        const entry = timelineFromItem(item as Record<string, unknown>);
        dispatch({ type: "upsert_entry", entry });

        const itemId = String((item as { id?: unknown }).id ?? "");
        if (itemId.length > 0) {
          dispatch({ type: "set_item_key", itemId, key: entry.key });
        }
      }
    }
  }, []);

  const startNewThread = useCallback(async () => {
    dispatch({ type: "clear_timeline" });
    try {
      const result = (await rpcRequest("thread/start", {})) as {
        thread?: { id?: string };
      };
      const threadId = result.thread?.id ?? null;
      dispatch({ type: "set_thread", threadId });
      appendSystem("thread started", "ok");
      try {
        await refreshThreads();
      } catch (error) {
        appendSystem(`thread/list failed: ${String((error as Error).message ?? error)}`, "warn");
      }
    } catch (error) {
      appendSystem(`thread/start failed: ${String((error as Error).message ?? error)}`, "error");
    }
  }, [appendSystem, refreshThreads, rpcRequest]);

  const resumeThread = useCallback(
    async (threadId: string) => {
      if (threadId.trim().length === 0) {
        return;
      }

      dispatch({ type: "clear_timeline" });
      try {
        const result = (await rpcRequest("thread/resume", {
          threadId,
        })) as {
          thread?: { id?: string } & Record<string, unknown>;
        };

        dispatch({ type: "set_thread", threadId: result.thread?.id ?? threadId });
        appendSystem(`resumed ${threadId}`, "ok");
        renderThreadHistory(result.thread);
        try {
          await refreshThreads();
        } catch (error) {
          appendSystem(`thread/list failed: ${String((error as Error).message ?? error)}`, "warn");
        }
      } catch (error) {
        appendSystem(`thread/resume failed: ${String((error as Error).message ?? error)}`, "error");
      }
    },
    [appendSystem, refreshThreads, renderThreadHistory, rpcRequest],
  );

  const sendPrompt = useCallback(
    async (text: string) => {
      const threadId = threadIdRef.current;
      if (!threadId) {
        appendSystem("No active thread. Start or resume a thread first.", "warn");
        return;
      }

      const trimmed = text.trim();
      if (trimmed.length === 0) {
        return;
      }

      const input: JsonValue[] = [
        {
          type: "text",
          text: trimmed,
          textElements: [],
        },
      ];

      try {
        await rpcRequest("turn/start", {
          threadId,
          input,
        });
      } catch (error) {
        appendSystem(`turn/start failed: ${String((error as Error).message ?? error)}`, "error");
      }
    },
    [appendSystem, rpcRequest],
  );

  const handleApprovalDecision = useCallback(
    (requestId: number, decision: "accept" | "acceptForSession" | "decline" | "cancel") => {
      try {
        rpcSend({
          id: requestId,
          result: {
            decision,
          },
        });
        dispatch({
          type: "complete_approval",
          requestId,
          status: `sent: ${decision}`,
        });
      } catch (error) {
        appendSystem(`failed to respond to approval: ${String((error as Error).message ?? error)}`, "error");
      }
    },
    [appendSystem, rpcSend],
  );

  const handleServerRequest = useCallback(
    (payload: Record<string, unknown>) => {
      const method = String(payload.method ?? "");
      const requestId = payload.id;
      const params = (payload.params ?? {}) as Record<string, unknown>;

      if (typeof requestId !== "number") {
        appendSystem(`unsupported server request id for ${method}`, "warn");
        return;
      }

      if (method === "item/commandExecution/requestApproval" || method === "item/fileChange/requestApproval") {
        if (activeApprovalsRef.current[requestId]) {
          return;
        }

        dispatch({ type: "activate_approval", requestId });

        const isCommand = method.includes("commandExecution");
        const reason = String(params.reason ?? "Approval requested.");
        const detail = isCommand
          ? `command: ${String(params.command ?? "(unknown)")}\ncwd: ${String(params.cwd ?? "(unknown)")}`
          : `itemId: ${String(params.itemId ?? "")}`;

        dispatch({
          type: "upsert_entry",
          entry: {
            key: `approval:${requestId}`,
            kind: "approval",
            label: isCommand ? "Command approval" : "File change approval",
            text: `${reason}\n\n${detail}`,
            status: "approval",
            requestId,
            method,
            createdAt: Date.now(),
          },
        });
        return;
      }

      dispatch({
        type: "upsert_entry",
        entry: {
          key: `serverreq:${requestId}:${method}`,
          kind: "notification",
          label: "Server request",
          text: JSON.stringify(params),
          status: method,
          createdAt: Date.now(),
        },
      });

      try {
        rpcSend({
          id: requestId,
          error: {
            code: -32601,
            message: `Unimplemented client handler for ${method}`,
          },
        });
      } catch {
        appendSystem(`failed to reject unimplemented method ${method}`, "warn");
      }
    },
    [appendSystem, rpcSend],
  );

  const handleNotification = useCallback(
    (payload: Record<string, unknown>) => {
      const method = String(payload.method ?? "");
      const params = (payload.params ?? {}) as Record<string, unknown>;

      if (method === "thread/started") {
        const threadObj = params.thread as { id?: string } | undefined;
        dispatch({ type: "set_thread", threadId: threadObj?.id ?? null });
        refreshThreads().catch((error) => {
          appendSystem(`thread/list failed: ${String((error as Error).message ?? error)}`, "warn");
        });
        return;
      }

      if (method === "turn/started") {
        dispatch({ type: "set_stream_active", active: true });
        appendSystem("turn started", "ok");
        return;
      }

      if (method === "turn/completed") {
        dispatch({ type: "set_stream_active", active: false });
        appendSystem("turn completed", "ok");
        refreshThreads().catch((error) => {
          appendSystem(`thread/list failed: ${String((error as Error).message ?? error)}`, "warn");
        });
        return;
      }

      if (method === "turn/failed" || method === "error") {
        dispatch({ type: "set_stream_active", active: false });
        appendSystem(`${method}: ${JSON.stringify(params.error ?? params)}`, "error");
        refreshThreads().catch((error) => {
          appendSystem(`thread/list failed: ${String((error as Error).message ?? error)}`, "warn");
        });
        return;
      }

      if (method === "item/started" || method === "item/completed") {
        const item = params.item;
        if (typeof item === "object" && item !== null) {
          const entry = timelineFromItem(item as Record<string, unknown>);
          dispatch({ type: "upsert_entry", entry });
          const itemId = String((item as { id?: unknown }).id ?? "");
          if (itemId.length > 0) {
            dispatch({ type: "set_item_key", itemId, key: entry.key });
          }
        }
        return;
      }

      if (
        method === "item/agentMessage/delta" ||
        method === "item/commandExecution/outputDelta" ||
        method === "item/fileChange/outputDelta"
      ) {
        const itemId = String(params.itemId ?? "");
        if (itemId.length > 0) {
          dispatch({ type: "append_delta", itemId, delta: String(params.delta ?? "") });
          dispatch({ type: "set_stream_active", active: true });
        }
        return;
      }

      dispatch({
        type: "upsert_entry",
        entry: {
          key: `notification:${Date.now()}:${method}`,
          kind: "notification",
          label: method,
          text: JSON.stringify(params),
          status: "notif",
          createdAt: Date.now(),
        },
      });
    },
    [appendSystem, refreshThreads],
  );

  const initialize = useCallback(async () => {
    await rpcRequest("initialize", {
      clientInfo: {
        name: "codex_companion",
        title: "Codex Companion",
        version: "0.0.0-dev",
      },
      capabilities: {
        experimentalApi: true,
      },
    });

    rpcNotify("initialized");
    dispatch({ type: "set_initialized", initialized: true });
    try {
      await refreshThreadsWithRetry();
    } catch (error) {
      appendSystem(`thread/list failed: ${String((error as Error).message ?? error)}`, "warn");
    }
    await startNewThread();

    if (initialPrompt.trim().length > 0 && !initialPromptSentRef.current) {
      initialPromptSentRef.current = true;
      await sendPrompt(initialPrompt.trim());
    }
  }, [appendSystem, initialPrompt, refreshThreadsWithRetry, rpcNotify, rpcRequest, sendPrompt, startNewThread]);

  const connect = useCallback(() => {
    if (token.length === 0) {
      dispatch({
        type: "set_connection",
        connection: "missing-token",
        statusText: "missing token",
      });
      appendSystem("Missing token. Use the URL printed by `codex --companion`.", "error");
      return;
    }

    dispatch({
      type: "set_connection",
      connection: "connecting",
      statusText: "connecting",
    });

    let backendUrl: URL;
    try {
      backendUrl = backend ? new URL(backend) : new URL(window.location.origin);
    } catch {
      dispatch({
        type: "set_connection",
        connection: "error",
        statusText: "bad backend URL",
      });
      appendSystem("Invalid backend URL in `backend` query param.", "error");
      return;
    }
    const protocol = backendUrl.protocol === "https:" ? "wss:" : "ws:";
    const wsUrl = `${protocol}//${backendUrl.host}/ws?token=${encodeURIComponent(token)}`;
    const socket = new WebSocket(wsUrl);
    wsRef.current = socket;

    socket.addEventListener("open", () => {
      dispatch({
        type: "set_connection",
        connection: "connected",
        statusText: "connected",
      });

      initialize().catch((error) => {
        appendSystem(`initialize failed: ${String((error as Error).message ?? error)}`, "error");
      });
    });

    socket.addEventListener("message", (event) => {
      let payload: Record<string, unknown>;
      try {
        payload = JSON.parse(String(event.data)) as Record<string, unknown>;
      } catch (error) {
        appendSystem(`bad JSON from server: ${String(error)}`, "error");
        return;
      }

      appendLog("in", payload);

      if (typeof payload.id === "number" && ("result" in payload || "error" in payload) && !("method" in payload)) {
        const pending = pendingRef.current.get(payload.id);
        pendingRef.current.delete(payload.id);
        if (!pending) {
          return;
        }

        if ("error" in payload) {
          const rpcError = payload.error as { message?: string } | undefined;
          pending.reject(new Error(rpcError?.message ?? "RPC error"));
        } else {
          pending.resolve(payload.result);
        }
        return;
      }

      if (typeof payload.id === "number" && typeof payload.method === "string") {
        handleServerRequest(payload);
        return;
      }

      if (typeof payload.method === "string") {
        handleNotification(payload);
      }
    });

    socket.addEventListener("close", () => {
      dispatch({
        type: "set_connection",
        connection: "disconnected",
        statusText: "disconnected",
      });
      dispatch({ type: "set_initialized", initialized: false });
      dispatch({ type: "set_stream_active", active: false });
    });

    socket.addEventListener("error", () => {
      dispatch({
        type: "set_connection",
        connection: "error",
        statusText: "socket error",
      });
      appendLog("error", "socket error");
    });
  }, [appendLog, appendSystem, backend, handleNotification, handleServerRequest, initialize, token]);

  const reconnect = useCallback(() => {
    if (wsRef.current) {
      wsRef.current.close();
    }

    for (const pending of pendingRef.current.values()) {
      pending.reject(new Error(`connection reset while waiting for ${pending.method}`));
    }
    pendingRef.current.clear();
    nextRpcIdRef.current = 1;
    connect();
  }, [connect]);

  useEffect(() => {
    connect();

    return () => {
      if (wsRef.current) {
        wsRef.current.close();
      }
      for (const pending of pendingRef.current.values()) {
        pending.reject(new Error("connection closed"));
      }
      pendingRef.current.clear();
    };
  }, [connect]);

  useEffect(() => {
    if (state.connection !== "connected" || !state.initialized) {
      return;
    }

    const intervalId = window.setInterval(() => {
      refreshThreads().catch(() => undefined);
    }, 5000);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [refreshThreads, state.connection, state.initialized]);

  return (
    <div className="app-shell">
      <TopBar
        connection={state.connection}
        logOpen={state.logPanelOpen}
        onReconnect={reconnect}
        onToggleLog={() => dispatch({ type: "set_log_panel", open: !state.logPanelOpen })}
        onToggleSidebar={() => dispatch({ type: "set_sidebar", open: !state.sidebarOpen })}
        onToggleTheme={() => dispatch({ type: "set_theme", theme: state.theme === "dark" ? "light" : "dark" })}
        sidebarOpen={state.sidebarOpen}
        statusText={state.statusText}
        streamActive={state.streamActive}
        theme={state.theme}
        threadId={state.threadId}
      />

      <div className="app-content">
        <Sidebar
          activeThreadId={state.threadId}
          onDismissMobile={() => dispatch({ type: "set_sidebar", open: false })}
          onLoadMore={() => {
            loadMoreThreads().catch(() => undefined);
          }}
          onNewThread={() => {
            dispatch({ type: "set_sidebar", open: false });
            startNewThread().catch(() => undefined);
          }}
          onRefresh={() => {
            refreshThreads().catch((error) => {
              appendSystem(`thread/list failed: ${String((error as Error).message ?? error)}`, "warn");
            });
          }}
          onResumeThread={(id) => {
            resumeThread(id).catch(() => undefined);
          }}
          loadingMore={state.threadsLoadingMore}
          nextCursor={state.threadsNextCursor}
          open={state.sidebarOpen}
          threads={state.threads}
          threadsLoaded={state.threadsLoaded}
        />

        <main className="main-panel">
          <ChatFeed
            activeApprovals={state.activeApprovals}
            entries={state.timeline}
            onApprovalDecision={handleApprovalDecision}
          />
          <Composer
            disabled={state.connection !== "connected" || !state.threadId || !state.initialized}
            onSubmit={(text) => {
              sendPrompt(text).catch(() => undefined);
            }}
          />
        </main>
      </div>

      <DevLogPanel
        logs={state.logs}
        onClose={() => dispatch({ type: "set_log_panel", open: false })}
        open={state.logPanelOpen}
      />

      {state.sidebarOpen ? (
        <button
          aria-label="Close sessions sidebar"
          className="sidebar-backdrop"
          onClick={() => dispatch({ type: "set_sidebar", open: false })}
          type="button"
        />
      ) : null}
    </div>
  );
}
