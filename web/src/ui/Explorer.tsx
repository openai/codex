import clsx from "clsx";
import { useEffect, useMemo, useState } from "react";
import { getTree, removeRoot, renameRoot, setRootCliCommand, type TreeEntry } from "./api";
import { useAppStore } from "./store";

function joinPosix(a: string, b: string) {
  const base = a.endsWith("/") ? a.slice(0, -1) : a;
  const add = b.startsWith("/") ? b : `/${b}`;
  return `${base}${add}`;
}

function iconFor(kind: TreeEntry["kind"], expanded: boolean) {
  if (kind === "dir") return expanded ? "▾" : "▸";
  if (kind === "file") return "•";
  if (kind === "symlink") return "↪";
  return "·";
}

function isDir(kind: TreeEntry["kind"]) {
  return kind === "dir";
}

export function Explorer(props: {
  onOpenAdd: () => void;
  onWorkspaceChanged: () => void | Promise<void>;
  onFileOpened?: () => void;
}) {
  const roots = useAppStore((s) => s.roots);
  const tree = useAppStore((s) => s.tree);
  const setTreeState = useAppStore((s) => s.setTreeState);
  const openTab = useAppStore((s) => s.openTab);

  const [expanded, setExpanded] = useState<Record<string, boolean>>({});

  async function ensureDirLoaded(rootId: string, dirPath: string) {
    const key = `${rootId}:${dirPath}`;
    const st = tree[key];
    if (st?.loading || st?.entries) return;
    setTreeState(key, { loading: true, error: undefined });
    try {
      const entries = await getTree(rootId, dirPath);
      setTreeState(key, { entries, loading: false });
    } catch (e) {
      setTreeState(key, {
        loading: false,
        error: e instanceof Error ? e.message : String(e),
      });
    }
  }

  useEffect(() => {
    for (const r of roots) {
      void ensureDirLoaded(r.id, "/");
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [roots.map((r) => r.id).join(",")]);

  const sortedRoots = useMemo(() => {
    return [...roots].sort((a, b) => a.order - b.order);
  }, [roots]);

  return (
    <div className="sidebar">
      <div className="sidebarHeader">
        <div className="sidebarHeaderTitle">EXPLORER</div>
        <button className="btn" onClick={props.onOpenAdd}>
          Add
        </button>
      </div>
      <div className="list">
        {sortedRoots.length === 0 ? (
          <div className="empty">
            roots がありません。Add から `~/` 配下のフォルダを追加してください。
          </div>
        ) : null}

        {sortedRoots.map((root) => (
          <div key={root.id} className="rootRow">
              <div className="rootTop">
                <div className="rootTitle">{root.label}</div>
                <div style={{ display: "flex", gap: 6 }}>
                  <select
                    className="rootSelect"
                    value={root.cliCommand}
                    onChange={async (e) => {
                      try {
                        const v = e.target.value === "codex" ? "codex" : "codez";
                        await setRootCliCommand(root.id, v);
                        await props.onWorkspaceChanged();
                      } catch (err) {
                        alert(err instanceof Error ? err.message : String(err));
                      }
                    }}
                    aria-label="CLI"
                    title="CLI"
                  >
                    <option value="codez">codez</option>
                    <option value="codex">codex</option>
                  </select>
                  <button
                    className="btn"
                    onClick={async () => {
                      try {
                        const next = prompt("label を入力", root.label);
                      if (next == null) return;
                      await renameRoot(root.id, next);
                      await props.onWorkspaceChanged();
                    } catch (e) {
                      alert(e instanceof Error ? e.message : String(e));
                    }
                  }}
                >
                  Rename
                </button>
                <button
                  className={clsx("btn", "btnDanger")}
                  onClick={async () => {
                    try {
                      const ok = confirm(`root を削除します: ${root.label}`);
                      if (!ok) return;
                      await removeRoot(root.id);
                      await props.onWorkspaceChanged();
                    } catch (e) {
                      alert(e instanceof Error ? e.message : String(e));
                    }
                  }}
                >
                  Remove
                </button>
              </div>
            </div>
            <div className="rootPath">{root.absPath}</div>

            <div className="tree">
              <TreeNode
                rootId={root.id}
                path="/"
                name={root.label}
                depth={0}
                kind="dir"
                expanded={!!expanded[`${root.id}:/`]}
                toggleExpanded={async () => {
                  const k = `${root.id}:/`;
                  const next = !expanded[k];
                  setExpanded((s) => ({ ...s, [k]: next }));
                  if (next) await ensureDirLoaded(root.id, "/");
                }}
                getChildren={() => tree[`${root.id}:/`]?.entries ?? []}
                loading={tree[`${root.id}:/`]?.loading}
                error={tree[`${root.id}:/`]?.error}
                onOpenFile={(filePath) => {
                  const title = filePath.split("/").filter(Boolean).at(-1) ?? filePath;
                  openTab({
                    id: `${root.id}:${filePath}`,
                    rootId: root.id,
                    path: filePath,
                    title,
                  });
                  props.onFileOpened?.();
                }}
                onToggleDir={async (dirPath) => {
                  const k = `${root.id}:${dirPath}`;
                  const next = !expanded[k];
                  setExpanded((s) => ({ ...s, [k]: next }));
                  if (next) await ensureDirLoaded(root.id, dirPath);
                }}
                isExpanded={(dirPath) => !!expanded[`${root.id}:${dirPath}`]}
              />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function TreeNode(props: {
  rootId: string;
  path: string;
  name: string;
  depth: number;
  kind: TreeEntry["kind"];
  expanded: boolean;
  toggleExpanded: () => void | Promise<void>;
  getChildren: () => TreeEntry[];
  loading?: boolean;
  error?: string;
  onOpenFile: (path: string) => void;
  onToggleDir: (path: string) => void | Promise<void>;
  isExpanded: (path: string) => boolean;
}) {
  const pad = 10 + props.depth * 14;
  const children = props.getChildren();
  return (
    <div>
      <div
        className="node"
        style={{ paddingLeft: pad }}
        onClick={async () => {
          if (isDir(props.kind)) {
            await props.toggleExpanded();
          }
        }}
      >
        <span className="nodeIcon">{iconFor(props.kind, props.expanded)}</span>
        <span>{props.name}</span>
      </div>

      {props.expanded ? (
        <div>
          {props.loading ? (
            <div className="empty" style={{ paddingLeft: pad + 14 }}>
              読み込み中…
            </div>
          ) : null}
          {props.error ? (
            <div className="empty" style={{ paddingLeft: pad + 14 }}>
              {props.error}
            </div>
          ) : null}
          {children.map((c) => (
            <div key={c.path}>
              <div
                className="node"
                style={{ paddingLeft: pad + 14 }}
                onClick={async () => {
                  if (c.kind === "dir") {
                    await props.onToggleDir(c.path);
                    return;
                  }
                  if (c.kind === "file" || c.kind === "symlink") {
                    props.onOpenFile(c.path);
                  }
                }}
              >
                <span className="nodeIcon">{iconFor(c.kind, props.isExpanded(c.path))}</span>
                <span>{c.name}</span>
              </div>

              {c.kind === "dir" && props.isExpanded(c.path) ? (
                <TreeNode
                  rootId={props.rootId}
                  path={c.path}
                  name={c.name}
                  depth={props.depth + 1}
                  kind="dir"
                  expanded={true}
                  toggleExpanded={() => props.onToggleDir(c.path)}
                  getChildren={() => {
                    const st = useAppStore.getState().tree[`${props.rootId}:${c.path}`];
                    return st?.entries ?? [];
                  }}
                  loading={useAppStore.getState().tree[`${props.rootId}:${c.path}`]?.loading}
                  error={useAppStore.getState().tree[`${props.rootId}:${c.path}`]?.error}
                  onOpenFile={props.onOpenFile}
                  onToggleDir={props.onToggleDir}
                  isExpanded={props.isExpanded}
                />
              ) : null}
            </div>
          ))}
        </div>
      ) : null}
    </div>
  );
}
