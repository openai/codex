import { useCallback, useEffect, useMemo, useRef, useState } from "react";

type RpcError = {
  message?: string;
  [key: string]: unknown;
};

type RpcMessage = {
  id?: number | string;
  method?: string;
  params?: Record<string, unknown>;
  result?: Record<string, unknown>;
  error?: RpcError;
};

type LogEntry = {
  id: number;
  direction: "in" | "out";
  label: string;
  detail?: string;
  time: string;
};

type ChatMessage = {
  id: string;
  role: "user" | "assistant";
  text: string;
};

type ApprovalRequest = {
  id: number | string;
  method: string;
  params: Record<string, unknown>;
  receivedAt: string;
};

type PendingRequest = {
  method: string;
};

const wsUrl = import.meta.env.VITE_APP_SERVER_WS ?? "ws://localhost:8787";

const formatTime = () =>
  new Date().toLocaleTimeString("en-US", {
    hour12: false,
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });

const summarizeParams = (params: Record<string, unknown> | undefined) => {
  if (!params) {
    return undefined;
  }

  try {
    return JSON.stringify(params);
  } catch {
    return "[unserializable params]";
  }
};

const shouldLogMethod = (method: string) => {
  if (method.includes("/delta")) {
    return false;
  }

  return true;
};

export default function App() {
  const wsRef = useRef<WebSocket | null>(null);
  const nextIdRef = useRef(1);
  const pendingRef = useRef<Map<number | string, PendingRequest>>(new Map());
  const agentIndexRef = useRef<Map<string, Map<string, number>>>(new Map());
  const selectedThreadIdRef = useRef<string | null>(null);
  const userItemIdsRef = useRef<Map<string, Set<string>>>(new Map());

  const [connected, setConnected] = useState(false);
  const [initialized, setInitialized] = useState(false);
  const [threads, setThreads] = useState<string[]>([]);
  const [selectedThreadId, setSelectedThreadId] = useState<string | null>(null);
  const [activeTurnId, setActiveTurnId] = useState<string | null>(null);
  const [activeTurnThreadId, setActiveTurnThreadId] = useState<string | null>(null);
  const [input, setInput] = useState("");
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [threadMessages, setThreadMessages] = useState<Record<string, ChatMessage[]>>({});
  const [approvals, setApprovals] = useState<ApprovalRequest[]>([]);
  const [connectionError, setConnectionError] = useState<string | null>(null);

  const pushLog = useCallback((entry: Omit<LogEntry, "id" | "time">) => {
    setLogs((prev) => {
      const next: LogEntry[] = [
        {
          id: prev.length ? prev[0].id + 1 : 1,
          time: formatTime(),
          ...entry,
        },
        ...prev,
      ];
      return next.slice(0, 200);
    });
  }, []);

  const sendPayload = useCallback(
    (payload: RpcMessage, label?: string) => {
      const socket = wsRef.current;
      if (!socket || socket.readyState !== WebSocket.OPEN) {
        return;
      }

      const json = JSON.stringify(payload);
      socket.send(json);
      pushLog({
        direction: "out",
        label: label ?? payload.method ?? "response",
        detail: summarizeParams(payload.params) ?? summarizeParams(payload.result),
      });
    },
    [pushLog],
  );

  const sendRequest = useCallback(
    (method: string, params?: Record<string, unknown>) => {
      const id = nextIdRef.current++;
      pendingRef.current.set(id, { method });
      sendPayload({ id, method, params }, method);
      return id;
    },
    [sendPayload],
  );

  const sendNotification = useCallback(
    (method: string, params?: Record<string, unknown>) => {
      sendPayload({ method, params }, method);
    },
    [sendPayload],
  );

  const selectThread = useCallback((threadId: string | null) => {
    selectedThreadIdRef.current = threadId;
    setSelectedThreadId(threadId);
  }, []);

  const ensureThread = useCallback(
    (threadId: string) => {
      setThreads((prev) => (prev.includes(threadId) ? prev : [...prev, threadId]));
      setThreadMessages((prev) => (prev[threadId] ? prev : { ...prev, [threadId]: [] }));
      if (!selectedThreadIdRef.current) {
        selectThread(threadId);
      }
    },
    [selectThread],
  );

  const getAgentIndexForThread = useCallback((threadId: string) => {
    const existing = agentIndexRef.current.get(threadId);
    if (existing) {
      return existing;
    }
    const next = new Map<string, number>();
    agentIndexRef.current.set(threadId, next);
    return next;
  }, []);

  const handleInitialize = useCallback(() => {
    sendRequest("initialize", {
      clientInfo: {
        name: "codex_app_server_ui",
        title: "Codex App Server UI",
        version: "0.1.0",
      },
    });
  }, [sendRequest]);

  const handleStartThread = useCallback(() => {
    if (!initialized) {
      return;
    }

    sendRequest("thread/start", {});
  }, [initialized, sendRequest]);

  const handleSendMessage = useCallback(() => {
    if (!initialized || !selectedThreadId || !input.trim()) {
      return;
    }

    const text = input.trim();
    setInput("");

    sendRequest("turn/start", {
      threadId: selectedThreadId,
      input: [{ type: "text", text }],
    });
  }, [initialized, selectedThreadId, input, sendRequest]);

  const handleApprovalDecision = useCallback(
    (approvalId: number | string, decision: "accept" | "decline") => {
      sendPayload({
        id: approvalId,
        result: {
          decision,
        },
      });

      setApprovals((prev) => prev.filter((approval) => approval.id !== approvalId));
    },
    [sendPayload],
  );

  const updateAgentMessage = useCallback(
    (threadId: string, itemId: string, delta: string) => {
      setThreadMessages((prev) => {
        const threadLog = prev[threadId] ?? [];
        const indexMap = getAgentIndexForThread(threadId);
        const existingIndex = indexMap.get(itemId);
        if (existingIndex === undefined) {
          indexMap.set(itemId, threadLog.length);
          return {
            ...prev,
            [threadId]: [...threadLog, { id: itemId, role: "assistant", text: delta }],
          };
        }

        const nextThreadLog = [...threadLog];
        nextThreadLog[existingIndex] = {
          ...nextThreadLog[existingIndex],
          text: nextThreadLog[existingIndex].text + delta,
        };
        return { ...prev, [threadId]: nextThreadLog };
      });
    },
    [getAgentIndexForThread],
  );

  const extractUserText = useCallback((content: unknown) => {
    if (!Array.isArray(content)) {
      return null;
    }
    const parts = content
      .map((entry) => {
        if (entry && typeof entry === "object" && (entry as { type?: string }).type === "text") {
          return (entry as { text?: string }).text ?? "";
        }
        return "";
      })
      .filter((text) => text.length > 0);
    return parts.length ? parts.join("\n") : null;
  }, []);

  const markUserItemSeen = useCallback((threadId: string, itemId: string) => {
    const seen = userItemIdsRef.current.get(threadId) ?? new Set<string>();
    if (!userItemIdsRef.current.has(threadId)) {
      userItemIdsRef.current.set(threadId, seen);
    }
    if (seen.has(itemId)) {
      return false;
    }
    seen.add(itemId);
    return true;
  }, []);

  const handleIncomingMessage = useCallback(
    (message: RpcMessage) => {
      if (message.id !== undefined && message.method) {
        const requestId = message.id;
        if (
          message.method === "item/commandExecution/requestApproval" ||
          message.method === "item/fileChange/requestApproval"
        ) {
          setApprovals((prev) => [
            ...prev,
            {
              id: requestId,
              method: message.method ?? "",
              params: message.params ?? {},
              receivedAt: formatTime(),
            },
          ]);
          pushLog({
            direction: "in",
            label: message.method,
            detail: summarizeParams(message.params),
          });
        }
        return;
      }

      if (message.id !== undefined) {
        const pending = pendingRef.current.get(message.id);
        pendingRef.current.delete(message.id);

        if (pending) {
          if (pending.method === "initialize") {
            const errorMessage =
              message.error && typeof message.error.message === "string"
                ? message.error.message
                : null;
            const alreadyInitialized = errorMessage === "Already initialized";
            if (!message.error) {
              sendNotification("initialized");
            }
            if (!message.error || alreadyInitialized) {
              setInitialized(true);
              sendRequest("thread/loaded/list");
            }
          }

          if (pending.method === "thread/start" || pending.method === "thread/resume") {
            const thread = message.result?.thread as { id?: string } | undefined;
            if (thread?.id) {
              ensureThread(thread.id);
            }
          }

          if (pending.method === "thread/loaded/list") {
            const data = message.result?.data;
            if (Array.isArray(data)) {
              const ids = data.filter((entry): entry is string => typeof entry === "string");
              setThreads(ids);
              setThreadMessages((prev) => {
                const next = { ...prev };
                for (const id of ids) {
                  if (!next[id]) {
                    next[id] = [];
                  }
                }
                return next;
              });
              if (!selectedThreadIdRef.current && ids.length > 0) {
                selectThread(ids[0]);
              }
            }
          }

          if (pending.method === "turn/start") {
            const turn = message.result?.turn as { id?: string } | undefined;
            if (turn?.id) {
              setActiveTurnId(turn.id);
              setActiveTurnThreadId(selectedThreadIdRef.current);
            }
          }
        }

        pushLog({
          direction: "in",
          label: pending?.method ?? "response",
          detail: summarizeParams(message.result) ?? summarizeParams(message.error),
        });
        return;
      }

      if (message.method) {
        const eventThreadId = message.params?.threadId as string | undefined;
        if (eventThreadId) {
          ensureThread(eventThreadId);
        }

        if (shouldLogMethod(message.method)) {
          pushLog({
            direction: "in",
            label: message.method,
            detail: summarizeParams(message.params),
          });
        }

        if (message.method === "thread/started") {
          const thread = (message.params?.thread as { id?: string } | undefined) ?? undefined;
          if (thread?.id) {
            ensureThread(thread.id);
          }
        }

        if (message.method === "turn/started") {
          const turn = (message.params?.turn as { id?: string } | undefined) ?? undefined;
          const threadId = message.params?.threadId as string | undefined;
          if (turn?.id) {
            setActiveTurnId(turn.id);
            setActiveTurnThreadId(threadId ?? null);
          }
        }

        if (message.method === "turn/completed") {
          setActiveTurnId(null);
          setActiveTurnThreadId(null);
        }

        if (message.method === "item/started") {
          const item = message.params?.item as {
            id?: string;
            type?: string;
            content?: unknown;
            text?: string;
          } | undefined;
          const threadId = message.params?.threadId as string | undefined;
          if (!threadId) {
            return;
          }
          const itemId = item?.id;
          if (item?.type === "agentMessage" && itemId) {
            setThreadMessages((prev) => {
              const threadLog = prev[threadId] ?? [];
              const indexMap = getAgentIndexForThread(threadId);
              indexMap.set(itemId, threadLog.length);
              return {
                ...prev,
                [threadId]: [...threadLog, { id: itemId, role: "assistant", text: "" }],
              };
            });
          }

          if (item?.type === "userMessage") {
            const userText = extractUserText(item.content);
            if (userText) {
              if (itemId && !markUserItemSeen(threadId, itemId)) {
                return;
              }
              setThreadMessages((prev) => {
                const threadLog = prev[threadId] ?? [];
                return {
                  ...prev,
                  [threadId]: [
                    ...threadLog,
                    { id: itemId ?? `user-${Date.now()}`, role: "user", text: userText },
                  ],
                };
              });
            }
          }
        }

        if (message.method === "item/agentMessage/delta") {
          const itemId = message.params?.itemId as string | undefined;
          const threadId = message.params?.threadId as string | undefined;
          const delta = message.params?.delta as string | undefined;
          if (itemId && delta && threadId) {
            updateAgentMessage(threadId, itemId, delta);
          }
        }

        if (message.method === "item/completed") {
          const item = message.params?.item as {
            id?: string;
            type?: string;
            text?: string;
            content?: unknown;
          } | undefined;
          const threadId = message.params?.threadId as string | undefined;
          if (!threadId) {
            return;
          }
          const itemId = item?.id;
          if (item?.type === "agentMessage" && itemId && typeof item.text === "string") {
            setThreadMessages((prev) => {
              const threadLog = prev[threadId] ?? [];
              const index = getAgentIndexForThread(threadId).get(itemId);
              if (index === undefined) {
                getAgentIndexForThread(threadId).set(itemId, threadLog.length);
                return {
                  ...prev,
                  [threadId]: [...threadLog, { id: itemId, role: "assistant", text: item.text ?? "" }],
                };
              }

              const nextThreadLog = [...threadLog];
              nextThreadLog[index] = { ...nextThreadLog[index], text: item.text ?? "" };
              return { ...prev, [threadId]: nextThreadLog };
            });
          }

          if (item?.type === "userMessage") {
            return;
          }
        }
      }
    },
    [
      ensureThread,
      extractUserText,
      getAgentIndexForThread,
      markUserItemSeen,
      pushLog,
      sendNotification,
      selectThread,
      sendRequest,
      updateAgentMessage,
    ],
  );

  const connect = useCallback(() => {
    if (wsRef.current) {
      wsRef.current.close();
    }

    const socket = new WebSocket(wsUrl);
    wsRef.current = socket;

    socket.onopen = () => {
      setConnected(true);
      setConnectionError(null);
      handleInitialize();
    };

    socket.onclose = () => {
      setConnected(false);
      setInitialized(false);
      setThreads([]);
      selectThread(null);
      setActiveTurnId(null);
      setActiveTurnThreadId(null);
      setApprovals([]);
      setThreadMessages({});
      agentIndexRef.current.clear();
      userItemIdsRef.current.clear();
      pendingRef.current.clear();
    };

    socket.onerror = () => {
      setConnectionError("WebSocket error. Check the bridge server.");
    };

    socket.onmessage = (event) => {
      try {
        const parsed = JSON.parse(event.data as string) as RpcMessage;
        handleIncomingMessage(parsed);
      } catch (err) {
        pushLog({
          direction: "in",
          label: "ui/error",
          detail: `Failed to parse message: ${String(err)}`,
        });
      }
    };
  }, [handleIncomingMessage, handleInitialize, pushLog, selectThread]);

  useEffect(() => {
    connect();

    return () => {
      wsRef.current?.close();
      wsRef.current = null;
    };
  }, [connect]);

  const statusLabel = useMemo(() => {
    if (!connected) {
      return "Disconnected";
    }

    if (!initialized) {
      return "Connecting";
    }

    return "Ready";
  }, [connected, initialized]);

  const activeMessages = selectedThreadId ? threadMessages[selectedThreadId] ?? [] : [];
  const displayedTurnId =
    selectedThreadId && activeTurnThreadId === selectedThreadId ? activeTurnId : null;

  return (
    <div className="app">
      <header className="hero">
        <div className="status">
          <span className={`status-dot ${connected ? "on" : "off"}`} />
          <div>
            <div className="status-label">{statusLabel}</div>
            <div className="status-meta">{wsUrl}</div>
          </div>
        </div>
      </header>

      <main className="grid">
        <section className="panel control">
          <div className="panel-title">Session</div>
          <div className="control-row">
            <div>
              <div className="label">Thread</div>
              <div className="value">{selectedThreadId ?? "none"}</div>
            </div>
            <div>
              <div className="label">Turn</div>
              <div className="value">{displayedTurnId ?? "idle"}</div>
            </div>
          </div>
          <div className="button-row">
            <button className="btn" onClick={connect} type="button">
              Reconnect
            </button>
            <button className="btn primary" onClick={handleStartThread} type="button" disabled={!initialized}>
              Start Thread
            </button>
          </div>
          {connectionError ? <div className="notice">{connectionError}</div> : null}

          <div className="composer">
            <label className="label" htmlFor="message">
              Message
            </label>
            <textarea
              id="message"
              value={input}
              placeholder="Ask Codex for a change or summary..."
              onChange={(event) => setInput(event.target.value)}
              rows={4}
            />
            <button
              className="btn primary"
              type="button"
              onClick={handleSendMessage}
              disabled={!initialized || !selectedThreadId || !input.trim()}
            >
              Send Turn
            </button>
          </div>

          <div className="thread-list">
            <div className="panel-title">Subscribed Threads</div>
            {threads.length === 0 ? (
              <div className="empty">No threads yet.</div>
            ) : (
              threads.map((id) => (
                <button
                  key={id}
                  type="button"
                  className={`thread-item ${selectedThreadId === id ? "active" : ""}`}
                  onClick={() => selectThread(id)}
                >
                  {id}
                </button>
              ))
            )}
          </div>

          {approvals.length ? (
            <div className="approvals">
              <div className="panel-title">Approvals</div>
              {approvals.map((approval) => (
                <div className="approval-card" key={String(approval.id)}>
                  <div className="approval-header">
                    <span>{approval.method}</span>
                    <span className="approval-time">{approval.receivedAt}</span>
                  </div>
                  <pre>{JSON.stringify(approval.params, null, 2)}</pre>
                  <div className="button-row">
                    <button
                      className="btn"
                      type="button"
                      onClick={() => handleApprovalDecision(approval.id, "decline")}
                    >
                      Decline
                    </button>
                    <button
                      className="btn primary"
                      type="button"
                      onClick={() => handleApprovalDecision(approval.id, "accept")}
                    >
                      Accept
                    </button>
                  </div>
                </div>
              ))}
            </div>
          ) : null}
        </section>

        <section className="panel chat">
          <div className="panel-title">Conversation</div>
          <div className="chat-scroll">
            {activeMessages.length === 0 ? (
              <div className="empty">No messages yet. Start a thread to begin.</div>
            ) : (
              activeMessages.map((message) => (
                <div className={`bubble ${message.role}`} key={message.id}>
                  <div className="bubble-role">{message.role === "user" ? "You" : "Codex"}</div>
                  <div className="bubble-text">{message.text}</div>
                </div>
              ))
            )}
          </div>
        </section>

        <section className="panel logs">
          <div className="panel-title">Event Log</div>
          <div className="log-scroll">
            {logs.length === 0 ? (
              <div className="empty">Events from app-server will appear here.</div>
            ) : (
              logs.map((entry) => (
                <div className={`log-entry ${entry.direction}`} key={entry.id}>
                  <div className="log-meta">
                    <span className="log-time">{entry.time}</span>
                    <span className="log-label">{entry.label}</span>
                  </div>
                  {entry.detail ? <div className="log-detail">{entry.detail}</div> : null}
                </div>
              ))
            )}
          </div>
        </section>
      </main>
    </div>
  );
}
