import * as vscode from "vscode";
import { randomUUID } from "node:crypto";
import { BackendProcess } from "./process";
import type { ThreadResumeParams } from "../generated/v2/ThreadResumeParams";
import type { ThreadStartParams } from "../generated/v2/ThreadStartParams";
import type { TurnStartParams } from "../generated/v2/TurnStartParams";
import type { UserInput } from "../generated/v2/UserInput";
import type { ThreadItem } from "../generated/v2/ThreadItem";
import type { Session, SessionStore } from "../sessions";
import type { ApprovalDecision } from "../generated/v2/ApprovalDecision";
import type { ServerRequest } from "../generated/ServerRequest";
import type { ThreadResumeResponse } from "../generated/v2/ThreadResumeResponse";
import type { ModelListResponse } from "../generated/v2/ModelListResponse";
import type { Model } from "../generated/v2/Model";
import type { ReasoningEffort } from "../generated/ReasoningEffort";
import type { GetAccountResponse } from "../generated/v2/GetAccountResponse";
import type { GetAccountRateLimitsResponse } from "../generated/v2/GetAccountRateLimitsResponse";
import type { SkillsListEntry } from "../generated/v2/SkillsListEntry";
import type { Thread } from "../generated/v2/Thread";
import type { AnyServerNotification } from "./types";
import type { FuzzyFileSearchResponse } from "../generated/FuzzyFileSearchResponse";

type ModelSettings = {
  model: string | null;
  provider: string | null;
  reasoning: string | null;
};

export class BackendManager implements vscode.Disposable {
  private readonly processes = new Map<string, BackendProcess>();
  private readonly streamState = new Map<
    string,
    { activeTurnId: string | null }
  >();
  private readonly latestDiffByThreadId = new Map<string, string>();
  private readonly modelsByBackendKey = new Map<string, Model[]>();
  private readonly itemsByThreadId = new Map<string, Map<string, ThreadItem>>();

  public onSessionAdded: ((session: Session) => void) | null = null;
  public onAssistantDelta:
    | ((session: Session, delta: string, turnId: string) => void)
    | null = null;
  public onTurnCompleted:
    | ((session: Session, status: string, turnId: string) => void)
    | null = null;
  public onDiffUpdated:
    | ((session: Session, diff: string, turnId: string) => void)
    | null = null;
  public onTrace:
    | ((
        session: Session,
        entry: {
          kind: "system" | "tool" | "reasoning";
          text: string;
          itemId?: string;
          append?: boolean;
        },
      ) => void)
    | null = null;
  public onApprovalRequest:
    | ((session: Session, req: V2ApprovalRequest) => Promise<ApprovalDecision>)
    | null = null;
  public onServerEvent:
    | ((session: Session | null, n: AnyServerNotification) => void)
    | null = null;

  public constructor(
    private readonly output: vscode.OutputChannel,
    private readonly sessions: SessionStore,
  ) {}

  public getRunningCommand(folder: vscode.WorkspaceFolder): string | null {
    const key = folder.uri.toString();
    const proc = this.processes.get(key);
    return proc ? proc.getCommand() : null;
  }

  public stopForWorkspaceFolder(folder: vscode.WorkspaceFolder): void {
    const key = folder.uri.toString();
    const proc = this.processes.get(key);
    if (!proc) return;
    this.output.appendLine(`Stopping backend for ${folder.uri.fsPath}`);
    try {
      proc.dispose();
    } finally {
      this.processes.delete(key);
      this.modelsByBackendKey.delete(key);
      this.itemsByThreadId.clear();
      this.latestDiffByThreadId.clear();
      for (const s of this.sessions.listAll()) {
        if (s.backendKey !== key) continue;
        this.streamState.delete(s.threadId);
      }
    }
  }

  public forceStopForWorkspaceFolder(folder: vscode.WorkspaceFolder): void {
    const key = folder.uri.toString();
    const proc = this.processes.get(key);
    if (!proc) return;

    this.output.appendLine(`Force stopping backend for ${folder.uri.fsPath}`);
    proc.kill("SIGKILL");
    this.stopForWorkspaceFolder(folder);
  }

