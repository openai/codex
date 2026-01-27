import { Allotment } from "allotment";
import "allotment/dist/style.css";
import { useEffect, useMemo, useRef, useState } from "react";
import { EditorPane } from "./EditorPane";
import { Explorer } from "./Explorer";
import { createChatSession, getWorkspace, patchWorkspaceSettings } from "./api";
import { AddRootModal } from "./AddRootModal";
import { useAppStore } from "./store";
import { VscodeChatPane, type VscodeChatPaneHandle } from "./VscodeChatPane";

function useMediaQuery(query: string) {
  const [matches, setMatches] = useState(false);
  useEffect(() => {
    const mql = window.matchMedia(query);
    const onChange = () => setMatches(mql.matches);
    onChange();
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, [query]);
  return matches;
}

export function App() {
  const roots = useAppStore((s) => s.roots);
  const rootsLoading = useAppStore((s) => s.rootsLoading);
  const rootsError = useAppStore((s) => s.rootsError);
  const setRoots = useAppStore((s) => s.setRoots);
  const workspaceSettings = useAppStore((s) => s.workspaceSettings);
  const setWorkspaceSettings = useAppStore((s) => s.setWorkspaceSettings);
  const setRootsLoading = useAppStore((s) => s.setRootsLoading);
  const setRootsError = useAppStore((s) => s.setRootsError);

  const [addOpen, setAddOpen] = useState(false);
  const isNarrow = useMediaQuery("(max-width: 820px)");
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [drawerTab, setDrawerTab] = useState<"explorer" | "viewer">("explorer");
  const [settingsOpen, setSettingsOpen] = useState(false);

  const [activeRootId, setActiveRootId] = useState<string | null>(() => {
    const v = localStorage.getItem("webActiveRootId");
    return v && v.length > 0 ? v : null;
  });
  const [rootPickerOpen, setRootPickerOpen] = useState(false);
  const [rootPickerMode, setRootPickerMode] = useState<"switch" | "newSession">("switch");

  const chatRef = useRef<VscodeChatPaneHandle | null>(null);

  const statusText = useMemo(() => {
    if (rootsLoading) return "ワークスペース読み込み中…";
    if (rootsError) return `エラー: ${rootsError}`;
    return `roots: ${roots.length}`;
  }, [roots.length, rootsError, rootsLoading]);

  const activeRoot =
    (activeRootId ? roots.find((r) => r.id === activeRootId) : null) ?? roots.at(0) ?? null;
  const cliLabel = workspaceSettings?.cliCommand ?? "codez";

  useEffect(() => {
    if (!activeRoot) return;
    localStorage.setItem("webActiveRootId", activeRoot.id);
    if (activeRootId !== activeRoot.id) setActiveRootId(activeRoot.id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeRoot?.id]);

  async function refreshWorkspace() {
    setRootsLoading(true);
    setRootsError(undefined);
    try {
      const ws = await getWorkspace();
      setRoots(ws.roots);
      setWorkspaceSettings(ws.settings ?? null);
    } catch (e) {
      setRootsError(e instanceof Error ? e.message : String(e));
    } finally {
      setRootsLoading(false);
    }
  }

  useEffect(() => {
    void refreshWorkspace();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const onMessage = (ev: MessageEvent) => {
      if (ev.origin !== window.location.origin) return;
      const data = ev.data as any;
      if (!data || typeof data !== "object") return;
      if (data.type === "codez.newSessionPickFolder") {
        setRootPickerMode("newSession");
        setRootPickerOpen(true);
        return;
      }
      if (data.type === "codez.openSettings") {
        setSettingsOpen(true);
        return;
      }
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, []);

  return (
    <div className="app">
      <div className="titlebar">
        <div className="titlebarLeft">
          {isNarrow ? (
            <button className="iconBtn" onClick={() => setDrawerOpen(true)} aria-label="Menu">
              ☰
            </button>
          ) : null}
          <button
            className="titleBtn"
            onClick={() => {
              setRootPickerMode("switch");
              setRootPickerOpen(true);
            }}
            title="root を選択"
          >
            <span className="title">
              {activeRoot ? `${activeRoot.label} (${cliLabel})` : "no root"}
            </span>
          </button>
        </div>
        <div className="titlebarActions">
          <button
            className="iconBtn"
            onClick={() => setSettingsOpen((v) => !v)}
            aria-label="Settings"
            title="Settings"
          >
            ⚙
          </button>
        </div>
      </div>

      <div className="workbench">
        {isNarrow ? (
          <VscodeChatPane ref={chatRef} rootId={activeRoot?.id ?? null} />
        ) : (
          <Allotment>
            <Allotment.Pane preferredSize={320} minSize={260}>
              <Explorer
                onOpenAdd={() => setAddOpen(true)}
                onWorkspaceChanged={() => void refreshWorkspace()}
              />
            </Allotment.Pane>
            <Allotment.Pane>
              <EditorPane />
            </Allotment.Pane>
            <Allotment.Pane preferredSize={520} minSize={360}>
              <VscodeChatPane ref={chatRef} rootId={activeRoot?.id ?? null} />
            </Allotment.Pane>
          </Allotment>
        )}
      </div>

      <div className="statusbar">{statusText}</div>

      {addOpen ? (
        <AddRootModal
          onClose={() => setAddOpen(false)}
          onAdded={async () => {
            setAddOpen(false);
            await refreshWorkspace();
          }}
        />
      ) : null}

      {isNarrow && drawerOpen ? (
        <div
          className="drawerBackdrop"
          onClick={(e) => {
            if (e.target === e.currentTarget) setDrawerOpen(false);
          }}
        >
          <div className="drawer">
            <div className="drawerHeader">
              <div className="drawerTabs">
                <button
                  className={drawerTab === "explorer" ? "drawerTab active" : "drawerTab"}
                  onClick={() => setDrawerTab("explorer")}
                >
                  Explorer
                </button>
                <button
                  className={drawerTab === "viewer" ? "drawerTab active" : "drawerTab"}
                  onClick={() => setDrawerTab("viewer")}
                >
                  Viewer
                </button>
              </div>
              <button className="iconBtn" onClick={() => setDrawerOpen(false)} aria-label="Close">
                ×
              </button>
            </div>
            <div className="drawerBody">
              {drawerTab === "explorer" ? (
                <Explorer
                  onOpenAdd={() => {
                    setDrawerOpen(false);
                    setAddOpen(true);
                  }}
                  onWorkspaceChanged={() => void refreshWorkspace()}
                  onFileOpened={() => {
                    setDrawerTab("viewer");
                  }}
                />
              ) : (
                <EditorPane />
              )}
            </div>
          </div>
        </div>
      ) : null}

      {settingsOpen ? (
        <div
          className="menuBackdrop"
          onClick={(e) => {
            if (e.target === e.currentTarget) setSettingsOpen(false);
          }}
        >
          <div className="menu">
            <div className="menuSectionTitle">CLI</div>
            <div className="menuSection">
              <button
                className={cliLabel === "codez" ? "segBtn active" : "segBtn"}
                onClick={async () => {
                  try {
                    await patchWorkspaceSettings({ cliCommand: "codez" });
                    setWorkspaceSettings({ cliCommand: "codez" });
                    setSettingsOpen(false);
                    await refreshWorkspace();
                  } catch (e) {
                    setRootsError(e instanceof Error ? e.message : String(e));
                  }
                }}
              >
                codez
              </button>
              <button
                className={cliLabel === "codex" ? "segBtn active" : "segBtn"}
                onClick={async () => {
                  try {
                    await patchWorkspaceSettings({ cliCommand: "codex" });
                    setWorkspaceSettings({ cliCommand: "codex" });
                    setSettingsOpen(false);
                    await refreshWorkspace();
                  } catch (e) {
                    setRootsError(e instanceof Error ? e.message : String(e));
                  }
                }}
              >
                codex
              </button>
              <div className="menuHint">
                切替すると app-server を再起動します（実行中の turn は中断される可能性があります）。
              </div>
            </div>
            <div className="menuSep" />
            <button
              className="menuItem"
              onClick={() => {
                setSettingsOpen(false);
                setRootPickerMode("switch");
                setRootPickerOpen(true);
              }}
            >
              Switch root
            </button>
            <button
              className="menuItem"
              onClick={() => {
                setSettingsOpen(false);
                setAddOpen(true);
              }}
            >
              Add folder
            </button>
            <button
              className="menuItem"
              onClick={() => {
                setSettingsOpen(false);
                void refreshWorkspace();
              }}
            >
              Refresh workspace
            </button>
          </div>
        </div>
      ) : null}

      {rootPickerOpen ? (
        <div
          className="modalBackdrop"
          onClick={(e) => {
            if (e.target === e.currentTarget) setRootPickerOpen(false);
          }}
        >
          <div className="modal">
            <div className="modalHeader">
              <div className="modalTitle">root を選択</div>
              <button className="btn" onClick={() => setRootPickerOpen(false)}>
                Close
              </button>
            </div>
            <div className="modalBody">
              {roots.length === 0 ? (
                <div className="empty">root がありません。先に Add folder してください。</div>
              ) : null}
              {rootPickerMode === "newSession" ? (
                <div className="empty">New session を作成する root を選択してください。</div>
              ) : null}
              {roots.map((r) => (
                <div
                  key={r.id}
                  className="dirRow"
                  onClick={async () => {
                    const prevRootId = activeRoot?.id ?? null;
                    if (rootPickerMode === "newSession") {
                      try {
                        await createChatSession(r.id);
                        if (prevRootId === r.id) {
                          chatRef.current?.refreshState();
                        }
                      } catch (e) {
                        setRootsError(e instanceof Error ? e.message : String(e));
                      }
                    }
                    setActiveRootId(r.id);
                    localStorage.setItem("webActiveRootId", r.id);
                    setRootPickerOpen(false);
                  }}
                  title={r.absPath}
                >
                  <span style={{ color: r.id === activeRoot?.id ? "#0e639c" : "#9da5b4" }}>
                    {r.id === activeRoot?.id ? "●" : "○"}
                  </span>
                  <span>{r.label}</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
