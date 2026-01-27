import * as vscode from "vscode";

import type { Session } from "../sessions";

export type SessionPanelChatLine =
  | { kind: "user"; text: string }
  | { kind: "assistant"; text: string }
  | { kind: "system"; text: string };

export class SessionPanel implements vscode.Disposable {
  private readonly panel: vscode.WebviewPanel;
  private readonly transcript: SessionPanelChatLine[] = [];
  private latestDiff: string | null = null;
  private baseTitle: string;
  private unread = false;
  private pendingAssistantDelta = "";
  private assistantDeltaFlushTimer: NodeJS.Timeout | null = null;

  public constructor(
    private readonly context: vscode.ExtensionContext,
    public readonly session: Session,
    private readonly onDispose: (sessionId: string) => void,
  ) {
    this.baseTitle = session.title;
    this.panel = vscode.window.createWebviewPanel(
      "codez.session",
      this.baseTitle,
      { viewColumn: vscode.ViewColumn.Beside, preserveFocus: true },
      {
        enableScripts: true,
        retainContextWhenHidden: true,
      },
    );

    this.panel.onDidChangeViewState(
      () => {
        if (this.panel.active) this.clearUnread();
      },
      null,
      this.context.subscriptions,
    );

    vscode.window.onDidChangeWindowState(
      (e) => {
        if (e.focused && this.panel.active) this.clearUnread();
      },
      null,
      this.context.subscriptions,
    );

    this.panel.onDidDispose(
      () => {
        this.onDispose(this.session.id);
        this.dispose();
      },
      null,
      this.context.subscriptions,
    );
    this.panel.webview.onDidReceiveMessage(
      (msg: unknown) => this.onMessage(msg),
      null,
      this.context.subscriptions,
    );

    this.render();
  }

  public show(preserveFocus = true): void {
    this.panel.reveal(undefined, preserveFocus);
  }

  public updateTitle(title: string): void {
    this.baseTitle = title;
    this.panel.title = this.unread ? `● ${this.baseTitle}` : this.baseTitle;
  }

  public dispose(): void {
    // no-op; panel disposal is handled by VS Code
  }

  public setLatestDiff(diff: string): void {
    this.latestDiff = diff;
    this.postState();
  }

  public setTranscript(transcript: SessionPanelChatLine[]): void {
    this.transcript.length = 0;
    this.transcript.push(...transcript);
    this.postState();
  }

  public addUserMessage(text: string): void {
    this.transcript.push({ kind: "user", text });
    this.transcript.push({ kind: "assistant", text: "" });
    this.postState();
  }

  public appendAssistantDelta(delta: string): void {
    const last = this.transcript.at(-1);
    if (!last || last.kind !== "assistant") {
      // The webview is not guaranteed to have an "active" assistant bubble yet.
      // Keep the slow full-state render for this structural change.
      this.transcript.push({ kind: "assistant", text: delta });
      this.postState();
    } else {
      last.text += delta;
      this.pendingAssistantDelta += delta;
      if (this.pendingAssistantDelta.length >= 64 * 1024) {
        this.flushAssistantDelta();
      } else {
        this.scheduleAssistantDeltaFlush();
      }
    }
    this.markUnreadIfInactive();
  }

  public addSystemMessage(text: string): void {
    this.transcript.push({ kind: "system", text });
    this.postState();
  }

  public markUnread(): void {
    this.markUnreadIfInactive();
  }

  private clearUnread(): void {
    if (!this.unread) return;
    this.unread = false;
    this.panel.title = this.baseTitle;
  }

  private markUnreadIfInactive(): void {
    if (this.panel.active && this.panel.visible && vscode.window.state.focused)
      return;
    if (this.unread) return;
    this.unread = true;
    this.panel.title = `● ${this.baseTitle}`;
  }