  public getActiveTurnId(threadId: string): string | null {
    return this.streamState.get(threadId)?.activeTurnId ?? null;
  }

  public async restartForWorkspaceFolder(
    folder: vscode.WorkspaceFolder,
  ): Promise<void> {
    this.stopForWorkspaceFolder(folder);
    await this.startForWorkspaceFolder(folder);
  }

  public async forceRestartForWorkspaceFolder(
    folder: vscode.WorkspaceFolder,
  ): Promise<void> {
    this.forceStopForWorkspaceFolder(folder);
    await this.startForWorkspaceFolder(folder);
  }

  public async startForWorkspaceFolder(
    folder: vscode.WorkspaceFolder,
  ): Promise<void> {
    const key = folder.uri.toString();
    const existing = this.processes.get(key);
    if (existing) {
      this.output.appendLine(
        `Backend already running for ${folder.uri.fsPath}`,
      );
      return;
    }

    const cfg = vscode.workspace.getConfiguration("codexMine", folder.uri);
    const cliVariantRaw = cfg.get<string>("cli.variant") ?? "auto";
    const cliVariant =
      cliVariantRaw === "upstream"
        ? "codex"
        : cliVariantRaw === "mine"
          ? "codex-mine"
          : cliVariantRaw;

    const codexCommand =
      cfg.get<string>("cli.commands.codex") ??
      cfg.get<string>("cli.commands.upstream") ??
      "codex";
    const codexMineCommand =
      cfg.get<string>("cli.commands.codexMine") ??
      cfg.get<string>("cli.commands.mine") ??
      "codex-mine";
    const commandFromBackend = cfg.get<string>("backend.command");
    const args = cfg.get<string[]>("backend.args");
    const logRpcPayloads = cfg.get<boolean>("debug.logRpcPayloads") ?? false;

    const command =
      cliVariant === "codex-mine"
        ? codexMineCommand
        : cliVariant === "codex"
          ? codexCommand
          : commandFromBackend;

    if (!command) {
      const keyName =
        cliVariant === "codex-mine"
          ? "codexMine.cli.commands.codexMine"
          : cliVariant === "codex"
            ? "codexMine.cli.commands.codex"
            : "codexMine.backend.command";
      throw new Error(`Missing configuration: ${keyName}`);
    }
    if (!args) throw new Error("Missing configuration: codexMine.backend.args");

    this.output.appendLine(
      `Starting backend: ${command} ${args.join(" ")} (cwd=${folder.uri.fsPath})`,
    );
    const proc = await BackendProcess.spawn({
      command,
      args,
      cwd: folder.uri.fsPath,
      logRpcPayloads,
      output: this.output,
    });

    this.processes.set(key, proc);
    proc.onDidExit(() => {
      this.processes.delete(key);
    });
    proc.onNotification = (n) => this.onServerNotification(key, n);
    proc.onApprovalRequest = async (req) => this.handleApprovalRequest(req);
  }

  public async newSession(
    folder: vscode.WorkspaceFolder,
    modelSettings?: ModelSettings,
  ): Promise<Session> {
    await this.startForWorkspaceFolder(folder);
    const backendKey = folder.uri.toString();
    const proc = this.processes.get(backendKey);
    if (!proc) throw new Error("Backend process was not started");

    const params: ThreadStartParams = {
      model: modelSettings?.model ?? null,
      modelProvider: modelSettings?.provider ?? null,
      cwd: folder.uri.fsPath,
      approvalPolicy: null,
      sandbox: null,
      config: modelSettings?.reasoning
        ? { reasoning_effort: modelSettings.reasoning }
        : null,
      baseInstructions: null,
      developerInstructions: null,
      experimentalRawEvents: false,
    };
    const res = await proc.threadStart(params);

    const session: Session = {
      id: randomUUID(),
      backendKey,
      workspaceFolderUri: folder.uri.toString(),
      title: folder.name,
      threadId: res.thread.id,
    };
    this.sessions.add(backendKey, session);
    this.output.appendLine(
      `[session] created: ${session.title} threadId=${session.threadId}`,
    );
    this.onSessionAdded?.(session);
    return session;
  }

