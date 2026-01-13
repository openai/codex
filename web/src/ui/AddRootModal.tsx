import { useEffect, useMemo, useState } from "react";
import { addRoot, browserList, getBrowserHome } from "./api";

export function AddRootModal(props: { onClose: () => void; onAdded: () => void }) {
  const [home, setHome] = useState<string>();
  const [path, setPath] = useState<string>();
  const [label, setLabel] = useState<string>("");
  const [labelTouched, setLabelTouched] = useState(false);

  const [dirs, setDirs] = useState<Array<{ name: string; absPath: string }>>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();

  const canUp = useMemo(() => {
    if (!home || !path) return false;
    const normHome = home.endsWith("/") ? home : `${home}/`;
    const normPath = path.endsWith("/") ? path : `${path}/`;
    return normPath !== normHome;
  }, [home, path]);

  async function load(p: string) {
    setLoading(true);
    setError(undefined);
    try {
      const res = await browserList(p);
      setDirs(res.entries);
      setPath(res.path);
      if (!labelTouched) {
        setLabel(res.path.split("/").filter(Boolean).at(-1) || "");
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void (async () => {
      try {
        const h = await getBrowserHome();
        setHome(h.home);
        await load(h.home);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div
      className="modalBackdrop"
      onClick={(e) => {
        if (e.target === e.currentTarget) props.onClose();
      }}
    >
      <div className="modal">
        <div className="modalHeader">
          <div className="modalTitle">フォルダを追加（~/ 配下のみ）</div>
          <button className="btn" onClick={props.onClose}>
            Close
          </button>
        </div>

        <div className="modalPath">
          <div className="modalPathText">{path ?? "(loading…)"}</div>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <input
              className="input"
              value={label}
              onChange={(e) => {
                setLabelTouched(true);
                setLabel(e.target.value);
              }}
              placeholder="Label (任意)"
            />
            <button
              className="btn"
              disabled={!canUp}
              onClick={async () => {
                if (!home || !path) return;
                const parent = path.split("/").slice(0, -1).join("/") || "/";
                await load(parent);
              }}
            >
              Up
            </button>
          </div>
        </div>

        <div className="modalBody">
          {loading ? <div className="empty">読み込み中…</div> : null}
          {error ? <div className="empty">{error}</div> : null}
          {!loading && !error && dirs.length === 0 ? (
            <div className="empty">子ディレクトリがありません。</div>
          ) : null}
          {dirs.map((d) => (
            <div
              key={d.absPath}
              className="dirRow"
              onClick={async () => {
                await load(d.absPath);
              }}
              title={d.absPath}
            >
              <span style={{ color: "#9da5b4" }}>▸</span>
              <span>{d.name}</span>
            </div>
          ))}
        </div>

        <div className="modalFooter">
          <div className="modalPathText">
            選択中: <span style={{ color: "#d4d4d4" }}>{path ?? "-"}</span>
          </div>
          <button
            className="btn"
            disabled={!path || loading}
            onClick={async () => {
              try {
                if (!path) return;
                await addRoot({ absPath: path, label: label.trim() || undefined });
                props.onAdded();
              } catch (e) {
                setError(e instanceof Error ? e.message : String(e));
              }
            }}
          >
            Add This Folder
          </button>
        </div>
      </div>
    </div>
  );
}
