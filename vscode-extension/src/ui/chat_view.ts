import * as crypto from "node:crypto";
import * as path from "node:path";
import * as vscode from "vscode";

import type { Session } from "../sessions";

export type ChatBlock =
  | { id: string; type: "user"; text: string }
  | { id: string; type: "assistant"; text: string; streaming?: boolean }
  | {
      id: string;
      type: "divider";
      text: string;
      status?: "inProgress" | "completed" | "failed";
    }
  | { id: string; type: "note"; text: string }
  | {
      id: string;
      type: "image";
      title: string;
      src: string;
      // Offloaded images omit `src` and use `imageKey` to request data on-demand.
      imageKey?: string;
      mimeType?: string;
      byteLength?: number;
      autoLoad?: boolean;
      alt: string;
      caption: string | null;
      role: "user" | "assistant" | "tool" | "system";
    }
  | {
      id: string;
      type: "imageGallery";
      title: string;
      images: Array<{
        title: string;
        src: string;
        // Offloaded images omit `src` and use `imageKey` to request data on-demand.
        imageKey?: string;
        mimeType?: string;
        byteLength?: number;
        autoLoad?: boolean;
        alt: string;
        caption: string | null;
      }>;
      role: "user" | "assistant" | "tool" | "system";
    }
  | { id: string; type: "info"; title: string; text: string }
  | { id: string; type: "webSearch"; query: string; status: string }
  | {
      id: string;
      type: "reasoning";
      summaryParts: string[];
      rawParts: string[];
      status: string;
    }
  | {
      id: string;
      type: "command";
      title: string;
      status: string;
      command: string;
      hideCommandText?: boolean;
      actionsText?: string | null;
      cwd: string | null;
      exitCode: number | null;
      durationMs: number | null;
      terminalStdin: string[];
      output: string;
    }
  | {
      id: string;
      type: "fileChange";
      title: string;
      status: string;
      files: string[];
      detail: string;
      hasDiff: boolean;
      diffs?: Array<{ path: string; diff: string }>;
    }
  | {
      id: string;
      type: "mcp";
      title: string;
      status: string;
      server: string;
      tool: string;
      detail: string;
    }
  | { id: string; type: "plan"; title: string; text: string }
  | { id: string; type: "error"; title: string; text: string }
  | { id: string; type: "system"; title: string; text: string };

export type ChatViewState = {
  capabilities?: {
    agents: boolean;
    cliVariant: "unknown" | "codex" | "codex-mine";
  };
  customPrompts?: Array<{
    name: string;
    description: string | null;
    argumentHint: string | null;
    source: string;
  }>;
  globalBlocks?: ChatBlock[];
  sessions: Session[];
  activeSession: Session | null;
  unreadSessionIds: string[];
  runningSessionIds: string[];
  blocks: ChatBlock[];
  latestDiff: string | null;
  sending: boolean;
  reloading: boolean;
  statusText?: string | null;
  statusTooltip?: string | null;
  modelState?: {
    model: string | null;
    provider: string | null;
    reasoning: string | null;
  } | null;
  models?: Array<{
    id: string;
    model: string;
    displayName: string;
    description: string;
    supportedReasoningEfforts: Array<{
      reasoningEffort: string;
      description: string;
    }>;
    defaultReasoningEffort: string;
    isDefault: boolean;
  }> | null;
  approvals: Array<{
    requestKey: string;
    title: string;
    detail: string;
    canAcceptForSession: boolean;
  }>;
};

type RewindRequest = {
  turnIndex: number;
};

let sessionModelState: {
  model: string | null;
  provider: string | null;
  reasoning: string | null;
} = { model: null, provider: null, reasoning: null };

export function getSessionModelState(): {
  model: string | null;
  provider: string | null;
  reasoning: string | null;
} {
  return sessionModelState;
}

export function setSessionModelState(state: {
  model: string | null;
  provider: string | null;
  reasoning: string | null;
}): void {
  sessionModelState = state;
}

function asNullableString(v: unknown): string | null {
  return typeof v === "string" && v.trim().length > 0 ? v.trim() : null;
}

export class ChatViewProvider implements vscode.WebviewViewProvider {
  public static readonly viewType = "codexMine.chatView";

  private view: vscode.WebviewView | null = null;
  private refreshTimer: NodeJS.Timeout | null = null;
  private blockAppendFlushTimer: NodeJS.Timeout | null = null;
  private readonly pendingBlockAppends = new Map<
    string,
    {
      sessionId: string;
      blockId: string;
      field: "assistantText" | "commandOutput" | "fileChangeDetail";
      delta: string;
      streaming: boolean | null;
    }
  >();
  private statePostInFlight = false;
  private statePostDirty = false;
  private lastStatePostSeq = 0;
  private lastStateAckSeq = 0;
  private stateAckTimeout: NodeJS.Timeout | null = null;
  private blocksSessionIdSynced: string | null = null;
  private readonly fileSearchCancellationTokenBySessionId = new Map<
    string,
    string
  >();

  public insertIntoInput(text: string): void {
    this.view?.webview.postMessage({ type: "insertText", text });
  }

  public toast(kind: "info" | "success" | "error", message: string): void {
    this.view?.webview.postMessage({ type: "toast", kind, message });
  }