  public getCachedModels(session: Session): Model[] | null {
    return this.modelsByBackendKey.get(session.backendKey) ?? null;
  }

  public async listSkillsForSession(
    session: Session,
  ): Promise<SkillsListEntry[]> {
    const folder = this.resolveWorkspaceFolder(session.workspaceFolderUri);
    if (!folder) {
      throw new Error(
        `WorkspaceFolder not found for session: ${session.workspaceFolderUri}`,
      );
    }

    await this.startForWorkspaceFolder(folder);
    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");

    const res = await proc.skillsList({
      cwds: [folder.uri.fsPath],
      forceReload: false,
    });
    return res.data ?? [];
  }

  public async fuzzyFileSearchForSession(
    session: Session,
    query: string,
    cancellationToken: string,
  ): Promise<FuzzyFileSearchResponse> {
    const folder = this.resolveWorkspaceFolder(session.workspaceFolderUri);
    if (!folder) {
      throw new Error(
        `WorkspaceFolder not found for session: ${session.workspaceFolderUri}`,
      );
    }

    await this.startForWorkspaceFolder(folder);
    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");

    return proc.fuzzyFileSearch({
      query,
      roots: [folder.uri.fsPath],
      cancellationToken,
    });
  }

  public async listModelsForSession(session: Session): Promise<Model[]> {
    const folder = this.resolveWorkspaceFolder(session.workspaceFolderUri);
    if (!folder) {
      throw new Error(
        `WorkspaceFolder not found for session: ${session.workspaceFolderUri}`,
      );
    }

    await this.startForWorkspaceFolder(folder);
    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");

    const cached = this.modelsByBackendKey.get(session.backendKey);
    if (cached) return cached;

    const models = await this.fetchAllModels(proc);
    this.modelsByBackendKey.set(session.backendKey, models);
    return models;
  }

