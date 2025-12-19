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

    const item = new vscode.TreeItem(
      element.session.title,
      vscode.TreeItemCollapsibleState.None,
    );
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
          .map((s) => ({ kind: "session", session: s })),
      );
    }

    return Promise.resolve([]);
  }
}

type FolderNode = { kind: "folder"; backendKey: string; label: string };
type SessionNode = { kind: "session"; session: Session };
type TreeNode = FolderNode | SessionNode;

function toFolderLabel(session: Session | null): string | null {
  if (!session) return null;
  try {
    const uri = vscode.Uri.parse(session.workspaceFolderUri);
    return uri.fsPath;
  } catch {
    return null;
  }
}
