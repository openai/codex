import * as vscode from "vscode";

import type { Session } from "../sessions";
import type { ChatBlock } from "./chat_view";
import { SessionPanel, type SessionPanelChatLine } from "./session_panel";

type InitialState = {
  blocks: ChatBlock[];
  latestDiff: string | null;
};

export class SessionPanelManager implements vscode.Disposable {
  private readonly panelsBySessionId = new Map<string, SessionPanel>();

  public constructor(private readonly context: vscode.ExtensionContext) {}

  public dispose(): void {
    // WebviewPanel disposal is owned by VS Code; clear references to avoid leaks.
    this.panelsBySessionId.clear();
  }

  public open(
    session: Session,
    initial: InitialState | null,
    preserveFocus = false,
  ): void {
    const existing = this.panelsBySessionId.get(session.id);
    if (existing) {
      existing.updateTitle(session.title);
      if (initial) {
        if (initial.latestDiff) existing.setLatestDiff(initial.latestDiff);
        existing.setTranscript(blocksToTranscript(initial.blocks));
      }
      existing.show(preserveFocus);
      return;
    }

    const panel = new SessionPanel(this.context, session, (sessionId) => {
      this.panelsBySessionId.delete(sessionId);
    });
    this.panelsBySessionId.set(session.id, panel);

    if (initial) {
      if (initial.latestDiff) panel.setLatestDiff(initial.latestDiff);
      panel.setTranscript(blocksToTranscript(initial.blocks));
    }

    panel.show(preserveFocus);
  }

  public updateTitle(sessionId: string, title: string): void {
    this.panelsBySessionId.get(sessionId)?.updateTitle(title);
  }

  public addUserMessage(sessionId: string, text: string): void {
    this.panelsBySessionId.get(sessionId)?.addUserMessage(text);
  }

  public appendAssistantDelta(sessionId: string, delta: string): void {
    this.panelsBySessionId.get(sessionId)?.appendAssistantDelta(delta);
  }

  public addSystemMessage(sessionId: string, text: string): void {
    this.panelsBySessionId.get(sessionId)?.addSystemMessage(text);
  }

  public setLatestDiff(sessionId: string, diff: string): void {
    this.panelsBySessionId.get(sessionId)?.setLatestDiff(diff);
  }

  public markTurnCompleted(sessionId: string): void {
    this.panelsBySessionId.get(sessionId)?.markUnread();
  }
}

function blocksToTranscript(blocks: ChatBlock[]): SessionPanelChatLine[] {
  const out: SessionPanelChatLine[] = [];
  for (const b of blocks) {
    if (b.type === "user") {
      out.push({ kind: "user", text: b.text });
      continue;
    }
    if (b.type === "assistant") {
      out.push({ kind: "assistant", text: b.text });
      continue;
    }
    if (b.type === "system") {
      out.push({
        kind: "system",
        text: b.title ? `${b.title}\n${b.text}` : b.text,
      });
      continue;
    }
    if (b.type === "error") {
      out.push({ kind: "system", text: `${b.title}\n${b.text}` });
      continue;
    }
  }
  return out;
}
