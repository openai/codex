import * as vscode from "vscode";
import { randomUUID } from "node:crypto";
import { BackendProcess, type BackendExitInfo } from "./process";
import type { ThreadResumeParams } from "../generated/v2/ThreadResumeParams";
import type { ThreadStartParams } from "../generated/v2/ThreadStartParams";
import type { ThreadCompactParams } from "../generated/v2/ThreadCompactParams";
import type { TurnStartParams } from "../generated/v2/TurnStartParams";
import type { UserInput } from "../generated/v2/UserInput";
import type { ThreadItem } from "../generated/v2/ThreadItem";
import type { Session, SessionStore } from "../sessions";
import type { ApprovalDecision } from "../generated/v2/ApprovalDecision";
import type { AskUserQuestionResponse } from "../generated/v2/AskUserQuestionResponse";
import type { ServerRequest } from "../generated/ServerRequest";
import type { ThreadResumeResponse } from "../generated/v2/ThreadResumeResponse";
import type { ThreadRollbackResponse } from "../generated/v2/ThreadRollbackResponse";
import type { ModelListResponse } from "../generated/v2/ModelListResponse";
import type { Model } from "../generated/v2/Model";
import type { ReasoningEffort } from "../generated/ReasoningEffort";
import type { GetAccountResponse } from "../generated/v2/GetAccountResponse";
import type { GetAccountRateLimitsResponse } from "../generated/v2/GetAccountRateLimitsResponse";
import type { ListAccountsResponse } from "../generated/v2/ListAccountsResponse";
import type { SwitchAccountParams } from "../generated/v2/SwitchAccountParams";
import type { SwitchAccountResponse } from "../generated/v2/SwitchAccountResponse";
import type { SkillsListEntry } from "../generated/v2/SkillsListEntry";
import type { Thread } from "../generated/v2/Thread";
import type { AnyServerNotification } from "./types";
import type { FuzzyFileSearchResponse } from "../generated/FuzzyFileSearchResponse";
import type { ListMcpServerStatusResponse } from "../generated/v2/ListMcpServerStatusResponse";

type ModelSettings = {
  model: string | null;
  provider: string | null;
  reasoning: string | null;
};

export type BackendTermination = {
  reason: "exit" | "stop";
  code: number | null;
  signal: NodeJS.Signals | null;
};

export class BackendManager implements vscode.Disposable {
  private readonly processes = new Map<string, BackendProcess>();
  private readonly startInFlight = new Map<string, Promise<void>>();
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
  public onAskUserQuestionRequest:
    | ((
        session: Session,
        req: V2AskUserQuestionRequest,
      ) => Promise<AskUserQuestionResponse>)
    | null = null;
  public onServerEvent:
    | ((
        backendKey: string,
        session: Session | null,
        n: AnyServerNotification,
      ) => void)
    | null = null;
  public onBackendTerminated:
    | ((backendKey: string, info: BackendTermination) => void)
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
    this.terminateBackend(key, proc, {
      reason: "stop",
      code: null,
      signal: null,
    });
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