  public constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly getState: () => ChatViewState,
    private readonly onSend: (
      text: string,
      images?: Array<{ name: string; url: string }>,
      rewind?: RewindRequest | null,
    ) => Promise<void>,
    private readonly onFileSearch: (
      sessionId: string,
      query: string,
      cancellationToken: string,
    ) => Promise<string[]>,
    private readonly onListAgents: (sessionId: string) => Promise<string[]>,
    private readonly onListSkills: (sessionId: string) => Promise<
      Array<{
        name: string;
        description: string | null;
        scope: string;
        path: string;
      }>
    >,
    private readonly onLoadImage: (
      imageKey: string,
    ) => Promise<{ mimeType: string; base64: string }>,
    private readonly onOpenLatestDiff: () => Promise<void>,
    private readonly onUiError: (message: string) => void,
  ) {}

  public reveal(): void {
    this.view?.show?.(true);
  }

  public refresh(): void {
    // Avoid flooding the Webview with full-state updates (especially during streaming).
    this.statePostDirty = true;
    if (this.statePostInFlight) return;
    if (this.refreshTimer) return;
    this.refreshTimer = setTimeout(() => {
      this.refreshTimer = null;
      this.postControlState();
    }, 16);
  }

  public syncBlocksForActiveSession(): void {
    if (!this.view) return;
    const st = this.getState();
    const active = st.activeSession;
    if (!active) return;
    this.blocksSessionIdSynced = active.id;
    void this.view.webview
      .postMessage({
        type: "blocksReset",
        sessionId: active.id,
        blocks: st.blocks,
      })
      .then(undefined, (err) => {
        this.onUiError(`Failed to post blocks to webview: ${String(err)}`);
      });
  }

  public postBlockUpsert(sessionId: string, block: ChatBlock): void {
    if (!this.view) return;
    const st = this.getState();
    const active = st.activeSession;
    if (!active || active.id !== sessionId) return;
    void this.view.webview
      .postMessage({ type: "blockUpsert", sessionId, block })
      .then(undefined, (err) => {
        this.onUiError(
          `Failed to post block update to webview: ${String(err)}`,
        );
      });
  }

  public postBlockAppend(
    sessionId: string,
    blockId: string,
    field: "assistantText" | "commandOutput" | "fileChangeDetail",
    delta: string,
    opts?: { streaming?: boolean },
  ): void {
    if (!this.view) return;
    const st = this.getState();
    const active = st.activeSession;
    if (!active || active.id !== sessionId) return;
    const key = `${sessionId}:${blockId}:${field}`;
    const prev = this.pendingBlockAppends.get(key);
    if (prev) {
      prev.delta += delta;
      if (typeof opts?.streaming === "boolean") prev.streaming = opts.streaming;
    } else {
      this.pendingBlockAppends.set(key, {
        sessionId,
        blockId,
        field,
        delta,
        streaming: opts?.streaming ?? null,
      });
    }
    this.scheduleBlockAppendFlush();
  }

  public resolveWebviewView(view: vscode.WebviewView): void {
    this.view = view;
    view.webview.options = {
      enableScripts: true,
      localResourceRoots: [
        vscode.Uri.joinPath(this.context.extensionUri, "dist"),
        vscode.Uri.joinPath(this.context.extensionUri, "resources"),
      ],
    };
    view.webview.html = this.renderHtml(view.webview);
    view.webview.onDidReceiveMessage((msg: unknown) => {
      void this.onMessage(msg).catch((err) => {
        this.onUiError(`Failed to handle webview message: ${String(err)}`);
      });
    });
    view.onDidDispose(() => {
      this.view = null;
      this.statePostInFlight = false;
      this.statePostDirty = false;
      if (this.stateAckTimeout) clearTimeout(this.stateAckTimeout);
      this.stateAckTimeout = null;
      this.blocksSessionIdSynced = null;
      if (this.blockAppendFlushTimer) clearTimeout(this.blockAppendFlushTimer);
      this.blockAppendFlushTimer = null;
      this.pendingBlockAppends.clear();
    });
    this.statePostDirty = true;
    this.postControlState();
  }

  private scheduleBlockAppendFlush(): void {
    if (this.blockAppendFlushTimer) return;
    this.blockAppendFlushTimer = setTimeout(() => {
      this.blockAppendFlushTimer = null;
      this.flushBlockAppends();
    }, 16);
  }

  private flushBlockAppends(): void {
    if (!this.view) return;
    if (this.pendingBlockAppends.size === 0) return;
    const st = this.getState();
    const activeId = st.activeSession?.id ?? null;
    if (!activeId) {
      this.pendingBlockAppends.clear();
      return;
    }

    const pending = [...this.pendingBlockAppends.values()];
    this.pendingBlockAppends.clear();

    for (const p of pending) {
      if (p.sessionId !== activeId) continue;
      void this.view.webview
        .postMessage({
          type: "blockAppend",
          sessionId: p.sessionId,
          blockId: p.blockId,
          field: p.field,
          delta: p.delta,
          streaming: p.streaming,
        })
        .then(undefined, (err) => {
          this.onUiError(
            `Failed to post block delta to webview: ${String(err)}`,
          );
        });
    }
  }

  private async onMessage(msg: unknown): Promise<void> {
    if (typeof msg !== "object" || msg === null) return;
    const anyMsg = msg as Record<string, unknown>;
    const type = anyMsg["type"];

    if (type === "ready") {
      this.statePostDirty = true;
      this.postControlState();
      this.syncBlocksForActiveSession();
      return;
    }

    if (type === "stateAck") {
      const seq = anyMsg["seq"];
      if (typeof seq !== "number") return;
      if (seq > this.lastStateAckSeq) this.lastStateAckSeq = seq;
      // Only unblock when the latest in-flight state is acknowledged.
      if (seq === this.lastStatePostSeq) {
        this.statePostInFlight = false;
        if (this.stateAckTimeout) clearTimeout(this.stateAckTimeout);
        this.stateAckTimeout = null;
        if (this.statePostDirty) this.postControlState();
      }
      return;
    }

    if (type === "send") {
      const text = anyMsg["text"];
      const rewind = anyMsg["rewind"];
      if (typeof text !== "string") return;
      await this.onSend(text, [], (rewind as any) ?? null);
      return;
    }

    if (type === "sendWithImages") {
      const text = anyMsg["text"];
      const images = anyMsg["images"];
      const rewind = anyMsg["rewind"];
      if (typeof text !== "string") return;
      if (!Array.isArray(images)) return;
      const normalized = images
        .filter(
          (img) =>
            typeof img === "object" &&
            img !== null &&
            typeof (img as any).url === "string",
        )
        .map((img) => ({
          name: typeof (img as any).name === "string" ? (img as any).name : "",
          url: (img as any).url as string,
        }));
      await this.onSend(text, normalized, (rewind as any) ?? null);
      return;
    }

    if (type === "uiError") {
      const message = anyMsg["message"];
      if (typeof message !== "string") return;
      this.onUiError(message);
      return;
    }

    if (type === "loadImage") {
      const imageKey = anyMsg["imageKey"];
      const requestId = anyMsg["requestId"];
      if (typeof imageKey !== "string") return;
      if (typeof requestId !== "string") return;
      if (!this.view) return;

      try {
        const { mimeType, base64 } = await this.onLoadImage(imageKey);
        await this.view.webview.postMessage({
          type: "imageData",
          requestId,
          ok: true,
          mimeType,
          base64,
        });
      } catch (err) {
        await this.view.webview.postMessage({
          type: "imageData",
          requestId,
          ok: false,
          error: String(err),
        });
      }
      return;
    }

    if (type === "stop") {
      await vscode.commands.executeCommand("codexMine.interruptTurn");
      return;
    }

    if (type === "reloadSession") {
      await vscode.commands.executeCommand("codexMine.reloadSession");
      return;
    }

    if (type === "selectSession") {
      const sessionId = anyMsg["sessionId"];
      if (typeof sessionId !== "string") return;
      await vscode.commands.executeCommand("codexMine.selectSession", {
        sessionId,
      });
      return;
    }

    if (type === "renameSession") {
      const sessionId = anyMsg["sessionId"];
      if (typeof sessionId !== "string") return;
      await vscode.commands.executeCommand("codexMine.renameSession", {
        sessionId,
      });
      return;
    }

    if (type === "sessionMenu") {
      const sessionId = anyMsg["sessionId"];
      if (typeof sessionId !== "string") return;
      await vscode.commands.executeCommand("codexMine.sessionMenu", {
        sessionId,
      });
      return;
    }

    if (type === "newSession") {
      const st = this.getState();
      const active = st.activeSession;
      if (active) {
        await vscode.commands.executeCommand("codexMine.newSession", {
          workspaceFolderUri: active.workspaceFolderUri,
        });
      } else {
        await vscode.commands.executeCommand("codexMine.newSession");
      }
      return;
    }

    if (type === "newSessionPickFolder") {
      await vscode.commands.executeCommand("codexMine.newSession", {
        forcePickFolder: true,
      });
      return;
    }

    if (type === "resumeFromHistory") {
      await vscode.commands.executeCommand("codexMine.resumeFromHistory");
      return;
    }

    if (type === "showStatus") {
      await vscode.commands.executeCommand("codexMine.showStatus");
      return;
    }

    if (type === "selectCliVariant") {
      await vscode.commands.executeCommand("codexMine.selectCliVariant");
      return;
    }

    if (type === "setModel") {
      const model = asNullableString(anyMsg["model"]);
      const provider = asNullableString(anyMsg["provider"]);
      const reasoning = asNullableString(anyMsg["reasoning"]);
      setSessionModelState({ model, provider, reasoning });
      this.refresh();
      return;
    }

    if (type === "archiveSession") {
      // No-op: Codex UI VS Code extension does not support archiving sessions.
      return;
    }

    if (type === "openDiff") {
      await this.onOpenLatestDiff();
      return;
    }

    if (type === "openExternal") {
      const url = anyMsg["url"];
      if (typeof url !== "string") return;
      try {
        await vscode.env.openExternal(vscode.Uri.parse(url));
      } catch (err) {
        void vscode.window.showErrorMessage(
          `Failed to open URL: ${url} (${String(err)})`,
        );
      }
      return;
    }

    if (type === "openFile") {
      const rawPath = anyMsg["path"];
      if (typeof rawPath !== "string" || !rawPath) return;

      const st = this.getState();
      const active = st.activeSession;

      let filePath = rawPath;
      let line: number | null = null;
      let column: number | null = null;

      const hashIdx = rawPath.indexOf("#");
      if (hashIdx >= 0) {
        filePath = rawPath.slice(0, hashIdx);
        const frag = rawPath.slice(hashIdx + 1);
        const lcFrag = frag.match(/^L(\d+)(?:C(\d+))?$/i);
        if (lcFrag) {
          line = Number(lcFrag[1] || "") || null;
          column = Number(lcFrag[2] || "") || 1;
        }
      }

      const lcMatch = filePath.match(/^(.*?):(\d+)(?::(\d+))?$/);
      if (lcMatch) {
        filePath = lcMatch[1] || filePath;
        line = Number(lcMatch[2] || "") || null;
        column = Number(lcMatch[3] || "") || 1;
      }

      let uri: vscode.Uri;
      if (path.isAbsolute(filePath)) {
        uri = vscode.Uri.file(filePath);
      } else {
        if (!active) {
          void vscode.window.showErrorMessage(
            `Cannot open file (no active session): ${filePath}`,
          );
          return;
        }
        const folderUri = vscode.Uri.parse(active.workspaceFolderUri);
        const rootFsPath = folderUri.fsPath;
        const resolved = path.resolve(rootFsPath, filePath);
        const prefix = rootFsPath.endsWith(path.sep)
          ? rootFsPath
          : rootFsPath + path.sep;
        if (!(resolved === rootFsPath || resolved.startsWith(prefix))) {
          void vscode.window.showErrorMessage(
            `Cannot open paths outside the workspace: ${filePath}`,
          );
          return;
        }
        uri = vscode.Uri.file(resolved);
      }

      const options: Record<string, unknown> = {
        preview: true,
        preserveFocus: false,
      };
      if (line != null) {
        const l = Math.max(0, line - 1);
        const c = Math.max(0, (column ?? 1) - 1);
        const pos = new vscode.Position(l, c);
        options["selection"] = new vscode.Range(pos, pos);
      }
      // Delegate error handling to VS Code (no custom "No matching result" dialog).
      await vscode.commands.executeCommand("vscode.open", uri, options);
      return;
    }

    if (type === "approve") {
      const requestKey = anyMsg["requestKey"];
      const decision = anyMsg["decision"];
      if (typeof requestKey !== "string") return;
      if (typeof decision !== "string") return;
      await vscode.commands.executeCommand("codexMine.respondApproval", {
        requestKey,
        decision,
      });
      return;
    }

    if (type === "requestFileSearch") {
      const sessionId = anyMsg["sessionId"];
      if (typeof sessionId !== "string") return;
      const query = anyMsg["query"];
      if (typeof query !== "string") return;
      const st = this.getState();
      const active = st.activeSession;
      if (!active || active.id !== sessionId) {
        this.refresh();
        this.syncBlocksForActiveSession();
        return;
      }

      const folderUri = vscode.Uri.parse(active.workspaceFolderUri);
      const folder = vscode.workspace.getWorkspaceFolder(folderUri);
      if (!folder) {
        this.view?.webview.postMessage({
          type: "fileSearchResult",
          sessionId,
          query,
          paths: [],
        });
        return;
      }

      const norm = normalizeFileSearchQuery(query);
      if (!norm) {
        this.view?.webview.postMessage({
          type: "fileSearchResult",
          sessionId,
          query,
          paths: [],
        });
        return;
      }

      const cancellationToken =
        this.fileSearchCancellationTokenBySessionId.get(sessionId) ??
        crypto.randomUUID();
      this.fileSearchCancellationTokenBySessionId.set(
        sessionId,
        cancellationToken,
      );

      let paths: string[] = [];
      try {
        paths = await this.onFileSearch(sessionId, norm, cancellationToken);
      } catch (err) {
        console.error("[codex-mine] file search failed:", err);
        paths = [];
      }

      this.view?.webview.postMessage({
        type: "fileSearchResult",
        sessionId,
        query,
        paths,
      });
      return;
    }

    if (type === "requestAgentIndex") {
      const sessionId = anyMsg["sessionId"];
      if (typeof sessionId !== "string") return;
      const st = this.getState();
      const active = st.activeSession;
      if (!active || active.id !== sessionId) {
        this.refresh();
        this.syncBlocksForActiveSession();
        return;
      }

      const caps = st.capabilities ?? {
        agents: false,
        cliVariant: "unknown" as const,
      };
      if (!caps.agents) {
        this.view?.webview.postMessage({ type: "agentIndex", agents: [] });
        return;
      }

      let agents: string[] = [];
      try {
        agents = await this.onListAgents(sessionId);
      } catch (err) {
        console.error("[codex-mine] agents list failed:", err);
        agents = [];
      }

      this.view?.webview.postMessage({ type: "agentIndex", agents });
      return;
    }

    if (type === "requestSkillIndex") {
      const sessionId = anyMsg["sessionId"];
      if (typeof sessionId !== "string") return;
      const st = this.getState();
      const active = st.activeSession;
      if (!active || active.id !== sessionId) {
        this.refresh();
        this.syncBlocksForActiveSession();
        return;
      }

      let skills: Array<{
        name: string;
        description: string | null;
        scope: string;
        path: string;
      }> = [];
      try {
        skills = await this.onListSkills(sessionId);
      } catch (err) {
        console.error("[codex-mine] skills list failed:", err);
        skills = [];
      }

      this.view?.webview.postMessage({
        type: "skillIndex",
        sessionId,
        skills,
      });
      return;
    }

    if (type === "webviewError") {
      const message = anyMsg["message"];
      const stack = anyMsg["stack"];
      const details =
        typeof message === "string"
          ? message + (typeof stack === "string" && stack ? "\n" + stack : "")
          : JSON.stringify(anyMsg, null, 2);
      console.error("[codex-mine] webview error:", details);
      return;
    }
  }
  private postControlState(): void {
    if (!this.view) return;
    if (this.statePostInFlight) return;
    if (!this.statePostDirty) return;
    this.statePostDirty = false;
    this.statePostInFlight = true;
    const seq = (this.lastStatePostSeq += 1);
    if (this.stateAckTimeout) clearTimeout(this.stateAckTimeout);
    // If the webview stops acknowledging state updates (e.g. render stuck),
    // do not deadlock future refreshes; surface the issue and keep going.
    this.stateAckTimeout = setTimeout(() => {
      this.stateAckTimeout = null;
      if (!this.view) return;
      if (!this.statePostInFlight) return;
      this.statePostInFlight = false;
      this.onUiError(
        `Webview did not acknowledge state update (seq=${String(seq)}) within 2000ms; continuing.`,
      );
      if (this.statePostDirty) this.postControlState();
    }, 2000);
    const full = this.getState();
	    const controlState = {
	      globalBlocks: full.globalBlocks,
	      capabilities: full.capabilities,
	      sessions: full.sessions,
	      activeSession: full.activeSession,
	      unreadSessionIds: full.unreadSessionIds,
	      runningSessionIds: full.runningSessionIds,
	      latestDiff: full.latestDiff,
	      sending: full.sending,
	      reloading: full.reloading,
	      statusText: full.statusText,
	      statusTooltip: full.statusTooltip,
	      modelState: full.modelState,
	      models: full.models,
	      approvals: full.approvals,
	      customPrompts: full.customPrompts,
	    };
    void this.view.webview
      .postMessage({ type: "controlState", seq, state: controlState })
      .then(undefined, (err) => {
        // Unblock if postMessage itself failed (e.g., disposed webview).
        this.statePostInFlight = false;
        if (this.stateAckTimeout) clearTimeout(this.stateAckTimeout);
        this.stateAckTimeout = null;
        this.onUiError(`Failed to post state to webview: ${String(err)}`);
      });

    const activeId = full.activeSession?.id ?? null;
    if (activeId && activeId !== this.blocksSessionIdSynced) {
      this.blocksSessionIdSynced = activeId;
      void this.view.webview
        .postMessage({
          type: "blocksReset",
          sessionId: activeId,
          blocks: full.blocks,
        })
        .then(undefined, (err) => {
          this.onUiError(`Failed to post blocks to webview: ${String(err)}`);
        });
    }
  }

  private renderHtml(webview: vscode.Webview): string {
    // CSP nonce must match the CSP nonce grammar (base64 charset).
    // NOTE: UUID contains '-' which is not valid for CSP nonces and will block scripts.
    const nonce = crypto.randomBytes(16).toString("base64");
    const csp = [
      "default-src 'none'",
      "img-src " + webview.cspSource + " data: blob:",
      "style-src 'unsafe-inline'",
      `script-src ${webview.cspSource} 'nonce-${nonce}'`,
    ].join("; ");

    const clientScriptUri = webview.asWebviewUri(
      vscode.Uri.joinPath(
        this.context.extensionUri,
        "dist",
        "ui",
        "chat_view_client.js",
      ),
    );
    const markdownItUri = webview.asWebviewUri(
      vscode.Uri.joinPath(
        this.context.extensionUri,
        "resources",
        "vendor",
        "markdown-it.min.js",
      ),
    );
    const cacheBusted = (uri: vscode.Uri): vscode.Uri =>
      uri.with({ query: `v=${nonce}` });
    const clientScriptUriV = cacheBusted(clientScriptUri);
    const markdownItUriV = cacheBusted(markdownItUri);

    return `<!doctype html>
<html lang="ja">
  <head>
    <meta charset="UTF-8" />
    <meta http-equiv="Content-Security-Policy" content="${csp}" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <style>
      :root {
        --cm-font-family: var(--vscode-font-family, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif);
        --cm-font-size: var(--vscode-font-size, 13px);
        --cm-font-weight: var(--vscode-font-weight, 400);
        --cm-editor-font-family: var(--vscode-editor-font-family, var(--cm-font-family));
        --cm-editor-font-size: var(--vscode-editor-font-size, var(--cm-font-size));
        --cm-editor-font-weight: var(--vscode-editor-font-weight, var(--cm-font-weight));
        --cm-line-height: 1.55;
        --cm-chat-image-max-height: 360px;
      }

      body { font-family: var(--cm-font-family); font-size: var(--cm-font-size); font-weight: var(--cm-font-weight); line-height: var(--cm-line-height); -webkit-font-smoothing: antialiased; margin: 0; padding: 0; height: 100vh; display: flex; flex-direction: column; overflow-x: hidden; }
      .top { padding: 10px 12px; border-bottom: 1px solid rgba(127,127,127,0.3); display: flex; flex-direction: column; gap: 8px; }
      .topRow { display: flex; align-items: center; justify-content: space-between; gap: 10px; }
      .title { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
      .statusText { font-size: 12px; opacity: 0.75; white-space: pre-wrap; word-break: break-word; }
      .actions { display: flex; gap: 8px; }
      button { padding: 6px 10px; border-radius: 6px; border: 1px solid rgba(127,127,127,0.35); background: transparent; color: inherit; cursor: pointer; }
      button:disabled { opacity: 0.5; cursor: default; }
      button.iconBtn { width: 30px; min-width: 30px; height: 30px; padding: 0; display: inline-flex; align-items: center; justify-content: center; line-height: 1; }
      button.iconBtn::before { content: "➤"; font-size: 14px; opacity: 0.95; }
      button.iconBtn[data-mode="stop"]::before { content: "■"; font-size: 12px; }
      button.iconBtn.settingsBtn::before { content: "⚙"; font-size: 14px; }
      .footerBar { border-top: 1px solid rgba(127,127,127,0.25); padding: 8px 12px 10px; display: flex; flex-wrap: nowrap; gap: 10px; align-items: center; position: relative; }
      .modelBar { display: flex; flex-wrap: nowrap; gap: 8px; align-items: center; margin: 0; min-width: 0; flex: 1 1 auto; overflow: hidden; }
      .modelSelect { background: var(--vscode-input-background); color: inherit; border: 1px solid rgba(127,127,127,0.35); border-radius: 6px; padding: 4px 6px; }
      .modelSelect.model { width: 160px; max-width: 160px; }
      .modelSelect.effort { width: 110px; max-width: 110px; }
      .footerStatus { margin-left: auto; font-size: 12px; opacity: 0.75; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
      .footerStatus.clickable { cursor: pointer; }
      .statusPopover { position: absolute; right: 12px; bottom: calc(100% + 6px); max-width: min(520px, calc(100vw - 24px)); background: var(--vscode-editorHoverWidget-background, var(--vscode-input-background)); border: 1px solid rgba(127,127,127,0.35); border-radius: 10px; box-shadow: 0 6px 18px rgba(0,0,0,0.25); padding: 8px 10px; font-size: 12px; line-height: 1.5; white-space: pre-wrap; word-break: break-word; }
      .tabs { display: flex; gap: 6px; overflow: auto; padding-bottom: 2px; }
      .tab { padding: 6px 10px; border-radius: 999px; border: 1px solid rgba(127,127,127,0.35); cursor: pointer; white-space: nowrap; user-select: none; }
      .tab.active { border-color: rgba(0, 120, 212, 0.9); }
      .tab.unread { background: rgba(255, 185, 0, 0.14); }
      .tab.running { background: rgba(0, 120, 212, 0.12); }
      .log { flex: 1; overflow-y: auto; overflow-x: hidden; padding: 12px; }
      .approvals { padding: 12px; border-bottom: 1px solid rgba(127,127,127,0.25); display: flex; flex-direction: column; gap: 10px; }
      .approval { border: 1px solid rgba(127,127,127,0.25); border-radius: 10px; padding: 10px 12px; background: rgba(255, 120, 0, 0.10); }
      .approvalTitle { font-weight: 600; margin-bottom: 6px; }
      .approvalActions { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 8px; }
      .editBanner { border: 1px solid rgba(127,127,127,0.25); border-radius: 10px; padding: 8px 10px; margin: 0 0 8px; background: rgba(0, 120, 212, 0.10); display: flex; align-items: center; gap: 10px; }
      .editBannerText { flex: 1 1 auto; min-width: 0; font-size: 12px; opacity: 0.9; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
      .editBanner button { padding: 4px 8px; font-size: 12px; border-radius: 8px; }
      .msg { margin: 10px 0; padding: 10px 12px; border-radius: 10px; border: 1px solid rgba(127,127,127,0.25); }
      .note { margin: 8px 2px; font-size: 12px; opacity: 0.7; color: var(--vscode-descriptionForeground, inherit); }
      /* Keep user distinct from webSearch (both were blue-ish in dark themes). */
      .user { background: rgba(255,255,255,0.035); border-color: rgba(0, 120, 212, 0.35); }
      .assistant { background: rgba(0,0,0,0.06); }
      .system { background: rgba(255, 185, 0, 0.12); }
      .tool { background: rgba(153, 69, 255, 0.10); }
      .tool.changes { background: rgba(255, 140, 0, 0.10); }
      .tool.mcp { background: rgba(0, 200, 170, 0.08); }
      .tool.webSearch { background: rgba(0, 180, 255, 0.10); border-color: rgba(0, 180, 255, 0.22); }
      .reasoning { background: rgba(0, 169, 110, 0.12); }
      .divider { background: rgba(255, 185, 0, 0.06); border-style: dashed; position: relative; padding-right: 28px; }
      .imageBlock { display: flex; flex-direction: column; gap: 8px; }
      .imageBlock-user { background: rgba(255,255,255,0.035); border-color: rgba(0, 120, 212, 0.35); }
      .imageBlock-assistant { background: rgba(0,0,0,0.06); }
      .imageBlock-tool { background: rgba(0, 200, 170, 0.08); }
      .imageBlock-system { background: rgba(255, 185, 0, 0.12); }
      .imageTitle { font-weight: 600; font-size: 12px; opacity: 0.8; }
      .imageCaption { font-size: 12px; opacity: 0.7; word-break: break-word; }
      .imageContent { width: 100%; max-width: 100%; height: auto; max-height: var(--cm-chat-image-max-height); object-fit: contain; border-radius: 8px; border: 1px solid rgba(127,127,127,0.25); background: rgba(0,0,0,0.02); }
      .imageGallery { display: flex; flex-direction: column; gap: 8px; }
      .imageGallery-user { background: rgba(255,255,255,0.035); border-color: rgba(0, 120, 212, 0.35); }
      .imageGallery-assistant { background: rgba(0,0,0,0.06); }
      .imageGallery-tool { background: rgba(0, 200, 170, 0.08); }
      .imageGallery-system { background: rgba(255, 185, 0, 0.12); }
      .imageGalleryTitle { font-weight: 600; font-size: 12px; opacity: 0.8; }
      .imageGalleryGrid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
      .imageGalleryTile { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
      .imageGalleryCaption { font-size: 12px; opacity: 0.7; word-break: break-word; }
      .imageGalleryImage { width: 100%; max-width: 100%; height: auto; max-height: min(240px, var(--cm-chat-image-max-height)); object-fit: contain; border-radius: 8px; border: 1px solid rgba(127,127,127,0.25); background: rgba(0,0,0,0.02); }
      details { border-radius: 10px; border: 1px solid rgba(127,127,127,0.25); padding: 4px 12px; margin: 5px 0; }
      details.notice { background: rgba(127,127,127,0.04); }
      details.notice.info { background: rgba(255,255,255,0.06); }
      details.notice.debug { background: rgba(255, 185, 0, 0.08); }
      details > summary { cursor: pointer; font-weight: 600; position: relative; padding-right: 8px; display: flex; align-items: center; gap: 8px; }
      details > summary > span[data-k="summaryText"] { flex: 1 1 auto; min-width: 0; }
      details > summary > span.statusIcon { position: static; top: auto; right: auto; transform: none; margin-left: auto; }
      .webSearchCard { position: relative; padding-right: 28px; }
      .webSearchCard .statusIcon { top: 12px; transform: none; }
      .divider .statusIcon { top: 12px; transform: none; }
      .webSearchRow { position: relative; }
      .statusIcon { position: absolute; right: 10px; top: 50%; transform: translateY(-50%); width: 16px; height: 16px; opacity: 0.9; }
      .statusIcon::before, .statusIcon::after { content: ""; display: block; box-sizing: border-box; }
      .msgHeader { display: flex; align-items: center; justify-content: space-between; gap: 10px; margin-bottom: 6px; }
      .msgHeaderTitle { font-size: 12px; opacity: 0.7; }
      .msgActions { display: flex; gap: 8px; }
      .msgActionBtn { padding: 2px 8px; font-size: 12px; border-radius: 999px; }

      /* inProgress: spinner */
      .statusIcon.status-inProgress::before { width: 14px; height: 14px; border: 2px solid rgba(180, 180, 180, 0.95); border-top-color: rgba(180, 180, 180, 0.15); border-radius: 50%; animation: cmSpin 0.9s linear infinite; margin: 1px; }
      @keyframes cmSpin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }

      /* completed: check */
      .statusIcon.status-completed::before { width: 6px; height: 10px; border-right: 2px solid rgba(180, 180, 180, 0.95); border-bottom: 2px solid rgba(180, 180, 180, 0.95); transform: rotate(45deg); margin: 1px 0 0 6px; }

      /* failed: X */
      .statusIcon.status-failed::before, .statusIcon.status-failed::after { position: absolute; left: 7px; top: 2px; width: 2px; height: 12px; background: rgba(180, 180, 180, 0.95); border-radius: 1px; }
      .statusIcon.status-failed::before { transform: rotate(45deg); }
      .statusIcon.status-failed::after { transform: rotate(-45deg); }

      /* declined/cancelled: minus */
      .statusIcon.status-declined::before, .statusIcon.status-cancelled::before { width: 12px; height: 2px; background: rgba(180, 180, 180, 0.95); border-radius: 1px; margin: 7px 0 0 2px; }
      .meta { font-size: 12px; opacity: 0.75; margin: 6px 0 0 0; }
      .tool .meta { font-size: 11px; opacity: 0.65; margin-top: 10px; }
      pre { margin: 0; white-space: pre-wrap; word-break: break-word; font-family: var(--cm-editor-font-family); font-size: var(--cm-editor-font-size); font-weight: var(--cm-editor-font-weight); line-height: var(--cm-line-height); }
      .md { line-height: var(--cm-line-height); }
      .md > :first-child { margin-top: 0; }
      .md > :last-child { margin-bottom: 0; }
      .md p { margin: 8px 0; }
      .md ul, .md ol { margin: 8px 0 8px 22px; padding: 0; }
      .md li { margin: 4px 0; }
      .md blockquote { margin: 8px 0; padding: 8px 10px; border-left: 3px solid rgba(127,127,127,0.35); background: rgba(127,127,127,0.05); color: var(--vscode-descriptionForeground, inherit); }
      .md blockquote strong, .md blockquote b { font-weight: inherit; }
      .md blockquote em { font-style: italic; opacity: 0.95; }
      .md hr { border: 0; border-top: 1px solid rgba(127,127,127,0.25); margin: 10px 0; }
      .md h1, .md h2, .md h3 { margin: 12px 0 8px; line-height: 1.25; }
      .md h1 { font-size: 1.35em; }
      .md h2 { font-size: 1.2em; }
      .md h3 { font-size: 1.1em; }
      .md code { font-family: var(--cm-editor-font-family); font-size: 0.95em; background: rgba(127,127,127,0.15); padding: 0 4px; border-radius: 4px; }
      .md pre code { background: transparent; padding: 0; }
      .md pre { background: rgba(127,127,127,0.10); padding: 10px 12px; border-radius: 8px; overflow-x: auto; }
      .md a { color: var(--vscode-textLink-foreground, rgba(0,120,212,0.9)); text-decoration: underline; }
      .md a:hover { color: var(--vscode-textLink-activeForeground, rgba(0,120,212,1)); }
      .composer { border-top: 1px solid rgba(127,127,127,0.3); padding: 10px 12px; display: flex; flex-direction: column; gap: 8px; position: relative; }
      .returnToBottomBtn { position: absolute; left: 50%; transform: translateX(-50%); border: 1px solid rgba(127,127,127,0.35); border-radius: 999px; padding: 4px 10px; background: rgba(127,127,127,0.08); color: inherit; opacity: 0.45; cursor: pointer; font-size: 12px; display: none; align-items: center; justify-content: center; z-index: 30; }
      .returnToBottomBtn:hover { opacity: 0.9; background: rgba(127,127,127,0.14); }
      .inputRow { display: flex; gap: 8px; align-items: flex-end; }
      textarea { flex: 1; resize: none; box-sizing: border-box; border-radius: 8px; border: 1px solid rgba(127,127,127,0.35); padding: 6px 10px; background: transparent; color: inherit; font-family: var(--cm-editor-font-family); font-size: var(--cm-editor-font-size); font-weight: var(--cm-editor-font-weight); line-height: 1.2; overflow-y: hidden; min-height: 30px; max-height: 200px; }
      textarea::placeholder { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
      .suggest { position: absolute; left: 12px; right: 12px; bottom: calc(100% + 8px); border: 1px solid var(--vscode-editorSuggestWidget-border, rgba(127,127,127,0.35)); border-radius: 10px; background: var(--vscode-editorSuggestWidget-background, rgba(30,30,30,0.95)); color: var(--vscode-editorSuggestWidget-foreground, inherit); max-height: 160px; overflow: auto; display: none; z-index: 20; box-shadow: 0 8px 24px rgba(0,0,0,0.35); }
      button.iconBtn.attachBtn::before { content: "📎"; font-size: 14px; }
      .attachments { display: none; flex-wrap: wrap; gap: 8px; }
      .attachmentChip { border: 1px solid rgba(127,127,127,0.35); border-radius: 10px; padding: 6px 8px; font-size: 12px; display: inline-flex; gap: 8px; align-items: center; max-width: 320px; }
      .attachmentThumb { width: 44px; height: 44px; object-fit: cover; border-radius: 8px; border: 1px solid rgba(127,127,127,0.25); background: rgba(0,0,0,0.02); flex: 0 0 auto; }
      .attachmentName { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; opacity: 0.9; }
      .attachmentRemove { cursor: pointer; opacity: 0.7; }
      .suggestItem { padding: 8px 10px; cursor: pointer; display: flex; justify-content: space-between; gap: 10px; }
      .suggestItem:hover { background: var(--vscode-list-hoverBackground, rgba(255,255,255,0.06)); }
      .suggestItem.active { background: var(--vscode-list-activeSelectionBackground, rgba(0,120,212,0.25)); }
      .suggestRight { opacity: 0.7; font-size: 12px; white-space: nowrap; }
      .fileList { margin-top: 6px; }
      .fileRow { margin: 2px 0; }
      .fileLink { color: var(--vscode-textLink-foreground, rgba(0,120,212,0.9)); text-decoration: underline; cursor: pointer; font-family: var(--cm-editor-font-family); font-size: var(--cm-editor-font-size); }
      .fileLink:hover { color: var(--vscode-textLink-activeForeground, rgba(0,120,212,1)); }
      .autoFileLink { color: inherit; text-decoration: none; cursor: text; }
      .autoFileLink.modHover { color: var(--vscode-textLink-foreground, rgba(0,120,212,0.9)); text-decoration: underline; cursor: pointer; }
      .autoFileLink.modHover:hover { color: var(--vscode-textLink-activeForeground, rgba(0,120,212,1)); }
      .autoUrlLink { color: inherit; text-decoration: none; cursor: text; }
      .autoUrlLink.modHover { color: var(--vscode-textLink-foreground, rgba(0,120,212,0.9)); text-decoration: underline; cursor: pointer; }
      .autoUrlLink.modHover:hover { color: var(--vscode-textLink-activeForeground, rgba(0,120,212,1)); }
	      .fileDiff { margin-top: 8px; }
	      .toast { position: fixed; top: 16px; left: 50%; transform: translateX(-50%); z-index: 1000; max-width: min(820px, calc(100vw - 32px)); border-radius: 10px; padding: 10px 12px; border: 1px solid rgba(127,127,127,0.35); box-shadow: 0 10px 30px rgba(0,0,0,0.35); background: var(--vscode-notifications-background, rgba(30,30,30,0.95)); color: var(--vscode-notifications-foreground, inherit); display: none; }
	      .toast.info { border-color: rgba(127,127,127,0.35); }
	      .toast.success { border-color: rgba(0,200,120,0.55); }
	      .toast.error { border-color: rgba(220,60,60,0.60); }
	    </style>
	  </head>
	  <body>
    <div class="top">
      <div class="topRow">
        <div id="title" class="title">Codex UI</div>
        <div class="actions">
          <button id="new">New</button>
          <button id="resume">Resume</button>
          <button id="reload" title="Reload session (codex-mine only)" disabled>Reload</button>
          <button id="settings" class="iconBtn settingsBtn" aria-label="Settings" title="Settings"></button>
        </div>
      </div>
      <div id="tabs" class="tabs"></div>
    </div>
    <div id="approvals" class="approvals" style="display:none"></div>
    <div id="log" class="log"></div>
    <div id="composer" class="composer">
      <div id="editBanner" class="editBanner" style="display:none"></div>
      <div id="attachments" class="attachments"></div>
      <button id="returnToBottom" class="returnToBottomBtn" title="Scroll to bottom">Return to Bottom</button>
      <div id="inputRow" class="inputRow">
        <input id="imageInput" type="file" accept="image/*" multiple style="display:none" />
        <button id="attach" class="iconBtn attachBtn" aria-label="Attach image" title="Attach image"></button>
        <textarea id="input" rows="1" placeholder="Type a message"></textarea>
        <button id="send" class="iconBtn" data-mode="send" aria-label="Send" title="Send (Esc: stop)"></button>
      </div>
      <div id="suggest" class="suggest"></div>
    </div>
	    <div class="footerBar">
	      <div id="modelBar" class="modelBar"></div>
	      <div id="statusText" class="footerStatus" style="display:none"></div>
	          <div id="statusPopover" class="statusPopover" style="display:none"></div>
	    </div>
	    <div id="toast" class="toast"></div>
	    <script nonce="${nonce}" src="${markdownItUriV}"></script>
	    <script nonce="${nonce}" src="${clientScriptUriV}"></script>
	  </body>
	</html>`;
  }
}

