import { Allotment } from "allotment";
import "allotment/dist/style.css";
import { useEffect, useMemo, useState } from "react";
import { EditorPane } from "./EditorPane";
import { Explorer } from "./Explorer";
import { getWorkspace } from "./api";
import { AddRootModal } from "./AddRootModal";
import { useAppStore } from "./store";

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
  const setRootsLoading = useAppStore((s) => s.setRootsLoading);
  const setRootsError = useAppStore((s) => s.setRootsError);

  const [addOpen, setAddOpen] = useState(false);
  const isNarrow = useMediaQuery("(max-width: 820px)");
  const [mobileExplorerOpen, setMobileExplorerOpen] = useState(false);

  const statusText = useMemo(() => {
    if (rootsLoading) return "ワークスペース読み込み中…";
    if (rootsError) return `エラー: ${rootsError}`;
    return `roots: ${roots.length}`;
  }, [roots.length, rootsError, rootsLoading]);

  async function refreshWorkspace() {
    setRootsLoading(true);
    setRootsError(undefined);
    try {
      const ws = await getWorkspace();
      setRoots(ws.roots);
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

  return (
    <div className="app">
      <div className="titlebar">
        <span className="title">Workspace Viewer (Read Only)</span>
        {isNarrow ? (
          <button className="btn" onClick={() => setMobileExplorerOpen(true)}>
            Explorer
          </button>
        ) : null}
        <button className="btn" onClick={() => setAddOpen(true)}>
          Add Folder
        </button>
        <button className="btn" onClick={() => void refreshWorkspace()}>
          Refresh
        </button>
      </div>

      <div className="workbench">
        {isNarrow ? (
          <div style={{ height: "100%", minHeight: 0 }}>
            <EditorPane />
            {mobileExplorerOpen ? (
              <div
                className="mobileDrawerBackdrop"
                onClick={(e) => {
                  if (e.target === e.currentTarget) setMobileExplorerOpen(false);
                }}
              >
                <div className="mobileDrawer">
                  <Explorer
                    onOpenAdd={() => setAddOpen(true)}
                    onWorkspaceChanged={() => void refreshWorkspace()}
                    onFileOpened={() => setMobileExplorerOpen(false)}
                  />
                </div>
              </div>
            ) : null}
          </div>
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
    </div>
  );
}