  public async startForWorkspaceFolder(
    folder: vscode.WorkspaceFolder,
  ): Promise<void> {
    const key = folder.uri.toString();
    const existing = this.processes.get(key);
    if (existing) {
      return;
    }

    const inflight = this.startInFlight.get(key);
    if (inflight) {
      await inflight;
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

    const startPromise = (async () => {
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
      proc.onDidExitWithInfo((info: BackendExitInfo) => {
        // Backend died unexpectedly (e.g. killed from outside VS Code).
        this.processes.delete(key);
        this.cleanupBackendCaches(key);
        this.onBackendTerminated?.(key, { reason: "exit", ...info });
      });
      proc.onNotification = (n) => this.onServerNotification(key, n);
      proc.onApprovalRequest = async (req) => this.handleApprovalRequest(req);
      proc.onAskUserQuestionRequest = async (req) =>
        this.handleAskUserQuestionRequest(req);
    })();

    this.startInFlight.set(
      key,
      startPromise.finally(() => this.startInFlight.delete(key)),
    );
    await startPromise;
  }

  public async listMcpServerStatus(
    backendKey: string,
    cwd: string,
  ): Promise<ListMcpServerStatusResponse> {
    const proc = this.processes.get(backendKey);
    if (!proc) {
      throw new Error(
        `Backend process not running for backendKey=${backendKey} (cannot list MCP servers)`,
      );
    }
    return await this.withTimeout(
      "mcpServerStatus/list",
      proc.mcpServerStatusList({ cursor: null, limit: null, cwd }),
      30_000,
    );
  }

  private async withTimeout<T>(
    label: string,
    promise: Promise<T>,
    timeoutMs: number,
  ): Promise<T> {
    let timeoutHandle: NodeJS.Timeout | null = null;
    const timeoutPromise = new Promise<T>((_, reject) => {
      timeoutHandle = setTimeout(() => {
        reject(new Error(`${label} timed out after ${timeoutMs}ms`));
      }, timeoutMs);
    });

    try {
      return await Promise.race([promise, timeoutPromise]);
    } finally {
      if (timeoutHandle) clearTimeout(timeoutHandle);
    }
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

  public async resumeSession(session: Session): Promise<ThreadResumeResponse> {
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
      // Resume should not override session settings. Overriding here would prevent the backend from
      // using a "fast path" for loaded conversations and can break streaming if the UI calls
      // thread/resume while a turn is in progress.
      model: null,
      modelProvider: null,
      cwd: null,
      approvalPolicy: null,
      sandbox: null,
      config: null,
      baseInstructions: null,
      developerInstructions: null,
    };
    return await proc.threadResume(params);
  }

  public async reloadSession(
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

    // Clear per-thread caches so the UI can rehydrate from the refreshed thread state.
    this.itemsByThreadId.delete(session.threadId);
    this.latestDiffByThreadId.delete(session.threadId);
    this.streamState.set(session.threadId, { activeTurnId: null });

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

  public async listAccounts(session: Session): Promise<ListAccountsResponse> {
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
    return await proc.accountList();
  }

  public async switchAccount(
    session: Session,
    params: SwitchAccountParams,
  ): Promise<SwitchAccountResponse> {
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
    return await proc.accountSwitch(params);
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
    images: Array<
      { kind: "imageUrl"; url: string } | { kind: "localImage"; path: string }
    >,
    modelSettings: ModelSettings | null | undefined,
  ): Promise<void> {
    const folder = this.resolveWorkspaceFolder(session.workspaceFolderUri);
    if (!folder) {
      throw new Error(
        `WorkspaceFolder not found for session: ${session.workspaceFolderUri}`,
      );
    }

    // Backend can terminate unexpectedly; ensure it is started before sending.
    await this.startForWorkspaceFolder(folder);

    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");

    const input: UserInput[] = [];
    if (text.trim()) input.push({ type: "text", text, text_elements: [] });
    for (const img of images) {
      if (!img) continue;
      if (img.kind === "imageUrl") {
        const url = img.url;
        if (typeof url !== "string" || url.trim() === "") continue;
        input.push({ type: "image", url });
        continue;
      }
      if (img.kind === "localImage") {
        const p = img.path;
        if (typeof p !== "string" || p.trim() === "") continue;
        input.push({ type: "localImage", path: p });
        continue;
      }
      const neverImg: never = img;
      throw new Error(`Unexpected image input: ${String(neverImg)}`);
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
      outputSchema: null,
    };

    const imageSuffix = images.length > 0 ? ` [images=${images.length}]` : "";
    this.output.appendLine(`\n>> (${session.title}) ${text}${imageSuffix}`);
    this.output.append(`<< (${session.title}) `);
    const turn = await this.withTimeout(
      "turn/start",
      proc.turnStart(params),
      10_000,
    );
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

  public async threadRollback(
    session: Session,
    args: { numTurns: number },
  ): Promise<ThreadRollbackResponse> {
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

    // Clear per-thread caches so the UI can rehydrate from the updated thread state.
    this.itemsByThreadId.delete(session.threadId);
    this.latestDiffByThreadId.delete(session.threadId);
    this.streamState.set(session.threadId, { activeTurnId: null });

    return await proc.threadRollback({
      threadId: session.threadId,
      numTurns: args.numTurns,
    });
  }

  public async threadCompact(session: Session): Promise<void> {
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

    const params: ThreadCompactParams = { threadId: session.threadId };
    this.output.appendLine(`\n>> (${session.title}) /compact`);
    this.output.append(`<< (${session.title}) `);
    await this.withTimeout(
      "thread/compact",
      proc.threadCompact(params),
      10_000,
    );
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

  private terminateBackend(
    backendKey: string,
    proc: BackendProcess,
    info: BackendTermination,
  ): void {
    // Clear cached state first so any UI reading from BackendManager doesn't see stale turns.
    this.processes.delete(backendKey);
    this.cleanupBackendCaches(backendKey);

    // Notify after internal cleanup so listeners can read an up-to-date state.
    this.onBackendTerminated?.(backendKey, info);

    // Finally, dispose the process (this intentionally removes child listeners,
    // so don't rely on proc.onDidExit for explicit stops).
    proc.dispose();
  }

  private cleanupBackendCaches(backendKey: string): void {
    this.modelsByBackendKey.delete(backendKey);
    const sessions = this.sessions.list(backendKey);
    for (const s of sessions) {
      this.itemsByThreadId.delete(s.threadId);
      this.latestDiffByThreadId.delete(s.threadId);
      this.streamState.delete(s.threadId);
    }
  }

  private onServerNotification(
    backendKey: string,
    n: AnyServerNotification,
  ): void {
    const session =
      "params" in n ? this.sessionFromParams((n as any).params) : null;
    this.onServerEvent?.(backendKey, session, n);

    const p = (n as any).params as any;

    if (n.method === "item/agentMessage/delta") {
      const state = this.streamState.get(p.threadId);
      if (!state || state.activeTurnId !== p.turnId) return;
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

  private async handleAskUserQuestionRequest(
    req: V2AskUserQuestionRequest,
  ): Promise<AskUserQuestionResponse> {
    const session = this.sessions.getByThreadId(req.params.threadId);
    if (!session) {
      throw new Error(
        `Session not found for ask-question request: threadId=${req.params.threadId}`,
      );
    }
    if (!this.onAskUserQuestionRequest) {
      throw new Error("onAskUserQuestionRequest handler is not set");
    }
    return this.onAskUserQuestionRequest(session, req);
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

type V2AskUserQuestionRequest = Extract<
  ServerRequest,
  {
    method: "user/askQuestion";
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