function normalizeFileSearchQuery(query: string): string | null {
  const q = query.trim().replace(/\\/g, "/");
  if (!q) return null;
  // Disallow path traversal / absolute-ish queries; this is purely a search hint.
  if (q.includes("..")) return null;
  if (q.startsWith("/")) return q.slice(1);
  return q;
}

function buildFileSearchIncludeGlob(query: string): string {
  // VS Code uses glob patterns (minimatch-like). We treat the user query as a
  // literal substring hint by escaping special characters.
  const q = escapeGlobLiteral(query);

  // If the user typed a trailing '/', treat it as a workspace-relative directory
  // prefix so the user can drill down after selecting a directory (e.g. "src/").
  if (query.endsWith("/")) {
    const rawTrimmed = query.replace(/\/+$/, "");
    const trimmed = escapeGlobLiteral(rawTrimmed);
    if (!trimmed) return "**/*";
    return `${trimmed}/**`;
  }

  // If the user already includes a '/', treat it as a workspace-relative prefix
  // to keep navigation predictable and enable directory drill-down.
  if (query.includes("/")) {
    const lastSlash = query.lastIndexOf("/");
    const baseRaw = query.slice(0, lastSlash + 1);
    const leafRaw = query.slice(lastSlash + 1);
    const base = escapeGlobLiteral(baseRaw);
    const leaf = escapeGlobLiteral(leafRaw);
    if (!leafRaw) return `${base}**`;
    return `${base}**/*${leaf}*`;
  }

  return `**/*${q}*`;
}

function escapeGlobLiteral(input: string): string {
  // Escape glob metacharacters for VS Code's glob.
  // minimatch treats backslash as escape; VS Code globs also accept it.
  return input.replace(/[\\{}()[\]*?]/g, "\\$&");
}
