import { RawLogEntry } from "../types";

interface DevLogPanelProps {
  logs: RawLogEntry[];
  open: boolean;
  onClose: () => void;
}

export function DevLogPanel({ logs, open, onClose }: DevLogPanelProps) {
  if (!open) {
    return null;
  }

  return (
    <>
      <button aria-label="Close JSON-RPC log drawer" className="devlog-backdrop" onClick={onClose} type="button" />
      <section className={`devlog ${open ? "devlog--open" : ""}`}>
        <div className="devlog__header">
          <p>JSON-RPC log</p>
          <button className="ghost-btn" onClick={onClose} type="button">
            Close
          </button>
        </div>

        <div className="devlog__body">
          {logs.length === 0 ? <p className="empty-note">No transport activity yet.</p> : null}

          {logs.map((log) => (
            <pre className={`log-line log-line--${log.direction}`} key={log.id}>
              [{new Date(log.at).toLocaleTimeString()}] {log.direction.toUpperCase()} {log.text}
            </pre>
          ))}
        </div>
      </section>
    </>
  );
}
