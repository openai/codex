import * as vscode from "vscode";

import type { Session } from "../sessions";
import type { BackendId } from "../sessions";
import type { SessionStore } from "../sessions";

export class SessionTreeDataProvider
  implements vscode.TreeDataProvider<TreeNode>, vscode.Disposable
{
  private readonly emitter = new vscode.EventEmitter<TreeNode | null>();
  public readonly onDidChangeTreeData = this.emitter.event;

  public onDidSelectSession: ((sessionId: string) => void) | null = null;

  public constructor(
    private readonly extensionUri: vscode.Uri,
    private readonly sessions: SessionStore,
    private readonly getWorkspaceColorIndex: (
      workspaceFolderUri: string,
    ) => number,
  ) {}

  public dispose(): void {
    this.emitter.dispose();
  }

  public refresh(): void {
    this.emitter.fire(null);
  }

  public getTreeItem(element: TreeNode): vscode.TreeItem {
    if (element.kind === "folder") {
      const item = new vscode.TreeItem(
        element.label,
        vscode.TreeItemCollapsibleState.Expanded,
      );
      if (element.workspaceFolderUri) {
        const idx = this.getWorkspaceColorIndex(element.workspaceFolderUri);
        item.iconPath = iconForColorIndex(this.extensionUri, idx);
      }
      item.contextValue = "codez.folder";
      return item;
    }

    if (element.kind === "backend") {
      const item = new vscode.TreeItem(
        element.label,
        vscode.TreeItemCollapsibleState.Expanded,
      );
      if (element.workspaceFolderUri) {
        const idx = this.getWorkspaceColorIndex(element.workspaceFolderUri);
        item.iconPath = iconForColorIndex(this.extensionUri, idx);
      }
      item.contextValue = "codez.backend";
      return item;
    }

    const title = normalizeTitle(element.session.title);
    const label = element.session.customTitle
      ? title
      : `${title} #${element.index}`;
    const item = new vscode.TreeItem(
      label,
      vscode.TreeItemCollapsibleState.None,
    );
    const idx = this.getWorkspaceColorIndex(element.session.workspaceFolderUri);
    item.iconPath = iconForColorIndex(this.extensionUri, idx);
    // Show full thread id in description for copyability; omit short id in label.
    item.description = element.session.threadId;
    item.contextValue = "codez.session";
    item.command = {
      command: "codez.openSession",
      title: "Open Session",
      arguments: [{ sessionId: element.session.id }],
    };
    return item;
  }

  public getChildren(element?: TreeNode): Thenable<TreeNode[]> {
    if (!element) {
      const grouped = new Map<string, Session[]>();
      for (const s of this.sessions.listAll()) {
        const list = grouped.get(s.workspaceFolderUri) ?? [];
        grouped.set(s.workspaceFolderUri, [...list, s]);
      }
      return Promise.resolve(
        [...grouped.entries()].map(([workspaceFolderUri, sessions]) => ({
          kind: "folder",
          label: toFolderLabel(sessions[0] ?? null) ?? workspaceFolderUri,
          workspaceFolderUri,
        })),
      );
    }

    if (element.kind === "folder") {
      const sessions = this.sessions
        .listByWorkspaceFolderUri(element.workspaceFolderUri ?? "")
        .filter((s) => s.workspaceFolderUri === element.workspaceFolderUri);
      const byBackendId = new Map<BackendId, Session[]>();
      for (const s of sessions) {
        const list = byBackendId.get(s.backendId) ?? [];
        byBackendId.set(s.backendId, [...list, s]);
      }
      const nodes: BackendNode[] = [...byBackendId.entries()]
        .sort(([a], [b]) => a.localeCompare(b))
        .map(([backendId, group]) => ({
          kind: "backend",
          backendId,
          workspaceFolderUri: element.workspaceFolderUri,
          label: `${backendId} (${group.length})`,
        }));
      return Promise.resolve(nodes);
    }

    if (element.kind === "backend") {
      const sessions = this.sessions
        .listByWorkspaceFolderUri(element.workspaceFolderUri ?? "")
        .filter(
          (s) =>
            s.workspaceFolderUri === element.workspaceFolderUri &&
            s.backendId === element.backendId,
        );
      return Promise.resolve(
        sessions.map((s, idx) => ({
          kind: "session",
          session: s,
          index: idx + 1,
        })),
      );
    }

    return Promise.resolve([]);
  }
}

type FolderNode = {
  kind: "folder";
  label: string;
  workspaceFolderUri: string | null;
};
type BackendNode = {
  kind: "backend";
  backendId: BackendId;
  label: string;
  workspaceFolderUri: string | null;
};
type SessionNode = { kind: "session"; session: Session; index: number };
type TreeNode = FolderNode | BackendNode | SessionNode;

function formatThreadId(threadId: string): string {
  const trimmed = threadId.trim();
  if (trimmed.length === 0) return "";
  if (trimmed.length <= 8) return `#${trimmed}`;
  return `#${trimmed.slice(0, 8)}`;
}

function normalizeTitle(title: string): string {
  const t = title.trim();
  const withoutShortId = t.replace(/\s*\([0-9a-f]{8}\)\s*$/i, "").trim();
  return withoutShortId.length > 0 ? withoutShortId : "(untitled)";
}

function toFolderLabel(session: Session | null): string | null {
  if (!session) return null;
  try {
    const uri = vscode.Uri.parse(session.workspaceFolderUri);
    return uri.fsPath;
  } catch {
    return null;
  }
}

const WORKTREE_COLOR_COUNT = 12;

function iconForColorIndex(
  extensionUri: vscode.Uri,
  idx: number,
): { light: vscode.Uri; dark: vscode.Uri } {
  const normalized = Math.trunc(idx);
  if (normalized < 0 || normalized >= WORKTREE_COLOR_COUNT) {
    throw new Error(`Invalid worktree color index: ${normalized}`);
  }
  const icon = vscode.Uri.joinPath(
    extensionUri,
    "resources",
    "worktree-colors",
    `dot-${normalized}.svg`,
  );
  return { light: icon, dark: icon };
}