  public async listThreadsForWorkspaceFolder(
    folder: vscode.WorkspaceFolder,
    opts?: {
      cursor?: string | null;
      limit?: number | null;
      modelProviders?: string[] | null;
    },
  ): Promise<{ data: Thread[]; nextCursor: string | null }> {
    await this.startForWorkspaceFolder(folder);
    const backendKey = folder.uri.toString();
    const proc = this.processes.get(backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");

    const res = await proc.threadList({
      cursor: opts?.cursor ?? null,
      limit: opts?.limit ?? null,
      modelProviders: opts?.modelProviders ?? null,
    });
    return { data: res.data ?? [], nextCursor: res.nextCursor ?? null };
  }

  private async fetchAllModels(proc: BackendProcess): Promise<Model[]> {
    const out: Model[] = [];
    let cursor: string | null = null;
    for (let i = 0; i < 10; i += 1) {
      const res: ModelListResponse = await proc.listModels({
        cursor,
        limit: 200,
      });
      out.push(...(res.data ?? []));
      cursor = res.nextCursor;
      if (!cursor) break;
    }
    return out;
  }

  public async pickSession(
    folder: vscode.WorkspaceFolder,
  ): Promise<Session | null> {
    const backendKey = folder.uri.toString();
    return this.sessions.pick(backendKey);
  }

  public async resumeSession(
    session: Session,
    modelSettings?: ModelSettings,
  ): Promise<ThreadResumeResponse> {
    const folder = this.resolveWorkspaceFolder(session.workspaceFolderUri);
    if (!folder) {
      throw new Error(
        `WorkspaceFolder not found for session: ${session.workspaceFolderUri}`,
      );
    }

    await this.startForWorkspaceFolder(folder);
    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");

    const params: ThreadResumeParams = {
      threadId: session.threadId,
      history: null,
      path: null,
      model: modelSettings?.model ?? null,
      modelProvider: modelSettings?.provider ?? null,
      cwd: folder.uri.fsPath,
      approvalPolicy: null,
      sandbox: null,
      config: modelSettings?.reasoning
        ? { reasoning_effort: modelSettings.reasoning }
        : null,
      baseInstructions: null,
      developerInstructions: null,
    };
    return await proc.threadResume(params);
  }

  public async archiveSession(session: Session): Promise<void> {
    const folder = this.resolveWorkspaceFolder(session.workspaceFolderUri);
    if (!folder) {
      throw new Error(
        `WorkspaceFolder not found for session: ${session.workspaceFolderUri}`,
      );
    }

    await this.startForWorkspaceFolder(folder);
    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");

    await proc.threadArchive({ threadId: session.threadId });
  }

  public async readAccount(session: Session): Promise<GetAccountResponse> {
    const folder = this.resolveWorkspaceFolder(session.workspaceFolderUri);
    if (!folder) {
      throw new Error(
        `WorkspaceFolder not found for session: ${session.workspaceFolderUri}`,
      );
    }
    await this.startForWorkspaceFolder(folder);
    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");
    return await proc.accountRead({ refreshToken: false });
  }

  public async readRateLimits(
    session: Session,
  ): Promise<GetAccountRateLimitsResponse> {
    const folder = this.resolveWorkspaceFolder(session.workspaceFolderUri);
    if (!folder) {
      throw new Error(
        `WorkspaceFolder not found for session: ${session.workspaceFolderUri}`,
      );
    }
    await this.startForWorkspaceFolder(folder);
    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");
    return await proc.accountRateLimitsRead();
  }

  public latestDiff(session: Session): string | null {
    return this.latestDiffByThreadId.get(session.threadId) ?? null;
  }

  public getItem(threadId: string, itemId: string): ThreadItem | null {
    return this.itemsByThreadId.get(threadId)?.get(itemId) ?? null;
  }

  public async sendMessage(session: Session, text: string): Promise<void> {
    await this.sendMessageWithModelAndImages(session, text, [], null);
  }

  public async sendMessageWithModel(
    session: Session,
    text: string,
    modelSettings: ModelSettings | undefined,
  ): Promise<void> {
    await this.sendMessageWithModelAndImages(session, text, [], modelSettings);
  }

  public async sendMessageWithModelAndImages(
    session: Session,
    text: string,
    images: string[],
    modelSettings: ModelSettings | null | undefined,
  ): Promise<void> {
    const folder = this.resolveWorkspaceFolder(session.workspaceFolderUri);
    if (!folder) {
      throw new Error(
        `WorkspaceFolder not found for session: ${session.workspaceFolderUri}`,
      );
    }

    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");

    const input: UserInput[] = [];
    if (text.trim()) input.push({ type: "text", text });
    for (const url of images) {
      if (typeof url !== "string" || url.trim() === "") continue;
      input.push({ type: "image", url });
    }
    if (input.length === 0) {
      throw new Error("Message must include text or images");
    }
    const effort = this.toReasoningEffort(modelSettings?.reasoning ?? null);
    const params: TurnStartParams = {
      threadId: session.threadId,
      input,
      cwd: null,
      approvalPolicy: null,
      sandboxPolicy: null,
      model: modelSettings?.model ?? null,
      effort,
      summary: null,
    };

    const imageSuffix = images.length > 0 ? ` [images=${images.length}]` : "";
    this.output.appendLine(`\n>> (${session.title}) ${text}${imageSuffix}`);
    this.output.append(`<< (${session.title}) `);
    const turn = await proc.turnStart(params);
    this.streamState.set(session.threadId, { activeTurnId: turn.turn.id });
  }

  public async interruptTurn(session: Session, turnId: string): Promise<void> {
    const folder = this.resolveWorkspaceFolder(session.workspaceFolderUri);
    if (!folder) {
      throw new Error(
        `WorkspaceFolder not found for session: ${session.workspaceFolderUri}`,
      );
    }

    await this.startForWorkspaceFolder(folder);
    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");

    await proc.turnInterrupt({ threadId: session.threadId, turnId });
  }

  private toReasoningEffort(effort: string | null): ReasoningEffort | null {
    if (!effort) return null;
    const e = effort.trim();
    if (!e) return null;
    const allowed: ReadonlySet<string> = new Set([
      "none",
      "minimal",
      "low",
      "medium",
      "high",
      "xhigh",
    ]);
    if (!allowed.has(e)) {
      this.output.appendLine(
        `[model] Invalid reasoning effort '${e}', ignoring (expected one of: ${[...allowed].join(", ")})`,
      );
      return null;
    }
    return e as ReasoningEffort;
  }

  private onServerNotification(
    _backendKey: string,
    n: AnyServerNotification,
  ): void {
    const session =
      "params" in n ? this.sessionFromParams((n as any).params) : null;
    this.onServerEvent?.(session, n);

    const p = (n as any).params as any;

    if (n.method === "item/agentMessage/delta") {
      const state = this.streamState.get(p.threadId);
      if (!state || state.activeTurnId !== p.turnId) return;
      this.output.append(p.delta);

      if (session) this.onAssistantDelta?.(session, p.delta, p.turnId);
      return;
    }

    if (n.method === "turn/completed") {
      const state = this.streamState.get(p.threadId);
      if (!state || state.activeTurnId !== p.turn.id) return;

      this.output.appendLine("");
      this.output.appendLine(
        `[turn] completed: status=${p.turn.status} turnId=${p.turn.id}`,
      );
      this.streamState.set(p.threadId, { activeTurnId: null });

      const session = this.sessions.getByThreadId(p.threadId);
      if (session) {
        this.onTurnCompleted?.(session, p.turn.status, p.turn.id);
      }
      return;
    }

    if (n.method === "turn/diff/updated") {
      const session = this.sessions.getByThreadId(p.threadId);
      if (!session) return;
      this.latestDiffByThreadId.set(p.threadId, p.diff);
      this.onDiffUpdated?.(session, p.diff, p.turnId);
      return;
    }

    if (n.method === "turn/plan/updated") {
      const session = this.sessions.getByThreadId(p.threadId);
      if (!session) return;
      const plan = p.plan as Array<{ status: string; step: string }>;
      const steps = plan
        .map((step) => `- [${step.status}] ${step.step}`)
        .join("\n");
      const text = p.explanation ? `${p.explanation}\n${steps}` : steps;
      this.onTrace?.(session, { kind: "system", text: `[plan]\n${text}` });
      return;
    }

    if (n.method === "item/started") {
      const session = this.sessions.getByThreadId(p.threadId);
      if (!session) return;
      const item = p.item;
      this.upsertItem(p.threadId, item);
      this.onTrace?.(session, {
        kind:
          item.type === "reasoning"
            ? "reasoning"
            : item.type === "agentMessage"
              ? "system"
              : "tool",
        itemId: item.id,
        text: summarizeItem("started", item),
      });
      return;
    }

    if (n.method === "item/completed") {
      const session = this.sessions.getByThreadId(p.threadId);
      if (!session) return;
      const item = p.item;
      this.upsertItem(p.threadId, item);
      this.onTrace?.(session, {
        kind:
          item.type === "reasoning"
            ? "reasoning"
            : item.type === "agentMessage"
              ? "system"
              : "tool",
        itemId: item.id,
        text: summarizeItem("completed", item),
      });
      return;
    }

    if (n.method === "item/commandExecution/outputDelta") {
      const session = this.sessions.getByThreadId(p.threadId);
      if (!session) return;
      this.onTrace?.(session, {
        kind: "tool",
        itemId: p.itemId,
        append: true,
        text: p.delta,
      });
      return;
    }

    if (n.method === "item/fileChange/outputDelta") {
      const session = this.sessions.getByThreadId(p.threadId);
      if (!session) return;
      this.onTrace?.(session, {
        kind: "tool",
        itemId: p.itemId,
        append: true,
        text: p.delta,
      });
      return;
    }

    if (n.method === "item/mcpToolCall/progress") {
      const session = this.sessions.getByThreadId(p.threadId);
      if (!session) return;
      this.onTrace?.(session, {
        kind: "tool",
        itemId: p.itemId,
        append: true,
        text: `${p.message}\n`,
      });
      return;
    }

    if (n.method === "item/reasoning/summaryTextDelta") {
      const session = this.sessions.getByThreadId(p.threadId);
      if (!session) return;
      this.onTrace?.(session, {
        kind: "reasoning",
        itemId: p.itemId,
        append: true,
        text: p.delta,
      });
      return;
    }

    if (n.method === "item/reasoning/textDelta") {
      const session = this.sessions.getByThreadId(p.threadId);
      if (!session) return;
      this.onTrace?.(session, {
        kind: "reasoning",
        itemId: p.itemId,
        append: true,
        text: p.delta,
      });
      return;
    }
  }

  private sessionFromParams(params: unknown): Session | null {
    if (typeof params !== "object" || params === null) return null;
    const o = params as Record<string, unknown>;
    const threadId =
      (typeof o["threadId"] === "string" ? (o["threadId"] as string) : null) ??
      (typeof o["conversationId"] === "string"
        ? (o["conversationId"] as string)
        : null) ??
      (typeof o["thread_id"] === "string" ? (o["thread_id"] as string) : null);

    if (!threadId && typeof o["msg"] === "object" && o["msg"] !== null) {
      const msg = o["msg"] as Record<string, unknown>;
      const msgThreadId =
        (typeof msg["thread_id"] === "string"
          ? (msg["thread_id"] as string)
          : null) ??
        (typeof msg["threadId"] === "string"
          ? (msg["threadId"] as string)
          : null);
      if (msgThreadId) return this.sessions.getByThreadId(msgThreadId);
    }

    if (typeof threadId !== "string") return null;
    return this.sessions.getByThreadId(threadId);
  }

  public dispose(): void {
    for (const proc of this.processes.values()) proc.dispose();
    this.processes.clear();
  }

  private async handleApprovalRequest(
    req: V2ApprovalRequest,
  ): Promise<ApprovalDecision> {
    const session = this.sessions.getByThreadId(req.params.threadId);
    if (!session) {
      throw new Error(
        `Session not found for approval request: threadId=${req.params.threadId}`,
      );
    }
    if (!this.onApprovalRequest) {
      throw new Error("onApprovalRequest handler is not set");
    }
    return this.onApprovalRequest(session, req);
  }

  private resolveWorkspaceFolder(
    workspaceFolderUri: string,
  ): vscode.WorkspaceFolder | null {
    const uri = vscode.Uri.parse(workspaceFolderUri);
    return vscode.workspace.getWorkspaceFolder(uri) ?? null;
  }

  private upsertItem(threadId: string, item: ThreadItem): void {
    const map =
      this.itemsByThreadId.get(threadId) ?? new Map<string, ThreadItem>();
    map.set(item.id, item);
    this.itemsByThreadId.set(threadId, map);
  }
}

type V2ApprovalRequest = Extract<
  ServerRequest,
  {
    method:
      | "item/commandExecution/requestApproval"
      | "item/fileChange/requestApproval";
  }
>;

function summarizeItem(
  phase: "started" | "completed",
  item: ThreadItem,
): string {
  const prefix = `[item ${phase}] ${item.type}`;
  switch (item.type) {
    case "commandExecution": {
      const status = phase === "completed" ? ` status=${item.status}` : "";
      const exitCode =
        item.exitCode !== null ? ` exitCode=${item.exitCode}` : "";
      return `${prefix}${status}${exitCode}\n$ ${item.command}\n`;
    }
    case "fileChange": {
      const files = item.changes.map((c) => c.path).join(", ");
      const status = phase === "completed" ? ` status=${item.status}` : "";
      return `${prefix}${status}\nfiles: ${files}\n`;
    }
    case "mcpToolCall": {
      const status = phase === "completed" ? ` status=${item.status}` : "";
      return `${prefix}${status}\n${item.server}.${item.tool}\n`;
    }
    case "webSearch": {
      return `${prefix}\nquery: ${item.query}\n`;
    }
    case "reasoning": {
      return `${prefix}\n`;
    }
    case "agentMessage": {
      return `${prefix}\n`;
    }
    case "imageView": {
      return `${prefix}\npath: ${item.path}\n`;
    }
    case "userMessage": {
      return `${prefix}\n`;
    }
    case "enteredReviewMode":
    case "exitedReviewMode": {
      return `${prefix}\n`;
    }
    default: {
      return `${prefix}\n`;
    }
  }
}