  private render(): void {
    const nonce = String(Date.now());
    this.panel.webview.html = `<!doctype html>
<html lang="ja">
  <head>
    <meta charset="UTF-8" />
    <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'nonce-${nonce}';" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>${escapeHtml(this.session.title)}</title>
    <style>
      body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; margin: 0; padding: 0; }
      .topbar { display: flex; align-items: center; justify-content: space-between; padding: 10px 12px; border-bottom: 1px solid rgba(127,127,127,0.3); }
      .title { font-weight: 600; }
      .actions { display: flex; gap: 8px; }
      button { padding: 6px 10px; border-radius: 6px; border: 1px solid rgba(127,127,127,0.35); background: transparent; color: inherit; cursor: pointer; }
      button:disabled { opacity: 0.5; cursor: default; }
      .log { padding: 12px; }
      .msg { margin: 10px 0; padding: 10px 12px; border-radius: 10px; border: 1px solid rgba(127,127,127,0.25); }
      .user { background: rgba(0, 120, 212, 0.08); }
      .assistant { background: rgba(0,0,0,0.06); }
      .system { background: rgba(255, 185, 0, 0.12); }
      pre { margin: 0; white-space: pre-wrap; word-break: break-word; }
    </style>
  </head>
  <body>
    <div class="topbar">
      <div class="title">${escapeHtml(this.session.title)}</div>
      <div class="actions">
        <button id="send">Send Message</button>
        <button id="diff" disabled>Open Latest Diff</button>
      </div>
    </div>
    <div id="log" class="log"></div>

    <script nonce="${nonce}">
      const vscode = acquireVsCodeApi();
      const sendBtn = document.getElementById("send");
      const diffBtn = document.getElementById("diff");
      const logEl = document.getElementById("log");

      function render(state) {
        diffBtn.disabled = !state.latestDiff;
        logEl.innerHTML = "";
        for (const line of state.transcript) {
          const div = document.createElement("div");
          div.className = "msg " + line.kind;
          const pre = document.createElement("pre");
          pre.textContent = line.text;
          div.appendChild(pre);
          logEl.appendChild(div);
        }
        window.scrollTo(0, document.body.scrollHeight);
      }

      window.addEventListener("message", (event) => {
        const msg = event.data;
        if (!msg) return;
        if (msg.type === "state") {
          render(msg.state);
          return;
        }
        if (msg.type === "assistantDelta") {
          const delta = typeof msg.delta === "string" ? msg.delta : "";
          if (!delta) return;
          const preEls = logEl.querySelectorAll(".msg.assistant pre");
          const pre = preEls.length > 0 ? preEls[preEls.length - 1] : null;
          if (!pre) return;

          const last = pre.lastChild;
          if (last && last.nodeType === Node.TEXT_NODE) {
            last.appendData(delta);
          } else {
            pre.appendChild(document.createTextNode(delta));
          }
          window.scrollTo(0, document.body.scrollHeight);
          return;
        }
      });

      sendBtn.addEventListener("click", () => {
        vscode.postMessage({ type: "sendMessage" });
      });
      diffBtn.addEventListener("click", () => {
        vscode.postMessage({ type: "openDiff" });
      });

      vscode.postMessage({ type: "ready" });
    </script>
  </body>
</html>`;
  }

  private postState(): void {
    this.panel.webview.postMessage({
      type: "state",
      state: {
        transcript: this.transcript,
        latestDiff: this.latestDiff,
      },
    });
  }

  private postAssistantDelta(delta: string): void {
    this.panel.webview.postMessage({ type: "assistantDelta", delta });
  }

  private scheduleAssistantDeltaFlush(): void {
    if (this.assistantDeltaFlushTimer) return;
    this.assistantDeltaFlushTimer = setTimeout(() => {
      this.assistantDeltaFlushTimer = null;
      this.flushAssistantDelta();
    }, 16);
  }

  private flushAssistantDelta(): void {
    const delta = this.pendingAssistantDelta;
    if (!delta) return;
    this.pendingAssistantDelta = "";
    this.postAssistantDelta(delta);
  }

  private onMessage(msg: unknown): void {
    if (typeof msg !== "object" || msg === null) return;
    const anyMsg = msg as Record<string, unknown>;
    const type = anyMsg["type"];
    if (type === "ready") {
      this.postState();
      return;
    }
    if (type === "sendMessage") {
      void vscode.commands.executeCommand("codez.sendMessage", {
        sessionId: this.session.id,
      });
      return;
    }
    if (type === "openDiff") {
      void vscode.commands.executeCommand("codez.openLatestDiff", {
        sessionId: this.session.id,
      });
      return;
    }
  }
}

function escapeHtml(text: string): string {
  return text.replace(/[&<>"']/g, (c) => {
    switch (c) {
      case "&":
        return "&amp;";
      case "<":
        return "&lt;";
      case ">":
        return "&gt;";
      case '"':
        return "&quot;";
      case "'":
        return "&#39;";
      default:
        return c;
    }
  });
}
