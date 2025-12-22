import * as vscode from "vscode";

import type { Session } from "../sessions";
import type { SessionStore } from "../sessions";

export class SessionTreeDataProvider
  implements vscode.TreeDataProvider<TreeNode>, vscode.Disposable
{
  private readonly emitter = new vscode.EventEmitter<TreeNode | null>();
  public readonly onDidChangeTreeData = this.emitter.event;

  public onDidSelectSession: ((sessionId: string) => void) | null = null;

  public constructor(private readonly sessions: SessionStore) {}

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
      item.contextValue = "codexMine.folder";
      return item;
    }

    const title = normalizeTitle(element.session.title);
    const label = element.session.customTitle
      ? title
      : `${title} #${element.index}`;
    const item = new vscode.TreeItem(label, vscode.TreeItemCollapsibleState.None);
    // Show full thread id in description for copyability; omit short id in label.
    item.description = element.session.threadId;
    item.contextValue = "codexMine.session";
    item.command = {
      command: "codexMine.openSession",
      title: "Open Session",
      arguments: [{ sessionId: element.session.id }],
    };
    return item;
  }

  public getChildren(element?: TreeNode): Thenable<TreeNode[]> {
    if (!element) {
      const grouped = new Map<string, Session[]>();
      for (const s of this.sessions.listAll()) {
        const list = grouped.get(s.backendKey) ?? [];
        grouped.set(s.backendKey, [...list, s]);
      }
      return Promise.resolve(
        [...grouped.entries()].map(([backendKey, sessions]) => ({
          kind: "folder",
          backendKey,
          label: toFolderLabel(sessions[0] ?? null) ?? backendKey,
        })),
      );
    }

    if (element.kind === "folder") {
      return Promise.resolve(
        this.sessions
          .list(element.backendKey)
          .map((s, idx) => ({ kind: "session", session: s, index: idx + 1 })),
      );
    }

    return Promise.resolve([]);
  }
}

type FolderNode = { kind: "folder"; backendKey: string; label: string };
type SessionNode = { kind: "session"; session: Session; index: number };
type TreeNode = FolderNode | SessionNode;

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
