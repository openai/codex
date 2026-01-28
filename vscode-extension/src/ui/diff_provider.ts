import * as vscode from "vscode";

type DiffState = {
  title: string;
  diff: string;
};

export class DiffDocumentProvider
  implements vscode.TextDocumentContentProvider, vscode.Disposable
{
  private readonly emitter = new vscode.EventEmitter<vscode.Uri>();
  private readonly diffs = new Map<string, DiffState>();

  public readonly onDidChange = this.emitter.event;

  public dispose(): void {
    this.emitter.dispose();
    this.diffs.clear();
  }

  public set(uri: vscode.Uri, state: DiffState): void {
    this.diffs.set(uri.toString(), state);
    this.emitter.fire(uri);
  }

  public provideTextDocumentContent(uri: vscode.Uri): string {
    const state = this.diffs.get(uri.toString());
    if (!state) return "No diff available yet.";
    return state.diff;
  }
}

export function makeDiffUri(sessionId: string): vscode.Uri {
  return vscode.Uri.parse(`codez-diff:${encodeURIComponent(sessionId)}.diff`);
}
