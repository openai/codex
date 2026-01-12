import Editor from "@monaco-editor/react";
import { useEffect } from "react";
import { getFileText } from "./api";
import { useAppStore } from "./store";

function languageForPath(path: string) {
  const ext = path.split(".").at(-1)?.toLowerCase() ?? "";
  switch (ext) {
    case "ts":
    case "tsx":
      return "typescript";
    case "js":
    case "jsx":
      return "javascript";
    case "json":
      return "json";
    case "md":
      return "markdown";
    case "rs":
      return "rust";
    case "py":
      return "python";
    case "yml":
    case "yaml":
      return "yaml";
    case "toml":
      return "toml";
    default:
      return undefined;
  }
}

export function EditorPane() {
  const openTabs = useAppStore((s) => s.openTabs);
  const activeTabId = useAppStore((s) => s.activeTabId);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const closeTab = useAppStore((s) => s.closeTab);

  const files = useAppStore((s) => s.files);
  const setFileState = useAppStore((s) => s.setFileState);

  const active = openTabs.find((t) => t.id === activeTabId) ?? openTabs.at(-1);

  useEffect(() => {
    if (!active) return;
    const key = `${active.rootId}:${active.path}`;
    const st = files[key];
    if (st?.loading || st?.text != null || st?.error) return;

    setFileState(key, { loading: true, error: undefined });
    void (async () => {
      try {
        const res = await getFileText(active.rootId, active.path);
        setFileState(key, { loading: false, text: res.text });
      } catch (e) {
        setFileState(key, {
          loading: false,
          error: e instanceof Error ? e.message : String(e),
        });
      }
    })();
  }, [active, files, setFileState]);

  const activeKey = active ? `${active.rootId}:${active.path}` : undefined;
  const activeState = activeKey ? files[activeKey] : undefined;

  return (
    <div style={{ height: "100%", minHeight: 0 }}>
      <div className="tabs">
        {openTabs.map((t) => (
          <div
            key={t.id}
            className={t.id === activeTabId ? "tab tabActive" : "tab"}
            onClick={() => setActiveTab(t.id)}
          >
            <span>{t.title}</span>
            <button
              className="tabClose"
              onClick={(e) => {
                e.stopPropagation();
                closeTab(t.id);
              }}
              aria-label="Close"
              title="Close"
            >
              ×
            </button>
          </div>
        ))}
      </div>

      <div className="editorWrap">
        {!active ? (
          <div className="empty">ファイルを選択してください（Read Only）。</div>
        ) : activeState?.loading ? (
          <div className="empty">読み込み中…</div>
        ) : activeState?.error ? (
          <div className="empty">{activeState.error}</div>
        ) : (
          <Editor
            path={`${active.rootId}${active.path}`}
            language={languageForPath(active.path)}
            value={activeState?.text ?? ""}
            theme="vs-dark"
            options={{
              readOnly: true,
              minimap: { enabled: false },
              wordWrap: "off",
              scrollBeyondLastLine: false,
              fontFamily:
                "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New', monospace",
              fontSize: 13,
            }}
          />
        )}
      </div>
    </div>
  );
}
