import { forwardRef, useEffect, useImperativeHandle, useMemo, useRef, useState } from "react";
import {
  createChatSession,
  getChatMessages,
  getChatState,
  putChatMessages,
  setChatActiveSession,
  updateChatSession,
  type ChatMessage,
} from "./api";
import { useAppStore } from "./store";

type Session = {
  id: string;
  title: string;
  threadId: string | null;
};

function uid() {
  return `${Date.now()}_${Math.random().toString(16).slice(2)}`;
}

type PendingRequest = {
  requestId: number;
  method: string;
  params: unknown;
};

export type CodexPaneHandle = {
  newSession: () => void;
  resumeSession: () => void;
  reloadSession: () => void;
};

export const CodexPane = forwardRef<
  CodexPaneHandle,
  { rootId: string | null; onRequestNewSession?: () => void }
>(function CodexPane(props, ref) {
  const roots = useAppStore((s) => s.roots);
  const workspaceSettings = useAppStore((s) => s.workspaceSettings);
  const activeRoot =
    (props.rootId ? roots.find((r) => r.id === props.rootId) : null) ?? roots.at(0) ?? null;

  const [sessions, setSessions] = useState<Session[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);

  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string>();
  const [pending, setPending] = useState<PendingRequest[]>([]);

  const logRef = useRef<HTMLDivElement | null>(null);
  const saveTimerRef = useRef<number | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const assistantBufRef = useRef<Record<string, string>>({});

  const statusText = useMemo(() => {
    if (running) return "実行中…";
    if (error) return `エラー: ${error}`;
    const cli = workspaceSettings?.cliCommand ?? "codex-mine";
    const rootLabel = activeRoot ? `${activeRoot.label} (${cli})` : "no root";
    const activeSession = activeSessionId ? sessions.find((s) => s.id === activeSessionId) : null;
    const th = activeSession?.threadId ? `thread: ${activeSession.threadId}` : "新規";
    return `${rootLabel} / ${activeSession?.title ?? "no session"} / ${th}`;
  }, [activeRoot, activeSessionId, error, running, sessions, workspaceSettings?.cliCommand]);

  useEffect(() => {
    const el = logRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  }, [messages.length]);

  useEffect(() => {
    if (!activeRoot) return;
    setError(undefined);
    void (async () => {
      try {
        const st = await getChatState(activeRoot.id);
        const list = st.sessions ?? [];
        setSessions(list);
        setActiveSessionId(st.activeSessionId ?? list.at(0)?.id ?? null);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
        setSessions([]);
        setActiveSessionId(null);
        setMessages([]);
      }
    })();
  }, [activeRoot?.id]);

  useEffect(() => {
    if (!activeRoot) return;
    // NOTE: モバイル前提で単一接続を想定。複数タブ同時利用は後で調整。
    const url = `${location.origin.replace(/^http/, "ws")}/api/ws`;
    const ws = new WebSocket(url);
    wsRef.current = ws;

    ws.addEventListener("open", () => {
      ws.send(JSON.stringify({ type: "subscribe", rootId: activeRoot.id }));
    });

    ws.addEventListener("message", (ev) => {
      let msg: any;
      try {
        msg = JSON.parse(String(ev.data));
      } catch {
        setError("ws: invalid json");
        return;
      }
      if (!msg || typeof msg !== "object") return;

      if (msg.type === "error") {
        setError(String(msg.message ?? "ws error"));
        return;
      }

      if (msg.type === "backend.request") {
        const req = msg.request;
        const requestId = Number(req?.id);
        const method = String(req?.method ?? "");
        const params = req?.params ?? null;
        if (!Number.isFinite(requestId) || !method) return;
        setPending((cur) => [...cur, { requestId, method, params }]);
        return;
      }

      if (msg.type === "backend.notification") {
        const n = msg.notification;
        const method = String(n?.method ?? "");
        const p = n?.params ?? null;

        if (method === "turn/started") {
          setRunning(true);
          return;
        }

        if (method === "turn/completed") {
          setRunning(false);
          return;
        }

        if (method === "item/agentMessage/delta") {
          const threadId = String(p?.threadId ?? "");
          const delta = String(p?.delta ?? "");
          if (!threadId || !delta) return;
          assistantBufRef.current[threadId] = (assistantBufRef.current[threadId] ?? "") + delta;
          setMessages((cur) => {
            const buf = assistantBufRef.current[threadId] ?? "";
            const last = cur.at(-1);
            if (last?.role === "assistant") {
              return [...cur.slice(0, -1), { ...last, text: buf }];
            }
            return [...cur, { id: uid(), role: "assistant", text: buf }];
          });
          return;
        }

        if (method === "item/started" || method === "item/completed") {
          const itemType = String(p?.item?.type ?? "");
          if (!itemType) return;
          // VSCode拡張は block として整形するが、まずは最低限可視化する。
          setMessages((cur) => [...cur, { id: uid(), role: "meta", text: `${method}: ${itemType}` }]);
        }
      }

      if (msg.type === "chat.sent") {
        const sessionId = String(msg.sessionId ?? "");
        const threadId = typeof msg.threadId === "string" ? msg.threadId : null;
        if (sessionId) setSessionThreadId(sessionId, threadId);
      }
    });

    ws.addEventListener("close", () => {
      if (wsRef.current === ws) wsRef.current = null;
    });

    return () => {
      try {
        ws.close();
      } catch {
        // ignore
      }
    };
  }, [activeRoot?.id]);

  useEffect(() => {
    if (!activeRoot) {
      setSessions([]);
      setActiveSessionId(null);
      setMessages([]);
      return;
    }
    if (!activeSessionId) {
      setMessages([]);
      return;
    }
    setError(undefined);
    void (async () => {
      try {
        const res = await getChatMessages(activeRoot.id, activeSessionId);
        setMessages(res.messages ?? []);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
        setMessages([]);
      }
    })();
  }, [activeRoot, activeSessionId]);

  useEffect(() => {
    if (!activeRoot) return;
    if (saveTimerRef.current) window.clearTimeout(saveTimerRef.current);
    saveTimerRef.current = window.setTimeout(() => {
      if (!activeSessionId) return;
      void putChatMessages({ rootId: activeRoot.id, sessionId: activeSessionId, messages }).catch(
        (e) => setError(e instanceof Error ? e.message : String(e)),
      );
    }, 450);
    return () => {
      if (saveTimerRef.current) window.clearTimeout(saveTimerRef.current);
    };
  }, [activeRoot, activeSessionId, messages]);

  function ensureSessionExists(): Session | null {
    if (!activeRoot) return null;
    if (sessions.length > 0) {
      const found = sessions.find((s) => s.id === activeSessionId) ?? sessions[0];
      if (activeSessionId !== found.id) setActiveSessionId(found.id);
      return found;
    }
    return null;
  }

  function setSessionThreadId(sessionId: string, threadId: string | null) {
    setSessions((cur) => cur.map((s) => (s.id === sessionId ? { ...s, threadId } : s)));
    if (!activeRoot) return;
    void updateChatSession({ rootId: activeRoot.id, sessionId, threadId }).catch((e) =>
      setError(e instanceof Error ? e.message : String(e)),
    );
  }

  function newSession() {
    if (!activeRoot) {
      setError("root がありません（先にワークスペースへフォルダを追加してください）");
      return;
    }
    setError(undefined);
    void (async () => {
      try {
        const res = await createChatSession(activeRoot.id);
        const next = res.session as Session;
        setSessions((cur) => [...cur, next]);
        setActiveSessionId(res.activeSessionId ?? next.id);
        setMessages([]);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    })();
  }

  function resumeSession() {
    if (!activeRoot) {
      setError("root がありません（先にワークスペースへフォルダを追加してください）");
      return;
    }
    const s = ensureSessionExists();
    if (!s) {
      setError("session がありません（New で作成してください）");
      return;
    }
    const tid = prompt("threadId を入力（履歴表示は app-server 移行後に対応）", s.threadId ?? "");
    if (tid == null) return;
    const v = tid.trim();
    if (!v) {
      setSessionThreadId(s.id, null);
      setMessages((cur) => [...cur, { id: uid(), role: "meta", text: "threadId をクリアしました" }]);
      return;
    }
    setSessionThreadId(s.id, v);
    setMessages((cur) => [
      ...cur,
      {
        id: uid(),
        role: "meta",
        text: `threadId を設定しました: ${v}\n※履歴のロード/復元は app-server 化後に実装します`,
      },
    ]);
  }

  function reloadSession() {
    setMessages((cur) => [
      ...cur,
      { id: uid(), role: "meta", text: "Reload は app-server 化後に対応します（現状は未対応）。" },
    ]);
  }

  useImperativeHandle(
    ref,
    () => ({
      newSession,
      resumeSession,
      reloadSession,
    }),
    [sessions, activeRoot],
  );

  async function send() {
    const text = input.trim();
    if (!text || running) return;
    if (!activeRoot) {
      setError("root がありません（先にワークスペースへフォルダを追加してください）");
      return;
    }
    let s = ensureSessionExists();
    if (!s) {
      // セッション一覧ロード前でも送信できるように、ここで作成して続行する。
      try {
        const created = await createChatSession(activeRoot.id);
        const next = created.session as Session;
        setSessions((cur) => [...cur, next]);
        setActiveSessionId(created.activeSessionId ?? next.id);
        s = next;
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
        return;
      }
    }
    setInput("");
    setError(undefined);
    setMessages((s) => [...s, { id: uid(), role: "user", text }]);
    const ws = wsRef.current;
    if (!ws || ws.readyState !== ws.OPEN) {
      setError("ws が未接続です");
      return;
    }
    try {
      ws.send(JSON.stringify({ type: "chat.send", sessionId: s.id, text }));
      setRunning(true);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setRunning(false);
    }
  }

  return (
    <div className="codexPane">
      <div className="codexTop">
        <div className="codexTitle">Codex</div>
        <div className="codexStatus" title={statusText}>
          {statusText}
        </div>
        <div className="codexActions">
          <button
            className="btn"
            onClick={() => {
              if (running) return;
              if (props.onRequestNewSession) props.onRequestNewSession();
              else newSession();
            }}
          >
            New
          </button>
        </div>
      </div>

      <div className="codexTabs">
        <div className="codexTabsScroll">
          {sessions.map((s) => (
            <button
              key={s.id}
              className={s.id === activeSessionId ? "codexTab active" : "codexTab"}
              onClick={() => {
                if (running) return;
                setActiveSessionId(s.id);
                setError(undefined);
                if (activeRoot) void setChatActiveSession(activeRoot.id, s.id);
              }}
              title={s.threadId ? `thread: ${s.threadId}` : "new"}
            >
              {s.title}
            </button>
          ))}
        </div>
      </div>

      <div className="codexLog" ref={logRef}>
        {error ? <div className="codexError">{error}</div> : null}
        {pending.length > 0 ? (
          <div className="codexApproval">
            <div style={{ fontWeight: 600, marginBottom: 6 }}>承認が必要です</div>
            {pending.map((p) => (
              <div key={p.requestId} style={{ marginBottom: 10 }}>
                <div style={{ fontFamily: "var(--mono)", fontSize: 12, color: "#9da5b4" }}>
                  {p.method} (id={p.requestId})
                </div>
                <details style={{ marginTop: 6 }}>
                  <summary style={{ cursor: "pointer" }}>details</summary>
                  <pre style={{ whiteSpace: "pre-wrap", wordBreak: "break-word", margin: 0 }}>
                    {JSON.stringify(p.params, null, 2)}
                  </pre>
                </details>
                <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginTop: 8 }}>
                  {(["accept", "acceptForSession", "decline", "cancel"] as const).map((d) => (
                    <button
                      key={d}
                      className="btn"
                      onClick={() => {
                        const ws = wsRef.current;
                        if (!ws || ws.readyState !== ws.OPEN) {
                          setError("ws が未接続です");
                          return;
                        }
                        ws.send(
                          JSON.stringify({
                            type: "approval.respond",
                            requestId: p.requestId,
                            decision: d,
                          }),
                        );
                        setPending((cur) => cur.filter((x) => x.requestId !== p.requestId));
                      }}
                    >
                      {d}
                    </button>
                  ))}
                </div>
              </div>
            ))}
          </div>
        ) : null}
        {messages.length === 0 ? (
          <div className="empty">右のパネルから Codex と会話できます（Read Only）。</div>
        ) : null}
        {messages.map((m) => (
          <div
            key={m.id}
            className={
              m.role === "user"
                ? "codexMsg codexUser"
                : m.role === "assistant"
                  ? "codexMsg codexAssistant"
                  : "codexMsg codexMeta"
            }
          >
            <div className="codexMsgRole">{m.role}</div>
            <div className="codexMsgText">{m.text}</div>
          </div>
        ))}
      </div>

      <div className="codexComposer">
        <textarea
          className="codexInput"
          rows={2}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="Codex に指示…"
          disabled={running}
        />
        <button className="btn" onClick={() => void send()} disabled={running || !input.trim()}>
          Send
        </button>
      </div>
    </div>
  );
});
