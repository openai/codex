import { ConnectionStatus } from "../types";

interface TopBarProps {
  connection: ConnectionStatus;
  statusText: string;
  streamActive: boolean;
  threadId: string | null;
  logOpen: boolean;
  sidebarOpen: boolean;
  onToggleSidebar: () => void;
  onToggleLog: () => void;
  onReconnect: () => void;
  theme: "dark" | "light";
  onToggleTheme: () => void;
}

export function TopBar({
  connection,
  statusText,
  streamActive,
  threadId,
  logOpen,
  sidebarOpen,
  onToggleSidebar,
  onToggleLog,
  onReconnect,
  theme,
  onToggleTheme,
}: TopBarProps) {
  const shortThreadId = threadId ? threadId.slice(0, 8) : null;

  return (
    <header className="topbar">
      <div className="topbar__left">
        <button
          aria-label="Toggle sessions sidebar"
          className={`ghost-btn mobile-only ${sidebarOpen ? "is-active" : ""}`}
          onClick={onToggleSidebar}
          type="button"
        >
          Sessions
        </button>
        <div>
          <h1 className="topbar__product">Codex Companion</h1>
          <p className="topbar__subtitle">
            {shortThreadId ? `Active session ${shortThreadId}` : "Start a session to begin"}
          </p>
        </div>
      </div>

      <div className="topbar__right">
        <div className={`status status--${connection}`}>
          <span className="status__dot" aria-hidden="true" />
          <span>{statusText}</span>
        </div>
        <div className={`status-chip ${streamActive ? "status-chip--live" : ""}`}>
          {streamActive ? "Responding" : "Idle"}
        </div>
        <button className="ghost-btn" onClick={onReconnect} type="button">
          Reconnect
        </button>
        <button className={`ghost-btn ${logOpen ? "is-active" : ""}`} onClick={onToggleLog} type="button">
          JSON-RPC Log
        </button>
        <button className="ghost-btn" onClick={onToggleTheme} type="button">
          {theme === "dark" ? "Light" : "Dark"}
        </button>
      </div>
    </header>
  );
}
