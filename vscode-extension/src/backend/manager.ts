import * as vscode from "vscode";
import { randomUUID } from "node:crypto";
import { pathToFileURL } from "node:url";
import { BackendProcess, type BackendExitInfo } from "./process";
import { OpencodeServerProcess } from "./opencode_process";
import {
  OpencodeHttpClient,
  type OpencodeEvent,
  type OpencodeFileDiff,
  type OpencodeMessageWithParts,
  type OpencodeSessionInfo,
  type OpencodeProviderAuthMethodsResponse,
  type OpencodeProviderAuthorization,
  type OpencodeProviderListResponse,
  type OpencodeAgentInfo,
  type OpencodeQuestionRequest,
} from "./opencode_http";
import type { ThreadResumeParams } from "../generated/v2/ThreadResumeParams";
import type { ThreadStartParams } from "../generated/v2/ThreadStartParams";
import type { ThreadCompactParams } from "../generated/v2/ThreadCompactParams";
import type { TurnStartParams } from "../generated/v2/TurnStartParams";
import type { UserInput } from "../generated/v2/UserInput";
import type { ThreadItem } from "../generated/v2/ThreadItem";
import type { BackendId, Session, SessionStore } from "../sessions";
import {
  makeBackendInstanceKey,
  parseBackendInstanceKey,
} from "./backend_instance_key";
import type { CommandExecutionApprovalDecision } from "../generated/v2/CommandExecutionApprovalDecision";
import type { FileChangeApprovalDecision } from "../generated/v2/FileChangeApprovalDecision";
import type { ServerRequest } from "../generated/ServerRequest";
import type { ThreadResumeResponse } from "../generated/v2/ThreadResumeResponse";
import type { ThreadRollbackResponse } from "../generated/v2/ThreadRollbackResponse";
import type { ModelListResponse } from "../generated/v2/ModelListResponse";
import type { Model } from "../generated/v2/Model";
import type { ReasoningEffort } from "../generated/ReasoningEffort";
import type { Personality } from "../generated/Personality";
import type { CollaborationMode } from "../generated/CollaborationMode";
import type { GetAccountResponse } from "../generated/v2/GetAccountResponse";
import type { GetAccountRateLimitsResponse } from "../generated/v2/GetAccountRateLimitsResponse";
import type { LoginAccountParams } from "../generated/v2/LoginAccountParams";
import type { LoginAccountResponse } from "../generated/v2/LoginAccountResponse";
import type { ListAccountsResponse } from "../generated/v2/ListAccountsResponse";
import type { LogoutAccountResponse } from "../generated/v2/LogoutAccountResponse";
import type { SwitchAccountParams } from "../generated/v2/SwitchAccountParams";
import type { SwitchAccountResponse } from "../generated/v2/SwitchAccountResponse";
import type { SkillsListEntry } from "../generated/v2/SkillsListEntry";
import type { Thread } from "../generated/v2/Thread";
import type { ThreadSourceKind } from "../generated/v2/ThreadSourceKind";
import type { Turn } from "../generated/v2/Turn";
import type { AppInfo } from "../generated/v2/AppInfo";
import type { AppsListResponse } from "../generated/v2/AppsListResponse";
import type { CollaborationModeMask } from "../generated/CollaborationModeMask";
import type { AnyServerNotification } from "./types";
import type { FuzzyFileSearchResponse } from "../generated/FuzzyFileSearchResponse";
import type { ListMcpServerStatusResponse } from "../generated/v2/ListMcpServerStatusResponse";
import type { AskForApproval } from "../generated/v2/AskForApproval";
import type { SandboxPolicy } from "../generated/v2/SandboxPolicy";
import { promptRequestUserInput } from "./request_user_input";

type ModelSettings = {
  model: string | null;
  provider: string | null;
  reasoning: string | null;
  agent?: string | null;
  personality?: Personality | null;
  collaborationMode?: CollaborationMode | null;
};

function imageMimeFromPath(filePath: string): string | null {
  const ext = filePath.trim().toLowerCase().split(".").pop() ?? "";
  switch (ext) {
    case "png":
      return "image/png";
    case "jpg":
    case "jpeg":
      return "image/jpeg";
    case "gif":
      return "image/gif";
    case "webp":
      return "image/webp";
    case "bmp":
      return "image/bmp";
    case "svg":
      return "image/svg+xml";
    case "tif":
    case "tiff":
      return "image/tiff";
    default:
      return null;
  }
}

export type BackendTermination = {
  reason: "exit" | "stop";
  code: number | null;
  signal: NodeJS.Signals | null;
};

export class BackendManager implements vscode.Disposable {
  private readonly processes = new Map<string, BackendProcess>();
  private readonly opencode = new Map<
    string,
    {
      client: OpencodeHttpClient;
      sse: AbortController;
      messageRoleById: Map<string, "user" | "assistant">;
      activeTurnIdBySession: Map<string, string>;
      pendingAssistantTextDeltasByKey: Map<string, string[]>;
      toolPartByToolId: Map<string, unknown>;
      toolIdsNeedingStepByMessageId: Map<string, Set<string>>;
      partItemIdByKey: Map<string, string>;
      activeStepIdByMessageId: Map<string, string>;
      sessionStatusById: Map<string, string>;
    }
  >();
  private opencodeServer: {
    proc: OpencodeServerProcess;
    command: string;
    args: string[];
  } | null = null;
  private opencodeServerInFlight: Promise<{
    proc: OpencodeServerProcess;
    command: string;
    args: string[];
  }> | null = null;
  private readonly startInFlight = new Map<string, Promise<void>>();
  private readonly streamState = new Map<
    string,
    { activeTurnId: string | null }
  >();
  private readonly latestDiffByThreadId = new Map<string, string>();
  private readonly modelsByBackendKey = new Map<string, Model[]>();
  private readonly opencodeDefaultModelKeyByBackendKey = new Map<
    string,
    string
  >();
  private readonly opencodeDefaultAgentNameByBackendKey = new Map<
    string,
    string
  >();
  private readonly itemsByThreadId = new Map<string, Map<string, ThreadItem>>();
  private readonly opencodeReasoningItemIdByMessageIdByBackendKey = new Map<
    string,
    Map<string, string>
  >();
  private readonly opencodeReasoningTextByItemIdByBackendKey = new Map<
    string,
    Map<string, string>
  >();
  private readonly opencodeReasoningItemIdsBySessionIdByBackendKey = new Map<
    string,
    Map<string, Set<string>>
  >();
  private readonly opencodeMessageSeqByMessageIdByBackendKey = new Map<
    string,
    Map<string, number>
  >();
  private readonly opencodeNextMessageSeqByBackendKey = new Map<
    string,
    number
  >();
  private readonly opencodePendingAssistantMessageIdsBySessionIdByBackendKey =
    new Map<string, Map<string, Set<string>>>();

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
    | ((
        session: Session,
        req: V2ApprovalRequest,
      ) => Promise<V2ApprovalDecision>)
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

  public getRunningCommandForBackendId(
    folder: vscode.WorkspaceFolder,
    backendId: BackendId,
  ): string | null {
    const backendKey = makeBackendInstanceKey(folder.uri.toString(), backendId);
    const proc = this.processes.get(backendKey);
    if (proc) return proc.getCommand();
    const oc = this.opencode.get(backendKey);
    if (oc) return "opencode";
    return null;
  }

  public stopForWorkspaceFolder(folder: vscode.WorkspaceFolder): void {
    const folderUri = folder.uri.toString();
    const toStop: Array<{ backendKey: string; backendId: BackendId }> = [];
    for (const backendKey of this.processes.keys()) {
      const parsed = parseBackendInstanceKey(backendKey);
      if (parsed.workspaceFolderUri === folderUri) {
        toStop.push({ backendKey, backendId: parsed.backendId });
      }
    }
    for (const backendKey of this.opencode.keys()) {
      const parsed = parseBackendInstanceKey(backendKey);
      if (parsed.workspaceFolderUri === folderUri) {
        toStop.push({ backendKey, backendId: parsed.backendId });
      }
    }
    for (const { backendKey, backendId } of toStop) {
      this.stopForBackendInstance(folder, backendId, backendKey);
    }
  }

  public stopForBackendInstance(
    folder: vscode.WorkspaceFolder,
    backendId: BackendId,
    backendKey = makeBackendInstanceKey(folder.uri.toString(), backendId),
  ): void {
    const proc = this.processes.get(backendKey);
    const oc = this.opencode.get(backendKey);
    if (proc) {
      this.output.appendLine(
        `Stopping backend (${backendId}) for ${folder.uri.fsPath}`,
      );
      this.terminateBackend(backendKey, proc, {
        reason: "stop",
        code: null,
        signal: null,
      });
      return;
    }
    if (oc) {
      this.output.appendLine(
        `Stopping opencode backend (${backendId}) for ${folder.uri.fsPath}`,
      );
      this.disposeOpencodeBackend(backendKey, {
        reason: "stop",
        code: null,
        signal: null,
      });
    }
  }

  public getActiveTurnId(threadId: string): string | null {
    return this.streamState.get(threadId)?.activeTurnId ?? null;
  }

  public async restartForWorkspaceFolder(
    folder: vscode.WorkspaceFolder,
  ): Promise<void> {
    const folderUri = folder.uri.toString();
    const backendIds: BackendId[] = [];
    for (const backendKey of this.processes.keys()) {
      const parsed = parseBackendInstanceKey(backendKey);
      if (parsed.workspaceFolderUri === folderUri)
        backendIds.push(parsed.backendId);
    }
    for (const backendKey of this.opencode.keys()) {
      const parsed = parseBackendInstanceKey(backendKey);
      if (parsed.workspaceFolderUri === folderUri)
        backendIds.push(parsed.backendId);
    }
    this.stopForWorkspaceFolder(folder);
    for (const backendId of backendIds) {
      await this.startForBackendId(folder, backendId);
    }
  }

  public async startForBackendId(
    folder: vscode.WorkspaceFolder,
    backendId: BackendId,
  ): Promise<void> {
    const backendKey = makeBackendInstanceKey(folder.uri.toString(), backendId);
    const existing = this.processes.get(backendKey);
    const existingOpencode = this.opencode.get(backendKey);
    if (existing || existingOpencode) return;

    const inflight = this.startInFlight.get(backendKey);
    if (inflight) {
      await inflight;
      return;
    }

    const cfg = vscode.workspace.getConfiguration("codez", folder.uri);
    if (backendId === "opencode") {
      const command = cfg.get<string>("opencode.command") ?? "opencode";
      const rawArgs = cfg.get<string[]>("opencode.args") ?? ["serve"];
      const args = [...rawArgs];
      if (!args.includes("--hostname")) args.push("--hostname", "127.0.0.1");
      if (!args.includes("--port")) args.push("--port", "0");

      const startPromise = (async () => {
        this.output.appendLine(
          `Starting opencode backend: ${command} ${args.join(" ")} (cwd=${folder.uri.fsPath})`,
        );
        const server = await this.ensureOpencodeServer({
          folder,
          command,
          args,
        });
        const baseUrl = server.proc.getBaseUrl();
        const client = new OpencodeHttpClient({
          baseUrl,
          directory: folder.uri.fsPath,
        });

        const messageRoleById = new Map<string, "user" | "assistant">();
        const activeTurnIdBySession = new Map<string, string>();
        const pendingAssistantTextDeltasByKey = new Map<string, string[]>();
        const toolPartByToolId = new Map<string, unknown>();
        const toolIdsNeedingStepByMessageId = new Map<string, Set<string>>();
        const partItemIdByKey = new Map<string, string>();
        const activeStepIdByMessageId = new Map<string, string>();
        const sessionStatusById = new Map<string, string>();

        const sse = await client.connectEventStream(
          (evt) => {
            this.onOpencodeEvent(
              backendKey,
              evt,
              messageRoleById,
              activeTurnIdBySession,
              pendingAssistantTextDeltasByKey,
              toolPartByToolId,
              toolIdsNeedingStepByMessageId,
              partItemIdByKey,
              activeStepIdByMessageId,
              sessionStatusById,
              client,
            );
          },
          (err) => {
            this.output.appendLine(
              `[opencode] event stream error: ${String(err)}`,
            );
          },
        );

        this.opencode.set(backendKey, {
          client,
          sse,
          messageRoleById,
          activeTurnIdBySession,
          pendingAssistantTextDeltasByKey,
          toolPartByToolId,
          toolIdsNeedingStepByMessageId,
          partItemIdByKey,
          activeStepIdByMessageId,
          sessionStatusById,
        });

        const cwd = folder.uri.fsPath;
        const lines: string[] = [];
        lines.push(`Server: ${baseUrl.toString()}`);
        lines.push(`Working directory: \`${cwd}\``);

        const results = await Promise.allSettled([
          this.withTimeout(
            "opencode /global/health",
            client.getHealth(),
            5_000,
          ),
          this.withTimeout("opencode /config", client.getConfig(), 5_000),
          this.withTimeout("opencode /provider", client.listProviders(), 5_000),
          this.withTimeout("opencode /skill", client.listSkills(cwd), 5_000),
        ]);

        const [healthRes, configSettled, providerRes, skillsRes] = results;

        if (healthRes.status === "fulfilled") {
          lines.push(`Version: \`${healthRes.value.version}\``);
        } else {
          lines.push(
            `Health: error: ${String(healthRes.reason instanceof Error ? healthRes.reason.message : healthRes.reason)}`,
          );
        }

        const cfgObj =
          configSettled.status === "fulfilled" ? configSettled.value : null;
        const providerObj =
          providerRes.status === "fulfilled" ? providerRes.value : null;

        if (providerObj) {
          const connected = Array.isArray(providerObj.connected)
            ? providerObj.connected.map((p) => String(p ?? "")).filter(Boolean)
            : [];
          const all = Array.isArray(providerObj.all) ? providerObj.all : [];
          const defaultByProvider =
            typeof providerObj.default === "object" &&
            providerObj.default !== null
              ? providerObj.default
              : {};

          if (connected.length === 0) {
            lines.push("Providers: (connected: none)");
          } else {
            lines.push("Providers:");
            const byId = new Map(
              all.map((p) => [String((p as any)?.id ?? ""), p]),
            );
            for (const providerId of connected) {
              const p = byId.get(providerId) as any;
              const name =
                typeof p?.name === "string" && p.name.trim()
                  ? String(p.name).trim()
                  : providerId;
              const modelCount = Array.isArray(p?.models)
                ? p.models.length
                : typeof p?.models === "object" && p.models !== null
                  ? Object.keys(p.models).length
                  : 0;
              const defaultModel =
                typeof (defaultByProvider as any)[providerId] === "string"
                  ? String((defaultByProvider as any)[providerId]).trim()
                  : "";
              lines.push(
                `- ✓ ${name} (${providerId}) models=${String(modelCount)}${defaultModel ? ` default=${defaultModel}` : ""}`,
              );
            }
          }
        } else if (providerRes.status === "rejected") {
          lines.push(
            `Providers: error: ${String(providerRes.reason instanceof Error ? providerRes.reason.message : providerRes.reason)}`,
          );
        }

        if (configSettled.status === "rejected") {
          lines.push(
            `Config: error: ${String(configSettled.reason instanceof Error ? configSettled.reason.message : configSettled.reason)}`,
          );
        }

        if (skillsRes.status === "rejected") {
          lines.push(
            `Skills: error: ${String(skillsRes.reason instanceof Error ? skillsRes.reason.message : skillsRes.reason)}`,
          );
        }

        this.onServerEvent?.(backendKey, null, {
          method: "opencode/started",
          params: {
            cwd,
            text: lines.join("\n"),
          },
        });
      })();

      this.startInFlight.set(
        backendKey,
        startPromise.finally(() => this.startInFlight.delete(backendKey)),
      );
      await startPromise;
      return;
    }

    const codexCommand =
      cfg.get<string>("cli.commands.codex") ??
      cfg.get<string>("cli.commands.upstream") ??
      "codex";
    const codezCommand =
      cfg.get<string>("cli.commands.codez") ??
      cfg.get<string>("cli.commands.mine") ??
      "codez";
    const args = cfg.get<string[]>("backend.args");
    const logRpcPayloads = cfg.get<boolean>("debug.logRpcPayloads") ?? false;

    if (!args) throw new Error("Missing configuration: codez.backend.args");

    const command = backendId === "codez" ? codezCommand : codexCommand;

    const startPromise = (async () => {
      this.output.appendLine(
        `Starting backend (${backendId}): ${command} ${args.join(" ")} (cwd=${folder.uri.fsPath})`,
      );
      const proc = await BackendProcess.spawn({
        command,
        args,
        cwd: folder.uri.fsPath,
        logRpcPayloads,
        output: this.output,
      });

      this.processes.set(backendKey, proc);
      proc.onDidExitWithInfo((info: BackendExitInfo) => {
        // Backend died unexpectedly (e.g. killed from outside VS Code).
        this.processes.delete(backendKey);
        this.cleanupBackendCaches(backendKey);
        this.onBackendTerminated?.(backendKey, { reason: "exit", ...info });
      });
      proc.onNotification = (n) => this.onServerNotification(backendKey, n);
      proc.onApprovalRequest = async (req) =>
        this.handleApprovalRequest(backendKey, req);
    })();

    this.startInFlight.set(
      backendKey,
      startPromise.finally(() => this.startInFlight.delete(backendKey)),
    );
    await startPromise;
  }

  public async listMcpServerStatus(
    backendKey: string,
  ): Promise<ListMcpServerStatusResponse> {
    const proc = this.processes.get(backendKey);
    if (!proc) {
      throw new Error(
        `Backend process not running for backendKey=${backendKey} (cannot list MCP servers)`,
      );
    }
    return await this.withTimeout(
      "mcpServerStatus/list",
      proc.mcpServerStatusList({ cursor: null, limit: null }),
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
    backendId: BackendId,
    modelSettings?: ModelSettings,
  ): Promise<Session> {
    const backendKey = makeBackendInstanceKey(folder.uri.toString(), backendId);
    await this.startForBackendId(folder, backendId);
    const oc = this.opencode.get(backendKey);
    if (oc) {
      const created = await oc.client.createSession();
      const session: Session = {
        id: randomUUID(),
        backendKey,
        backendId,
        workspaceFolderUri: folder.uri.toString(),
        title: folder.name,
        threadId: created.id,
      };
      this.sessions.add(backendKey, session);
      this.output.appendLine(
        `[session] created (opencode): ${session.title} sessionId=${session.threadId}`,
      );
      this.onSessionAdded?.(session);
      return session;
    }

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
      personality: modelSettings?.personality ?? null,
      ephemeral: null,
      dynamicTools: null,
      experimentalRawEvents: false,
    };
    const res = await proc.threadStart(params);

    const session: Session = {
      id: randomUUID(),
      backendKey,
      backendId,
      workspaceFolderUri: folder.uri.toString(),
      title: folder.name,
      threadId: res.thread.id,
      personality: modelSettings?.personality ?? null,
      collaborationModePresetName: null,
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

  public getOpencodeDefaultModelKey(session: Session): string | null {
    if (session.backendId !== "opencode") return null;
    return (
      this.opencodeDefaultModelKeyByBackendKey.get(session.backendKey) ?? null
    );
  }

  public getOpencodeDefaultAgentName(session: Session): string | null {
    if (session.backendId !== "opencode") return null;
    return (
      this.opencodeDefaultAgentNameByBackendKey.get(session.backendKey) ?? null
    );
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

    await this.startForBackendId(folder, session.backendId);
    const oc = this.opencode.get(session.backendKey);
    if (oc) {
      return await oc.client.listSkills(folder.uri.fsPath);
    }
    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");

    const res = await proc.skillsList({
      cwds: [folder.uri.fsPath],
      forceReload: false,
    });
    return res.data ?? [];
  }

  public async listAgentsForSession(
    session: Session,
  ): Promise<OpencodeAgentInfo[]> {
    if (session.backendId !== "opencode") {
      return [];
    }
    const oc = this.opencode.get(session.backendKey);
    if (!oc) {
      return [];
    }
    try {
      return await oc.client.listAgents();
    } catch (err) {
      this.output.appendLine(
        `[opencode] Failed to list agents: ${String((err as Error)?.message ?? err)}`,
      );
      return [];
    }
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

    await this.startForBackendId(folder, session.backendId);
    const oc = this.opencode.get(session.backendKey);
    if (oc) {
      return await oc.client.fuzzyFileSearch({
        query,
        roots: [folder.uri.fsPath],
        cancellationToken,
      });
    }
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

    await this.startForBackendId(folder, session.backendId);
    const oc = this.opencode.get(session.backendKey);
    if (oc) {
      let providers: OpencodeProviderListResponse;
      try {
        providers = await oc.client.listProviders();
      } catch (err) {
        throw new Error(
          `opencode /provider failed: ${String((err as Error)?.message ?? err)}`,
        );
      }

      let cfg: Record<string, unknown> | null = null;
      try {
        cfg = await oc.client.getConfig();
      } catch (err) {
        this.output.appendLine(
          `[opencode] Failed to read /config for default model: ${String((err as Error)?.message ?? err)}`,
        );
      }

      const providerAllowlist = parseOpencodeProviderAllowlist(cfg);
      const providerBlocklist = parseOpencodeDisabledProviders(cfg);
      const defaultAgentName = resolveOpencodeDefaultAgentName(cfg);
      if (defaultAgentName) {
        this.opencodeDefaultAgentNameByBackendKey.set(
          session.backendKey,
          defaultAgentName,
        );
      } else {
        this.opencodeDefaultAgentNameByBackendKey.delete(session.backendKey);
      }
      const connected = Array.isArray(providers?.connected)
        ? new Set(
            providers.connected.map((p) => String(p ?? "")).filter(Boolean),
          )
        : null;
      const filteredProviders = {
        ...providers,
        all: (Array.isArray(providers?.all) ? providers.all : []).filter(
          (p) => {
            const id = String((p as any)?.id ?? "").trim();
            if (!id) return false;
            if (providerBlocklist && providerBlocklist.has(id)) return false;
            if (providerAllowlist && !providerAllowlist.has(id)) return false;
            // If the server reports connected providers, only show connected providers.
            if (connected && connected.size > 0 && !connected.has(id))
              return false;
            return true;
          },
        ),
      } satisfies OpencodeProviderListResponse;

      const models = oc.client.modelsFromProviders(filteredProviders);
      this.modelsByBackendKey.set(session.backendKey, models);
      const defaultKey = resolveOpencodeDefaultModelKey(cfg, providers);
      if (defaultKey) {
        this.opencodeDefaultModelKeyByBackendKey.set(
          session.backendKey,
          defaultKey,
        );
      } else {
        this.opencodeDefaultModelKeyByBackendKey.delete(session.backendKey);
      }
      return models;
    }
    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");

    const cached = this.modelsByBackendKey.get(session.backendKey);
    if (cached) return cached;

    const models = await this.fetchAllModels(proc);
    this.modelsByBackendKey.set(session.backendKey, models);
    return models;
  }

  public async listThreadsForWorkspaceFolderAndBackendId(
    folder: vscode.WorkspaceFolder,
    backendId: BackendId,
    opts?: {
      cursor?: string | null;
      limit?: number | null;
      modelProviders?: string[] | null;
      sortKey?: "created_at" | "updated_at" | null;
      sourceKinds?: ThreadSourceKind[] | null;
      archived?: boolean | null;
    },
  ): Promise<{ data: Thread[]; nextCursor: string | null }> {
    const backendKey = makeBackendInstanceKey(folder.uri.toString(), backendId);
    await this.startForBackendId(folder, backendId);
    const oc = this.opencode.get(backendKey);
    if (oc) {
      const sessions = await oc.client.listSessions();
      const threads = sessions
        .filter(
          (s) =>
            typeof s?.directory === "string" &&
            s.directory === folder.uri.fsPath,
        )
        .map((s) => this.threadFromOpencodeSession(s));
      // NOTE: no pagination cursor support for now; return everything.
      return { data: threads, nextCursor: null };
    }
    const proc = this.processes.get(backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");

    const res = await proc.threadList({
      cursor: opts?.cursor ?? null,
      limit: opts?.limit ?? null,
      sortKey: opts?.sortKey ?? null,
      modelProviders: opts?.modelProviders ?? null,
      sourceKinds: opts?.sourceKinds ?? null,
      archived: opts?.archived ?? null,
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

  private async fetchAllApps(proc: BackendProcess): Promise<AppInfo[]> {
    const out: AppInfo[] = [];
    let cursor: string | null = null;
    for (let i = 0; i < 10; i += 1) {
      const res: AppsListResponse = await proc.appsList({
        cursor,
        limit: 200,
      });
      out.push(...(res.data ?? []));
      cursor = res.nextCursor ?? null;
      if (!cursor) break;
    }
    return out;
  }

  public async listAppsForSession(session: Session): Promise<AppInfo[]> {
    if (session.backendId === "opencode") {
      return [];
    }

    const folder = this.resolveWorkspaceFolder(session.workspaceFolderUri);
    if (!folder) {
      throw new Error(
        `WorkspaceFolder not found for session: ${session.workspaceFolderUri}`,
      );
    }

    await this.startForBackendId(folder, session.backendId);
    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");

    return await this.fetchAllApps(proc);
  }

  public async listCollaborationModePresetsForSession(
    session: Session,
  ): Promise<CollaborationModeMask[]> {
    if (session.backendId === "opencode") {
      return [];
    }

    const folder = this.resolveWorkspaceFolder(session.workspaceFolderUri);
    if (!folder) {
      throw new Error(
        `WorkspaceFolder not found for session: ${session.workspaceFolderUri}`,
      );
    }

    await this.startForBackendId(folder, session.backendId);
    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");

    const res = await proc.collaborationModeList({});
    return res.data ?? [];
  }

  public async pickSession(
    folder: vscode.WorkspaceFolder,
  ): Promise<Session | null> {
    const workspaceFolderUri = folder.uri.toString();
    const sessions = this.sessions.listByWorkspaceFolderUri(workspaceFolderUri);
    if (sessions.length === 0) return null;
    const picked = await vscode.window.showQuickPick(
      sessions.map((s) => ({
        label: s.title,
        description: `${s.backendId}  ${s.threadId}`,
        session: s,
      })),
      { title: "Codex UI: Select a session" },
    );
    return picked?.session ?? null;
  }

  public async resumeSession(session: Session): Promise<ThreadResumeResponse> {
    const folder = this.resolveWorkspaceFolder(session.workspaceFolderUri);
    if (!folder) {
      throw new Error(
        `WorkspaceFolder not found for session: ${session.workspaceFolderUri}`,
      );
    }

    await this.startForBackendId(folder, session.backendId);
    const oc = this.opencode.get(session.backendKey);
    if (oc) {
      const thread = await this.buildThreadFromOpencodeSession(
        session.threadId,
        oc.client,
      );
      return {
        thread,
        model: "",
        modelProvider: "",
        cwd: folder.uri.fsPath,
        approvalPolicy: "on-request" as AskForApproval,
        sandbox: { type: "dangerFullAccess" } as SandboxPolicy,
        reasoningEffort: null,
      };
    }
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
      personality: null,
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

    await this.startForBackendId(folder, session.backendId);
    const oc = this.opencode.get(session.backendKey);
    if (oc) {
      // Clear per-thread caches so the UI can rehydrate from the refreshed thread state.
      this.itemsByThreadId.delete(session.threadId);
      this.latestDiffByThreadId.delete(session.threadId);
      this.streamState.set(session.threadId, { activeTurnId: null });

      const thread = await this.buildThreadFromOpencodeSession(
        session.threadId,
        oc.client,
      );
      return {
        thread,
        model: modelSettings?.model ?? "",
        modelProvider: modelSettings?.provider ?? "",
        cwd: folder.uri.fsPath,
        approvalPolicy: "on-request" as AskForApproval,
        sandbox: { type: "dangerFullAccess" } as SandboxPolicy,
        reasoningEffort: null,
      };
    }
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
      personality: null,
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

    await this.startForBackendId(folder, session.backendId);
    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");

    await proc.threadArchive({ threadId: session.threadId });
  }

  public async unarchiveThreadForWorkspaceFolderAndBackendId(
    folder: vscode.WorkspaceFolder,
    backendId: BackendId,
    threadId: string,
  ): Promise<void> {
    if (backendId === "opencode") {
      throw new Error(
        "thread/unarchive is not supported for opencode backend.",
      );
    }

    const backendKey = makeBackendInstanceKey(folder.uri.toString(), backendId);
    await this.startForBackendId(folder, backendId);
    const proc = this.processes.get(backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");

    await proc.threadUnarchive({ threadId });
  }

  public async readAccount(session: Session): Promise<GetAccountResponse> {
    const folder = this.resolveWorkspaceFolder(session.workspaceFolderUri);
    if (!folder) {
      throw new Error(
        `WorkspaceFolder not found for session: ${session.workspaceFolderUri}`,
      );
    }
    await this.startForBackendId(folder, session.backendId);
    if (this.opencode.get(session.backendKey)) {
      throw new Error(
        "Accounts are not supported when running opencode backend.",
      );
    }
    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");
    return await proc.accountRead({ refreshToken: false });
  }

  public async loginAccount(
    session: Session,
    params: LoginAccountParams,
  ): Promise<LoginAccountResponse> {
    const folder = this.resolveWorkspaceFolder(session.workspaceFolderUri);
    if (!folder) {
      throw new Error(
        `WorkspaceFolder not found for session: ${session.workspaceFolderUri}`,
      );
    }
    await this.startForBackendId(folder, session.backendId);
    if (this.opencode.get(session.backendKey)) {
      throw new Error(
        "Accounts are not supported when running opencode backend.",
      );
    }
    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");
    return await proc.accountLoginStart(params);
  }

  public async listAccounts(session: Session): Promise<ListAccountsResponse> {
    const folder = this.resolveWorkspaceFolder(session.workspaceFolderUri);
    if (!folder) {
      throw new Error(
        `WorkspaceFolder not found for session: ${session.workspaceFolderUri}`,
      );
    }
    await this.startForBackendId(folder, session.backendId);
    if (this.opencode.get(session.backendKey)) {
      throw new Error(
        "Accounts are not supported when running opencode backend.",
      );
    }
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
    await this.startForBackendId(folder, session.backendId);
    if (this.opencode.get(session.backendKey)) {
      throw new Error(
        "Accounts are not supported when running opencode backend.",
      );
    }
    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");
    return await proc.accountSwitch(params);
  }

  public async logoutAccount(session: Session): Promise<LogoutAccountResponse> {
    const folder = this.resolveWorkspaceFolder(session.workspaceFolderUri);
    if (!folder) {
      throw new Error(
        `WorkspaceFolder not found for session: ${session.workspaceFolderUri}`,
      );
    }
    await this.startForBackendId(folder, session.backendId);
    if (this.opencode.get(session.backendKey)) {
      throw new Error(
        "Accounts are not supported when running opencode backend.",
      );
    }
    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");
    return await proc.accountLogout();
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
    await this.startForBackendId(folder, session.backendId);
    if (this.opencode.get(session.backendKey)) {
      throw new Error(
        "Accounts are not supported when running opencode backend.",
      );
    }
    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");
    return await proc.accountRateLimitsRead();
  }

  public async opencodeListProviders(
    session: Session,
  ): Promise<OpencodeProviderListResponse> {
    const oc = this.opencode.get(session.backendKey);
    if (!oc)
      throw new Error(
        "opencode backend is not running for this workspace folder",
      );
    return await oc.client.listProviders();
  }

  public async opencodeListProviderAuthMethods(
    session: Session,
  ): Promise<OpencodeProviderAuthMethodsResponse> {
    const oc = this.opencode.get(session.backendKey);
    if (!oc)
      throw new Error(
        "opencode backend is not running for this workspace folder",
      );
    return await oc.client.listProviderAuthMethods();
  }

  public async opencodeProviderOauthAuthorize(
    session: Session,
    args: { providerID: string; method: number },
  ): Promise<OpencodeProviderAuthorization | null> {
    const oc = this.opencode.get(session.backendKey);
    if (!oc)
      throw new Error(
        "opencode backend is not running for this workspace folder",
      );
    return await oc.client.providerOauthAuthorize(args);
  }

  public async opencodeProviderOauthCallback(
    session: Session,
    args: { providerID: string; method: number; code?: string },
  ): Promise<void> {
    const oc = this.opencode.get(session.backendKey);
    if (!oc)
      throw new Error(
        "opencode backend is not running for this workspace folder",
      );
    await oc.client.providerOauthCallback(args);
  }

  public async opencodeSetProviderApiKey(
    session: Session,
    args: { providerID: string; apiKey: string },
  ): Promise<void> {
    const oc = this.opencode.get(session.backendKey);
    if (!oc)
      throw new Error(
        "opencode backend is not running for this workspace folder",
      );
    await oc.client.setProviderApiKey(args);
  }

  public latestDiff(session: Session): string | null {
    return this.latestDiffByThreadId.get(session.threadId) ?? null;
  }

  public getItem(threadId: string, itemId: string): ThreadItem | null {
    return this.itemsByThreadId.get(threadId)?.get(itemId) ?? null;
  }

  public getOpencodeSessionStatus(session: Session): string | null {
    const oc = this.opencode.get(session.backendKey);
    if (!oc) return null;
    return oc.sessionStatusById.get(session.threadId) ?? null;
  }

  public isOpencodeSessionBusy(session: Session): boolean {
    return this.getOpencodeSessionStatus(session) === "busy";
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
    extraInput: UserInput[] = [],
  ): Promise<void> {
    const folder = this.resolveWorkspaceFolder(session.workspaceFolderUri);
    if (!folder) {
      throw new Error(
        `WorkspaceFolder not found for session: ${session.workspaceFolderUri}`,
      );
    }

    // Backend can terminate unexpectedly; ensure it is started before sending.
    await this.startForBackendId(folder, session.backendId);

    const oc = this.opencode.get(session.backendKey);
    if (oc) {
      const trimmed = text.trim();
      if (!trimmed && images.length === 0) {
        throw new Error("Message must include text or images");
      }

      const turnId = randomUUID();
      oc.activeTurnIdBySession.set(session.threadId, turnId);
      this.streamState.set(session.threadId, { activeTurnId: turnId });
      this.emitNotification(session.backendKey, session, {
        method: "turn/started",
        params: { threadId: session.threadId, turn: { id: turnId } },
      });

      const model = (() => {
        if (modelSettings?.provider && modelSettings?.model) {
          return {
            providerID: modelSettings.provider,
            modelID: modelSettings.model,
          };
        }
        const raw = String(modelSettings?.model ?? "").trim();
        const idx = raw.indexOf(":");
        if (idx > 0 && idx < raw.length - 1) {
          return { providerID: raw.slice(0, idx), modelID: raw.slice(idx + 1) };
        }
        return undefined;
      })();
      const parts: Array<Record<string, unknown>> = [];
      if (trimmed) parts.push({ type: "text", text: trimmed });
      for (const img of images) {
        if (img.kind === "localImage") {
          const mime = imageMimeFromPath(img.path);
          if (!mime) {
            throw new Error(
              `Unsupported image extension for opencode: ${img.path} (expected png/jpg/jpeg/gif/webp/bmp/svg/tiff)`,
            );
          }
          parts.push({
            type: "file",
            mime,
            filename: img.path.split(/[\\/]/).pop() ?? "image",
            url: pathToFileURL(img.path).toString(),
          });
          continue;
        }
        if (img.kind === "imageUrl") {
          const raw = String(img.url ?? "").trim();
          // Avoid guessing MIME types for remote URLs; require a data URL if we can't infer.
          const m = /^data:([^;]+);base64,/.exec(raw);
          if (!m) {
            throw new Error(
              "opencode imageUrl input requires a data:...;base64,... URL (remote URLs are not supported by this UI yet).",
            );
          }
          const mime = m[1] ?? "";
          if (!mime.startsWith("image/")) {
            throw new Error(
              `opencode imageUrl input must be an image/* data URL (got ${mime || "unknown"})`,
            );
          }
          parts.push({
            type: "file",
            mime,
            filename: "image",
            url: raw,
          });
          continue;
        }
        const _exhaustive: never = img;
        void _exhaustive;
      }

      const imageSuffix = images.length > 0 ? ` [images=${images.length}]` : "";
      const preview = trimmed ? trimmed : "(image only)";
      this.output.appendLine(
        `\n>> (${session.title}) ${preview}${imageSuffix}`,
      );
      this.output.append(`<< (${session.title}) `);

      try {
        const variant = String(modelSettings?.reasoning ?? "").trim();
        // IMPORTANT:
        // OpenCode streams steps/tools/reasoning/text via SSE while processing the message.
        // Do not also render the returned /message response here, or we'll duplicate cards and
        // break ordering/state in the UI.
        await oc.client.prompt(session.threadId, {
          parts,
          model,
          agent: modelSettings?.agent ?? undefined,
          variant: variant ? variant : undefined,
        });
      } catch (err) {
        if (isOpencodeFetchConnectionFailure(err)) {
          const baseUrl = (() => {
            try {
              return (
                this.opencodeServer?.proc.getBaseUrl().toString() ?? "(unknown)"
              );
            } catch {
              return "(unknown)";
            }
          })();
          this.output.appendLine(
            `[opencode] prompt failed due to connection error; disposing backend (backendKey=${session.backendKey} baseUrl=${baseUrl})`,
          );
          // Do not silently retry the request; just reset the backend to a known state so the
          // next user action can succeed.
          this.disposeAllOpencodeBackends({
            reason: "exit",
            code: null,
            signal: null,
          });
        }
        this.emitNotification(session.backendKey, session, {
          method: "error",
          params: { error: { message: String(err) }, willRetry: false },
        });
        this.emitNotification(session.backendKey, session, {
          method: "turn/completed",
          params: {
            threadId: session.threadId,
            turn: { id: turnId, status: "failed" },
          },
        });
        this.streamState.set(session.threadId, { activeTurnId: null });
        oc.activeTurnIdBySession.delete(session.threadId);
        throw err;
      }
      return;
    }

    const proc = this.processes.get(session.backendKey);
    if (!proc)
      throw new Error("Backend is not running for this workspace folder");

    const input: UserInput[] = [];
    if (text.trim()) input.push({ type: "text", text, text_elements: [] });
    input.push(...extraInput);
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
    const collaborationMode = modelSettings?.collaborationMode ?? null;
    const params: TurnStartParams = {
      threadId: session.threadId,
      input,
      cwd: null,
      approvalPolicy: null,
      sandboxPolicy: null,
      model: collaborationMode ? null : (modelSettings?.model ?? null),
      effort: collaborationMode ? null : effort,
      summary: null,
      personality: modelSettings?.personality ?? null,
      outputSchema: null,
      collaborationMode,
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

  public async replyOpencodePermission(args: {
    session: Session;
    requestID: string;
    reply: "once" | "always" | "reject";
    message?: string;
  }): Promise<void> {
    const folder = this.resolveWorkspaceFolder(args.session.workspaceFolderUri);
    if (!folder) {
      throw new Error(
        `WorkspaceFolder not found for session: ${args.session.workspaceFolderUri}`,
      );
    }
    await this.startForBackendId(folder, "opencode");
    const oc = this.opencode.get(args.session.backendKey);
    if (!oc) {
      throw new Error(
        `opencode backend is not running for backendKey=${args.session.backendKey}`,
      );
    }
    await oc.client.replyPermission({
      requestID: args.requestID,
      reply: args.reply,
      message: args.message,
    });
  }

  public async interruptTurn(session: Session, turnId: string): Promise<void> {
    const folder = this.resolveWorkspaceFolder(session.workspaceFolderUri);
    if (!folder) {
      throw new Error(
        `WorkspaceFolder not found for session: ${session.workspaceFolderUri}`,
      );
    }

    await this.startForBackendId(folder, session.backendId);
    const oc = this.opencode.get(session.backendKey);
    if (oc) {
      await oc.client.abort(session.threadId);
      // Best-effort: mark turn completed in the UI (opencode uses session.abort).
      const active = oc.activeTurnIdBySession.get(session.threadId) ?? turnId;
      this.emitNotification(session.backendKey, session, {
        method: "turn/completed",
        params: {
          threadId: session.threadId,
          turn: { id: active, status: "interrupted" },
        },
      });
      this.streamState.set(session.threadId, { activeTurnId: null });
      oc.activeTurnIdBySession.delete(session.threadId);
      return;
    }
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

    await this.startForBackendId(folder, session.backendId);
    const oc = this.opencode.get(session.backendKey);
    if (oc) {
      if (oc.sessionStatusById.get(session.threadId) === "busy") {
        throw new Error(
          "OpenCode session is busy. Stop the current turn before rewinding.",
        );
      }
      // Clear per-thread caches so the UI can rehydrate from the updated thread state.
      this.itemsByThreadId.delete(session.threadId);
      this.latestDiffByThreadId.delete(session.threadId);
      this.streamState.set(session.threadId, { activeTurnId: null });

      const threadBefore = await this.buildThreadFromOpencodeSession(
        session.threadId,
        oc.client,
      );
      const turns = Array.isArray(threadBefore.turns) ? threadBefore.turns : [];
      if (turns.length === 0) {
        throw new Error("No turns to rewind.");
      }
      const numTurns = Math.trunc(args.numTurns);
      if (!Number.isFinite(numTurns) || numTurns < 1) {
        throw new Error(`Invalid numTurns: ${String(args.numTurns)}`);
      }
      if (numTurns > turns.length) {
        throw new Error(
          `Cannot rewind ${numTurns} turns (total=${turns.length}).`,
        );
      }
      // Each opencode "turn" is based on a user message ID (turn.id is `turn:<messageId>`).
      const targetTurn = turns[turns.length - numTurns]!;
      const turnId = String(targetTurn.id ?? "");
      const m = turnId.match(/^turn:(.+)$/);
      if (!m) {
        throw new Error(`Unexpected opencode turn id: ${turnId}`);
      }
      const messageID = m[1]!;
      await oc.client.revert(session.threadId, messageID);
      const thread = await this.buildThreadFromOpencodeSession(
        session.threadId,
        oc.client,
      );
      return { thread };
    }
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

    await this.startForBackendId(folder, session.backendId);
    const oc = this.opencode.get(session.backendKey);
    if (oc) {
      throw new Error(
        "opencode backend does not support /compact via this UI yet (requires selecting provider/model for summarization).",
      );
    }
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

  private emitNotification(
    backendKey: string,
    session: Session | null,
    n: AnyServerNotification,
  ): void {
    // Maintain minimal caches needed by the extension outside of the webview runtime.
    if (n.method === "turn/diff/updated" && session) {
      const diff = (n as any).params?.diff;
      if (typeof diff === "string") {
        this.latestDiffByThreadId.set(session.threadId, diff);
      }
    }
    if (
      (n.method === "item/started" || n.method === "item/completed") &&
      session
    ) {
      const p = (n as any).params as { threadId?: unknown; item?: unknown };
      const threadId =
        typeof p?.threadId === "string" ? p.threadId : session.threadId;
      const item = p?.item as ThreadItem | undefined;
      if (threadId && item && typeof (item as any).id === "string") {
        this.upsertItem(threadId, item);
      }
    }
    this.onServerEvent?.(backendKey, session, n);
  }

  private onOpencodeEvent(
    backendKey: string,
    raw: OpencodeEvent,
    messageRoleById: Map<string, "user" | "assistant">,
    activeTurnIdBySession: Map<string, string>,
    pendingAssistantTextDeltasByKey: Map<string, string[]>,
    toolPartByToolId: Map<string, unknown>,
    toolIdsNeedingStepByMessageId: Map<string, Set<string>>,
    partItemIdByKey: Map<string, string>,
    activeStepIdByMessageId: Map<string, string>,
    sessionStatusById: Map<string, string>,
    client: OpencodeHttpClient,
  ): void {
    const assistantDeltaKey = (sessionID: string, messageID: string): string =>
      `${sessionID}:${messageID}`;

    const ensureOpencodeMessageSeq = (
      sessionID: string,
      messageID: string,
    ): number | null => {
      if (!sessionID || !messageID) return null;
      const byMsg =
        this.opencodeMessageSeqByMessageIdByBackendKey.get(backendKey) ??
        new Map<string, number>();
      this.opencodeMessageSeqByMessageIdByBackendKey.set(backendKey, byMsg);
      const existing = byMsg.get(messageID);
      if (typeof existing === "number") return existing;
      const next = this.opencodeNextMessageSeqByBackendKey.get(backendKey) ?? 1;
      byMsg.set(messageID, next);
      this.opencodeNextMessageSeqByBackendKey.set(backendKey, next + 1);
      return next;
    };

    const getOpencodeMessageSeq = (messageID: string): number | null => {
      const byMsg =
        this.opencodeMessageSeqByMessageIdByBackendKey.get(backendKey) ?? null;
      const v = byMsg?.get(messageID);
      return typeof v === "number" ? v : null;
    };

    const setOpencodePendingAssistantMessage = (args: {
      sessionID: string;
      messageID: string;
      pending: boolean;
    }): void => {
      const bySession =
        this.opencodePendingAssistantMessageIdsBySessionIdByBackendKey.get(
          backendKey,
        ) ?? new Map<string, Set<string>>();
      this.opencodePendingAssistantMessageIdsBySessionIdByBackendKey.set(
        backendKey,
        bySession,
      );
      const set = bySession.get(args.sessionID) ?? new Set<string>();
      if (args.pending) set.add(args.messageID);
      else set.delete(args.messageID);
      if (set.size > 0) bySession.set(args.sessionID, set);
      else bySession.delete(args.sessionID);
    };

    const hasOpencodePendingAssistantMessages = (
      sessionID: string,
    ): boolean => {
      const bySession =
        this.opencodePendingAssistantMessageIdsBySessionIdByBackendKey.get(
          backendKey,
        ) ?? null;
      const set = bySession?.get(sessionID) ?? null;
      return Boolean(set && set.size > 0);
    };

    const emitToolItem = (args: {
      session: Session;
      sessionID: string;
      turnId: string;
      messageID: string;
      stepId: string | null;
      toolId: string;
      part: any;
    }): void => {
      const callID =
        typeof args.part?.callID === "string"
          ? (args.part.callID as string)
          : null;
      const toolName =
        typeof args.part?.tool === "string"
          ? (args.part.tool as string)
          : "tool";

      const rawStatus =
        typeof args.part?.state?.status === "string"
          ? String(args.part.state.status)
          : null;
      const status = opencodeToolUiStatus(rawStatus);
      const title =
        typeof args.part?.state?.input?.description === "string"
          ? (args.part.state.input.description as string)
          : typeof args.part?.title === "string"
            ? (args.part.title as string)
            : null;

      const input =
        typeof args.part?.state?.input !== "undefined"
          ? args.part.state.input
          : null;
      const output =
        typeof args.part?.state?.output !== "undefined"
          ? args.part.state.output
          : typeof args.part?.state?.metadata?.output !== "undefined"
            ? args.part.state.metadata.output
            : null;

      const completed = rawStatus === "completed";
      const opencodeSeq = getOpencodeMessageSeq(args.messageID);
      this.emitNotification(backendKey, args.session, {
        method: completed ? "item/completed" : "item/started",
        params: {
          threadId: args.sessionID,
          turnId: args.turnId,
          item: {
            type: "opencodeTool",
            id: args.toolId,
            stepId: args.stepId,
            messageID: args.messageID,
            opencodeSeq,
            callID,
            tool: toolName,
            status,
            title,
            input,
            output,
            raw: args.part,
          },
        },
      });
    };

    const evt = (raw as any)?.payload
      ? ((raw as any).payload as any)
      : (raw as any);
    const type = typeof evt?.type === "string" ? (evt.type as string) : "";
    const properties = evt?.properties as any;
    if (!type) return;

    if (type === "server.heartbeat" || type === "session.updated") {
      // Noise events: ignore by default (no UI updates).
      return;
    }

    if (type === "permission.asked") {
      const requestID =
        typeof properties?.id === "string" ? String(properties.id) : null;
      const sessionID =
        typeof properties?.sessionID === "string"
          ? String(properties.sessionID)
          : null;
      const permission =
        typeof properties?.permission === "string"
          ? String(properties.permission)
          : null;
      const patterns = Array.isArray(properties?.patterns)
        ? (properties.patterns as unknown[])
            .map((p) => String(p ?? ""))
            .filter(Boolean)
        : [];
      const always = Array.isArray(properties?.always)
        ? (properties.always as unknown[])
            .map((p) => String(p ?? ""))
            .filter(Boolean)
        : [];
      const metadata =
        typeof properties?.metadata === "object" && properties.metadata !== null
          ? (properties.metadata as Record<string, unknown>)
          : null;
      if (!requestID || !sessionID || !permission) return;

      const session = this.sessions.getByThreadId(backendKey, sessionID);
      if (!session) return;
      const turnId = activeTurnIdBySession.get(sessionID) ?? "";
      this.emitNotification(backendKey, session, {
        method: "opencode/permission/asked",
        params: {
          threadId: sessionID,
          turnId,
          requestID,
          permission,
          patterns,
          always,
          metadata,
        },
      });
      return;
    }

    if (type === "permission.replied") {
      const sessionID =
        typeof properties?.sessionID === "string"
          ? String(properties.sessionID)
          : null;
      const requestID =
        typeof properties?.requestID === "string"
          ? String(properties.requestID)
          : null;
      const reply =
        typeof properties?.reply === "string" ? String(properties.reply) : null;
      if (!sessionID || !requestID || !reply) return;
      const session = this.sessions.getByThreadId(backendKey, sessionID);
      if (!session) return;
      const turnId = activeTurnIdBySession.get(sessionID) ?? "";
      this.emitNotification(backendKey, session, {
        method: "opencode/permission/replied",
        params: {
          threadId: sessionID,
          turnId,
          requestID,
          reply,
        },
      });
      return;
    }

    if (type === "question.asked") {
      const requestID =
        typeof properties?.id === "string" ? String(properties.id) : null;
      const sessionID =
        typeof properties?.sessionID === "string"
          ? String(properties.sessionID)
          : null;
      const questionsRaw = Array.isArray(properties?.questions)
        ? (properties.questions as unknown[])
        : null;
      const toolRaw =
        typeof properties?.tool === "object" && properties.tool !== null
          ? (properties.tool as Record<string, unknown>)
          : null;

      if (!requestID || !sessionID || !questionsRaw) return;

      const questions: OpencodeQuestionRequest["questions"] = [];
      for (const q of questionsRaw) {
        if (typeof q !== "object" || q === null) continue;
        const qq = q as Record<string, unknown>;
        const question =
          typeof qq.question === "string" ? String(qq.question) : null;
        const header = typeof qq.header === "string" ? String(qq.header) : "";
        const optionsRaw = Array.isArray(qq.options)
          ? (qq.options as unknown[])
          : [];
        const options: Array<{ label: string; description: string }> = [];
        for (const opt of optionsRaw) {
          if (typeof opt !== "object" || opt === null) continue;
          const oo = opt as Record<string, unknown>;
          const label = typeof oo.label === "string" ? String(oo.label) : null;
          const description =
            typeof oo.description === "string" ? String(oo.description) : "";
          if (!label) continue;
          options.push({ label, description });
        }
        if (!question) continue;
        const multiple =
          typeof qq.multiple === "boolean" ? Boolean(qq.multiple) : undefined;
        const custom =
          typeof qq.custom === "boolean" ? Boolean(qq.custom) : undefined;
        questions.push({ question, header, options, multiple, custom });
      }
      const tool =
        toolRaw &&
        typeof toolRaw.messageID === "string" &&
        typeof toolRaw.callID === "string"
          ? {
              messageID: String(toolRaw.messageID),
              callID: String(toolRaw.callID),
            }
          : undefined;

      const session = this.sessions.getByThreadId(backendKey, sessionID);
      if (!session) return;

      const turnId = activeTurnIdBySession.get(sessionID) ?? "";
      void this.handleOpencodeQuestionAsked({
        backendKey,
        session,
        client,
        requestID,
        sessionID,
        turnId,
        questions,
        tool,
      });
      return;
    }

    if (type === "session.status" || type === "session.idle") {
      const sessionID =
        typeof properties?.sessionID === "string"
          ? (properties.sessionID as string)
          : null;
      const statusType =
        type === "session.idle"
          ? "idle"
          : typeof properties?.status?.type === "string"
            ? String(properties.status.type)
            : null;
      if (sessionID && statusType) sessionStatusById.set(sessionID, statusType);
      if (sessionID && statusType === "busy") {
        const session = this.sessions.getByThreadId(backendKey, sessionID);
        if (!session) return;
        if (!activeTurnIdBySession.get(sessionID)) {
          const turnId = randomUUID();
          activeTurnIdBySession.set(sessionID, turnId);
          this.streamState.set(sessionID, { activeTurnId: turnId });
          this.emitNotification(backendKey, session, {
            method: "turn/started",
            params: { threadId: sessionID, turn: { id: turnId } },
          });
        }
        return;
      }
      if (sessionID && statusType === "idle") {
        // IMPORTANT:
        // In OpenCode, "session.status=idle" can be observed while a multi-message response is still ongoing.
        // Mirror the official TUI's behavior: only complete the turn when all assistant messages have `time.completed`.
        if (hasOpencodePendingAssistantMessages(sessionID)) return;

        // Session became idle and there are no pending assistant messages => finalize the current turn.
        const session = this.sessions.getByThreadId(backendKey, sessionID);
        if (!session) return;
        const turnId = activeTurnIdBySession.get(sessionID) ?? "";
        const idsBySession =
          this.opencodeReasoningItemIdsBySessionIdByBackendKey.get(
            backendKey,
          ) ?? null;
        const textByItem =
          this.opencodeReasoningTextByItemIdByBackendKey.get(backendKey) ??
          null;
        const ids = idsBySession?.get(sessionID) ?? null;
        if (ids && ids.size > 0) {
          for (const itemId of ids) {
            const text = textByItem?.get(itemId) ?? "";
            this.emitNotification(backendKey, session, {
              method: "item/completed",
              params: {
                threadId: sessionID,
                turnId,
                item: {
                  type: "reasoning",
                  id: itemId,
                  summary: text.trim() ? [text] : [],
                  content: [],
                },
              },
            });
          }
          idsBySession?.delete(sessionID);
        }

        const activeTurnId = activeTurnIdBySession.get(sessionID) ?? null;
        if (activeTurnId) {
          this.emitNotification(backendKey, session, {
            method: "turn/completed",
            params: {
              threadId: sessionID,
              turn: { id: activeTurnId, status: "completed" },
            },
          });
        }
        this.streamState.set(sessionID, { activeTurnId: null });
        activeTurnIdBySession.delete(sessionID);
      }
      return;
    }

    if (type === "message.updated") {
      const info = properties?.info as any;
      const role = info?.role;
      const messageID =
        typeof info?.id === "string" ? (info.id as string) : null;
      const sessionID =
        typeof info?.sessionID === "string" ? (info.sessionID as string) : null;
      if (sessionID && messageID) {
        ensureOpencodeMessageSeq(sessionID, messageID);
      }
      if (messageID && (role === "user" || role === "assistant")) {
        messageRoleById.set(messageID, role);
      }
      if (sessionID && messageID && role === "assistant") {
        const completed =
          typeof info?.time?.completed === "number" &&
          Number.isFinite(info.time.completed)
            ? Number(info.time.completed)
            : null;
        setOpencodePendingAssistantMessage({
          sessionID,
          messageID,
          pending: completed === null,
        });

        // If we already saw status=idle, complete the turn as soon as the final pending assistant message is done.
        if (
          completed !== null &&
          !hasOpencodePendingAssistantMessages(sessionID)
        ) {
          const status = sessionStatusById.get(sessionID) ?? null;
          if (status === "idle") {
            const session = this.sessions.getByThreadId(backendKey, sessionID);
            if (!session) return;
            const activeTurnId = activeTurnIdBySession.get(sessionID) ?? null;
            if (activeTurnId) {
              this.emitNotification(backendKey, session, {
                method: "turn/completed",
                params: {
                  threadId: sessionID,
                  turn: { id: activeTurnId, status: "completed" },
                },
              });
            }
            this.streamState.set(sessionID, { activeTurnId: null });
            activeTurnIdBySession.delete(sessionID);
          }
        }
      }
      if (sessionID && messageID && role === "assistant") {
        const key = assistantDeltaKey(sessionID, messageID);
        const pending = pendingAssistantTextDeltasByKey.get(key) ?? null;
        if (pending && pending.length > 0) {
          const session = this.sessions.getByThreadId(backendKey, sessionID);
          if (!session) return;
          const turnId = activeTurnIdBySession.get(sessionID) ?? "";
          for (const delta of pending) {
            this.emitNotification(backendKey, session, {
              method: "item/agentMessage/delta",
              params: {
                threadId: sessionID,
                turnId,
                itemId: messageID,
                delta,
                opencodeSeq: getOpencodeMessageSeq(messageID),
              },
            });
          }
        }
        pendingAssistantTextDeltasByKey.delete(key);
      }
      return;
    }

    if (type === "message.part.updated") {
      const part = properties?.part as any;
      const delta =
        typeof properties?.delta === "string"
          ? (properties.delta as string)
          : null;
      const sessionID =
        typeof part?.sessionID === "string" ? (part.sessionID as string) : null;
      const messageID =
        typeof part?.messageID === "string" ? (part.messageID as string) : null;
      const partType =
        typeof part?.type === "string" ? (part.type as string) : null;
      if (!sessionID || !messageID || !partType) return;
      const role = messageRoleById.get(messageID) ?? null;
      const session = this.sessions.getByThreadId(backendKey, sessionID);
      if (!session) return;
      const turnId = activeTurnIdBySession.get(sessionID) ?? "";

      if (partType === "text") {
        if (!delta) return;
        if (role !== "assistant") {
          // Opencode can emit part updates before message metadata (role) arrives.
          // Buffer assistant text deltas until we confirm role=assistant.
          if (role === null) {
            const key = assistantDeltaKey(sessionID, messageID);
            const prev = pendingAssistantTextDeltasByKey.get(key) ?? [];
            pendingAssistantTextDeltasByKey.set(key, [...prev, delta]);
          }
          return;
        }
        this.emitNotification(backendKey, session, {
          method: "item/agentMessage/delta",
          params: {
            threadId: sessionID,
            turnId,
            itemId: messageID,
            delta,
            opencodeSeq: getOpencodeMessageSeq(messageID),
          },
        });
        return;
      }

      if (partType === "step-start") {
        const snapshot =
          typeof part?.snapshot === "string" ? (part.snapshot as string) : null;
        const stepId = opencodeStepItemId({
          messageID,
          snapshot,
          partId: typeof part?.id === "string" ? (part.id as string) : null,
        });
        activeStepIdByMessageId.set(messageID, stepId);
        this.emitNotification(backendKey, session, {
          method: "item/started",
          params: {
            threadId: sessionID,
            turnId,
            item: {
              type: "opencodeStep",
              id: stepId,
              status: "inProgress",
              messageID,
              opencodeSeq: getOpencodeMessageSeq(messageID),
              snapshot,
              reason: null,
              cost: null,
              tokens: null,
            },
          },
        });

        const needing = toolIdsNeedingStepByMessageId.get(messageID) ?? null;
        if (needing && needing.size > 0) {
          for (const toolId of needing) {
            const stored = toolPartByToolId.get(toolId) ?? null;
            if (!stored) continue;
            emitToolItem({
              session,
              sessionID,
              turnId,
              messageID,
              stepId,
              toolId,
              part: stored,
            });
          }
          toolIdsNeedingStepByMessageId.delete(messageID);
        }
        return;
      }

      if (partType === "step-finish") {
        const snapshot =
          typeof part?.snapshot === "string" ? (part.snapshot as string) : null;
        const activeStepId = activeStepIdByMessageId.get(messageID) ?? null;
        const stepId =
          activeStepId ??
          opencodeStepItemId({
            messageID,
            snapshot,
            partId: typeof part?.id === "string" ? (part.id as string) : null,
          });
        const reason =
          typeof part?.reason === "string" ? (part.reason as string) : null;
        const cost =
          typeof part?.cost === "number" && Number.isFinite(part.cost)
            ? Number(part.cost)
            : null;
        const tokens =
          typeof part?.tokens === "object" && part.tokens !== null
            ? (part.tokens as Record<string, unknown>)
            : null;
        this.emitNotification(backendKey, session, {
          method: "item/completed",
          params: {
            threadId: sessionID,
            turnId,
            item: {
              type: "opencodeStep",
              id: stepId,
              status: "completed",
              messageID,
              opencodeSeq: getOpencodeMessageSeq(messageID),
              snapshot,
              reason,
              cost,
              tokens,
            },
          },
        });

        const needing = toolIdsNeedingStepByMessageId.get(messageID) ?? null;
        if (needing && needing.size > 0) {
          for (const toolId of needing) {
            const stored = toolPartByToolId.get(toolId) ?? null;
            if (!stored) continue;
            emitToolItem({
              session,
              sessionID,
              turnId,
              messageID,
              stepId,
              toolId,
              part: stored,
            });
          }
          toolIdsNeedingStepByMessageId.delete(messageID);
        }
        activeStepIdByMessageId.delete(messageID);
        return;
      }

      if (partType === "tool") {
        const callID =
          typeof part?.callID === "string" ? (part.callID as string) : null;
        const toolName =
          typeof part?.tool === "string" ? (part.tool as string) : "tool";
        const toolId = opencodeToolItemId({
          messageID,
          callID,
          partId: typeof part?.id === "string" ? (part.id as string) : null,
        });
        const stepId = activeStepIdByMessageId.get(messageID) ?? null;

        const rawStatus =
          typeof part?.state?.status === "string"
            ? String(part.state.status)
            : null;
        const status = opencodeToolUiStatus(rawStatus);
        const title =
          typeof part?.state?.title === "string"
            ? (part.state.title as string)
            : typeof part?.state?.input?.description === "string"
              ? (part.state.input.description as string)
              : typeof part?.title === "string"
                ? (part.title as string)
                : null;

        const input =
          typeof part?.state?.input !== "undefined" ? part.state.input : null;
        const output =
          typeof part?.state?.output !== "undefined"
            ? part.state.output
            : typeof part?.state?.metadata?.output !== "undefined"
              ? part.state.metadata.output
              : null;

        toolPartByToolId.set(toolId, part);
        if (!stepId) {
          const set = toolIdsNeedingStepByMessageId.get(messageID) ?? new Set();
          set.add(toolId);
          toolIdsNeedingStepByMessageId.set(messageID, set);
        }
        const completed = rawStatus === "completed";
        this.emitNotification(backendKey, session, {
          method: completed ? "item/completed" : "item/started",
          params: {
            threadId: sessionID,
            turnId,
            item: {
              type: "opencodeTool",
              id: toolId,
              stepId,
              messageID,
              opencodeSeq: getOpencodeMessageSeq(messageID),
              callID,
              tool: toolName,
              status,
              title,
              input,
              output,
              raw: part,
            },
          },
        });

        if (!stepId) {
          this.output.appendLine(
            `[opencode] tool part without active step: messageID=${messageID} callID=${callID ?? ""} tool=${toolName}`,
          );
        }
        return;
      }

      if (partType === "file") {
        const fileId = opencodePartItemIdFromPart({
          messageID,
          partType,
          partId: typeof part?.id === "string" ? (part.id as string) : null,
          index: 0,
        });
        const filename =
          typeof part?.filename === "string" ? (part.filename as string) : null;
        const mime =
          typeof part?.mime === "string"
            ? (part.mime as string)
            : "application/octet-stream";
        const url = typeof part?.url === "string" ? (part.url as string) : null;
        this.emitNotification(backendKey, session, {
          method: "item/completed",
          params: {
            threadId: sessionID,
            turnId,
            item: {
              type: "opencodeFile",
              id: fileId,
              messageID,
              opencodeSeq: getOpencodeMessageSeq(messageID),
              role,
              filename,
              mime,
              url,
              raw: part,
            },
          },
        });
        return;
      }

      if (partType === "patch") {
        const patchId = opencodePartItemIdFromPart({
          messageID,
          partType,
          partId: typeof part?.id === "string" ? (part.id as string) : null,
          index: 0,
        });
        const hash =
          typeof part?.hash === "string" ? (part.hash as string) : null;
        const files = Array.isArray(part?.files)
          ? (part.files as string[])
          : [];
        this.emitNotification(backendKey, session, {
          method: "item/completed",
          params: {
            threadId: sessionID,
            turnId,
            item: {
              type: "opencodePatch",
              id: patchId,
              messageID,
              opencodeSeq: getOpencodeMessageSeq(messageID),
              hash,
              files,
              raw: part,
            },
          },
        });
        return;
      }

      if (partType === "agent") {
        const agentId = opencodePartItemIdFromPart({
          messageID,
          partType,
          partId: typeof part?.id === "string" ? (part.id as string) : null,
          index: 0,
        });
        const agentName =
          typeof part?.name === "string" ? (part.name as string) : "agent";
        const source =
          typeof part?.source === "object" && part.source !== null
            ? (part.source as { value?: string; start?: number; end?: number })
            : null;
        this.emitNotification(backendKey, session, {
          method: "item/completed",
          params: {
            threadId: sessionID,
            turnId,
            item: {
              type: "opencodeAgent",
              id: agentId,
              messageID,
              opencodeSeq: getOpencodeMessageSeq(messageID),
              name: agentName,
              source,
              raw: part,
            },
          },
        });
        return;
      }

      if (partType === "snapshot") {
        const snapshotId = opencodePartItemIdFromPart({
          messageID,
          partType,
          partId: typeof part?.id === "string" ? (part.id as string) : null,
          index: 0,
        });
        const snapshot =
          typeof part?.snapshot === "string" ? (part.snapshot as string) : null;
        this.emitNotification(backendKey, session, {
          method: "item/completed",
          params: {
            threadId: sessionID,
            turnId,
            item: {
              type: "opencodeSnapshot",
              id: snapshotId,
              messageID,
              opencodeSeq: getOpencodeMessageSeq(messageID),
              snapshot,
              raw: part,
            },
          },
        });
        return;
      }

      if (partType === "retry") {
        const retryId = opencodePartItemIdFromPart({
          messageID,
          partType,
          partId: typeof part?.id === "string" ? (part.id as string) : null,
          index: 0,
        });
        const attempt =
          typeof part?.attempt === "number" && Number.isFinite(part.attempt)
            ? Math.trunc(part.attempt as number)
            : 1;
        const errorMessage =
          typeof part?.error?.message === "string"
            ? (part.error.message as string)
            : typeof part?.error === "string"
              ? (part.error as string)
              : "Retry failed";
        this.emitNotification(backendKey, session, {
          method: "item/completed",
          params: {
            threadId: sessionID,
            turnId,
            item: {
              type: "opencodeRetry",
              id: retryId,
              messageID,
              opencodeSeq: getOpencodeMessageSeq(messageID),
              attempt,
              error: errorMessage,
              raw: part,
            },
          },
        });
        return;
      }

      if (partType === "compaction") {
        const compactionId = opencodePartItemIdFromPart({
          messageID,
          partType,
          partId: typeof part?.id === "string" ? (part.id as string) : null,
          index: 0,
        });
        const auto =
          typeof part?.auto === "boolean" ? Boolean(part.auto) : false;
        this.emitNotification(backendKey, session, {
          method: "item/completed",
          params: {
            threadId: sessionID,
            turnId,
            item: {
              type: "opencodeCompaction",
              id: compactionId,
              messageID,
              opencodeSeq: getOpencodeMessageSeq(messageID),
              auto,
              raw: part,
            },
          },
        });
        return;
      }

      if (partType === "subtask") {
        const subtaskId = opencodePartItemIdFromPart({
          messageID,
          partType,
          partId: typeof part?.id === "string" ? (part.id as string) : null,
          index: 0,
        });
        const prompt =
          typeof part?.prompt === "string" ? (part.prompt as string) : null;
        const description =
          typeof part?.description === "string"
            ? (part.description as string)
            : null;
        const agent =
          typeof part?.agent === "string" ? (part.agent as string) : null;
        const model =
          typeof part?.model === "object" && part.model !== null
            ? (part.model as { providerID?: string; modelID?: string })
            : null;
        const command =
          typeof part?.command === "string" ? (part.command as string) : null;
        this.emitNotification(backendKey, session, {
          method: "item/completed",
          params: {
            threadId: sessionID,
            turnId,
            item: {
              type: "opencodeSubtask",
              id: subtaskId,
              messageID,
              opencodeSeq: getOpencodeMessageSeq(messageID),
              prompt,
              description,
              agent,
              model,
              command,
              raw: part,
            },
          },
        });
        return;
      }

      // For unknown/other part types, only surface a UI item if we actually have new text.
      // Otherwise we'd create empty "black cards" (started without any progress content).
      const fullText =
        typeof part?.text === "string" ? (part.text as string) : null;
      if (!delta && !fullText) return;

      const partIndex =
        typeof part?.index === "number" && Number.isFinite(part.index)
          ? Math.trunc(part.index)
          : null;
      const partKey = `${sessionID}:${messageID}:${partType}:${String(partIndex ?? "")}`;
      const itemId = (() => {
        if (partType === "reasoning") {
          const byMsg =
            this.opencodeReasoningItemIdByMessageIdByBackendKey.get(
              backendKey,
            ) ?? new Map<string, string>();
          this.opencodeReasoningItemIdByMessageIdByBackendKey.set(
            backendKey,
            byMsg,
          );
          const existing = byMsg.get(messageID) ?? null;
          if (existing) return existing;
          const id = `${messageID}:reasoning`;
          byMsg.set(messageID, id);

          const byItem =
            this.opencodeReasoningTextByItemIdByBackendKey.get(backendKey) ??
            new Map<string, string>();
          this.opencodeReasoningTextByItemIdByBackendKey.set(
            backendKey,
            byItem,
          );

          const bySession =
            this.opencodeReasoningItemIdsBySessionIdByBackendKey.get(
              backendKey,
            ) ?? new Map<string, Set<string>>();
          this.opencodeReasoningItemIdsBySessionIdByBackendKey.set(
            backendKey,
            bySession,
          );
          const set = bySession.get(sessionID) ?? new Set<string>();
          set.add(id);
          bySession.set(sessionID, set);

          this.emitNotification(backendKey, session, {
            method: "item/started",
            params: {
              threadId: sessionID,
              turnId,
              item: {
                type: "reasoning",
                id,
                messageID,
                opencodeSeq: getOpencodeMessageSeq(messageID),
                summary: [],
                content: [],
              } as any,
            },
          });
          return id;
        }

        return (
          partItemIdByKey.get(partKey) ??
          (() => {
            const index = partIndex ?? partItemIdByKey.size;
            const id = opencodePartItemIdFromPart({
              messageID,
              partType,
              partId: typeof part?.id === "string" ? (part.id as string) : null,
              index,
            });
            partItemIdByKey.set(partKey, id);
            this.emitNotification(backendKey, session, {
              method: "item/started",
              params: {
                threadId: sessionID,
                turnId,
                item: {
                  type: "mcpToolCall",
                  id,
                  server: "opencode",
                  tool: partType,
                  opencodeSeq: getOpencodeMessageSeq(messageID),
                  status: "inProgress",
                  result: null,
                  error: null,
                },
              },
            });
            return id;
          })()
        );
      })();

      if (partType === "reasoning") {
        const byItem =
          this.opencodeReasoningTextByItemIdByBackendKey.get(backendKey) ??
          new Map<string, string>();
        if (delta) {
          const prev = byItem.get(itemId) ?? "";
          byItem.set(itemId, prev + delta);
        } else if (fullText !== null) {
          byItem.set(itemId, fullText);
        }
        this.opencodeReasoningTextByItemIdByBackendKey.set(backendKey, byItem);
        if (delta) {
          this.emitNotification(backendKey, session, {
            method: "item/reasoning/summaryTextDelta",
            params: {
              threadId: sessionID,
              turnId,
              itemId,
              summaryIndex: 0,
              delta,
            },
          });
        }

        const end =
          typeof part?.time?.end === "number" && Number.isFinite(part.time.end)
            ? Number(part.time.end)
            : null;
        if (!delta || end !== null) {
          const text = byItem.get(itemId) ?? "";
          this.emitNotification(backendKey, session, {
            method: "item/completed",
            params: {
              threadId: sessionID,
              turnId,
              item: {
                type: "reasoning",
                id: itemId,
                messageID,
                opencodeSeq: getOpencodeMessageSeq(messageID),
                summary: text.trim() ? [text] : [],
                content: [],
              } as any,
            },
          });
        }
        return;
      }
      this.emitNotification(backendKey, session, {
        method: "item/mcpToolCall/progress",
        params: {
          threadId: sessionID,
          turnId,
          itemId,
          server: "opencode",
          tool: partType,
          opencodeSeq: getOpencodeMessageSeq(messageID),
          message: delta ?? fullText ?? "",
        },
      });
      return;
    }

    if (type === "session.diff") {
      const sessionID =
        typeof properties?.sessionID === "string"
          ? (properties.sessionID as string)
          : null;
      const diff = Array.isArray(properties?.diff)
        ? (properties.diff as OpencodeFileDiff[])
        : null;
      if (!sessionID || !diff) return;
      const session = this.sessions.getByThreadId(backendKey, sessionID);
      if (!session) return;
      const turnId = activeTurnIdBySession.get(sessionID) ?? "";
      const formatted = client.formatFileDiffs(diff);
      this.emitNotification(backendKey, session, {
        method: "turn/diff/updated",
        params: { threadId: sessionID, turnId, diff: formatted },
      });
      return;
    }

    // Debug: log unknown opencode event types so we can add structured handling later.
    // Keep this in output channel (not UI) to avoid surprising user-facing behavior.
    this.output.appendLine(
      `[opencode] Unhandled event: type=${type} keys=${Object.keys(properties ?? {}).join(",")}`,
    );
  }

  private async handleOpencodeQuestionAsked(args: {
    backendKey: string;
    session: Session;
    client: OpencodeHttpClient;
    requestID: string;
    sessionID: string;
    turnId: string;
    questions: OpencodeQuestionRequest["questions"];
    tool?: { messageID: string; callID: string };
  }): Promise<void> {
    const title =
      args.questions[0] && typeof args.questions[0].header === "string"
        ? args.questions[0].header
        : "OpenCode question";

    const questions = args.questions.map((q, idx) => ({
      id: `${args.requestID}:${String(idx)}`,
      header: q.header ?? title,
      question: q.question,
      allowMultiple: Boolean(q.multiple),
      isOther: q.custom === false ? false : true,
      isSecret: false,
      options:
        Array.isArray(q.options) && q.options.length > 0
          ? q.options.map((o) => ({
              label: o.label,
              description: o.description,
            }))
          : null,
    }));

    const { cancelled, answersById } = await promptRequestUserInput({
      title,
      questions,
    });

    if (cancelled) {
      await args.client.rejectQuestion({ requestID: args.requestID });
      return;
    }

    const answers: string[][] = questions.map((q) => answersById[q.id] ?? []);
    await args.client.replyQuestion({ requestID: args.requestID, answers });
  }

  private threadFromOpencodeSession(s: OpencodeSessionInfo): Thread {
    const createdMs =
      typeof s?.time?.created === "number" ? s.time.created : Date.now();
    const updatedMs =
      typeof s?.time?.updated === "number" ? s.time.updated : createdMs;
    return {
      id: String(s.id),
      preview: String(s.title ?? ""),
      modelProvider: "",
      createdAt: Math.floor(createdMs / 1000),
      updatedAt: Math.floor(updatedMs / 1000),
      path: "",
      cwd: String(s.directory ?? ""),
      cliVersion: "opencode",
      source: "unknown",
      gitInfo: null,
      turns: [],
    };
  }

  private async buildThreadFromOpencodeSession(
    sessionID: string,
    client: OpencodeHttpClient,
  ): Promise<Thread> {
    const session = await client.getSession(sessionID);
    const messages = await client.listMessages(sessionID);
    const thread = this.threadFromOpencodeSession(session);
    thread.turns = buildTurnsFromOpencodeMessages(messages);
    return thread;
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
    this.opencodeDefaultModelKeyByBackendKey.delete(backendKey);
    this.opencodeDefaultAgentNameByBackendKey.delete(backendKey);
    this.opencodeMessageSeqByMessageIdByBackendKey.delete(backendKey);
    this.opencodeNextMessageSeqByBackendKey.delete(backendKey);
    this.opencodePendingAssistantMessageIdsBySessionIdByBackendKey.delete(
      backendKey,
    );
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
      "params" in n
        ? this.sessionFromParams(backendKey, (n as any).params)
        : null;
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

      const session = this.sessions.getByThreadId(backendKey, p.threadId);
      if (session) {
        this.onTurnCompleted?.(session, p.turn.status, p.turn.id);
      }
      return;
    }

    if (n.method === "turn/diff/updated") {
      const session = this.sessions.getByThreadId(backendKey, p.threadId);
      if (!session) return;
      this.latestDiffByThreadId.set(p.threadId, p.diff);
      this.onDiffUpdated?.(session, p.diff, p.turnId);
      return;
    }

    if (n.method === "turn/plan/updated") {
      const session = this.sessions.getByThreadId(backendKey, p.threadId);
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
      const session = this.sessions.getByThreadId(backendKey, p.threadId);
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
      const session = this.sessions.getByThreadId(backendKey, p.threadId);
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
      const session = this.sessions.getByThreadId(backendKey, p.threadId);
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
      const session = this.sessions.getByThreadId(backendKey, p.threadId);
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
      const session = this.sessions.getByThreadId(backendKey, p.threadId);
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
      const session = this.sessions.getByThreadId(backendKey, p.threadId);
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
      const session = this.sessions.getByThreadId(backendKey, p.threadId);
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

  private sessionFromParams(
    backendKey: string,
    params: unknown,
  ): Session | null {
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
      if (msgThreadId)
        return this.sessions.getByThreadId(backendKey, msgThreadId);
    }

    if (typeof threadId !== "string") return null;
    return this.sessions.getByThreadId(backendKey, threadId);
  }

  public dispose(): void {
    for (const proc of this.processes.values()) proc.dispose();
    this.processes.clear();
    for (const oc of this.opencode.values()) oc.sse.abort();
    this.opencode.clear();
    this.disposeOpencodeServerIfRunning();
  }

  private async handleApprovalRequest(
    backendKey: string,
    req: V2ApprovalRequest,
  ): Promise<V2ApprovalDecision> {
    const session = this.sessions.getByThreadId(
      backendKey,
      req.params.threadId,
    );
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

  private disposeOpencodeBackend(
    backendKey: string,
    info: BackendTermination,
  ): void {
    const current = this.opencode.get(backendKey);
    if (!current) return;
    try {
      current.sse.abort();
    } catch {
      // ignore
    }
    this.opencode.delete(backendKey);
    this.cleanupBackendCaches(backendKey);
    this.onBackendTerminated?.(backendKey, info);
    this.disposeOpencodeServerIfUnused();
  }

  private disposeOpencodeServerIfUnused(): void {
    if (this.opencode.size > 0) return;
    this.disposeOpencodeServerIfRunning();
  }

  private disposeOpencodeServerIfRunning(): void {
    const server = this.opencodeServer;
    if (!server) return;
    server.proc.dispose();
    this.opencodeServer = null;
    this.opencodeServerInFlight = null;
  }

  private async ensureOpencodeServer(args: {
    folder: vscode.WorkspaceFolder;
    command: string;
    args: string[];
  }): Promise<{
    proc: OpencodeServerProcess;
    command: string;
    args: string[];
  }> {
    if (this.opencodeServer) {
      const sameCommand = this.opencodeServer.command === args.command;
      const sameArgs =
        this.opencodeServer.args.length === args.args.length &&
        this.opencodeServer.args.every((v, i) => v === args.args[i]);
      if (!sameCommand || !sameArgs) {
        throw new Error(
          `opencode server is already running with different command/args; stop opencode backends and retry (current=${this.opencodeServer.command} ${this.opencodeServer.args.join(" ")} next=${args.command} ${args.args.join(" ")})`,
        );
      }
      return this.opencodeServer;
    }
    if (this.opencodeServerInFlight) return await this.opencodeServerInFlight;

    const start = (async () => {
      const proc = await OpencodeServerProcess.spawn({
        command: args.command,
        args: args.args,
        cwd: args.folder.uri.fsPath,
        output: this.output,
      });
      const server = { proc, command: args.command, args: [...args.args] };

      proc.onDidExit(({ code, signal }) => {
        // Shared opencode server died unexpectedly. Tear down every per-workspace SSE stream.
        const backendKeys = [...this.opencode.keys()];
        for (const backendKey of backendKeys) {
          this.disposeOpencodeBackend(backendKey, {
            reason: "exit",
            code,
            signal,
          });
        }
        this.disposeOpencodeServerIfRunning();
      });

      this.opencodeServer = server;
      this.opencodeServerInFlight = null;
      return server;
    })();

    this.opencodeServerInFlight = start;
    return await start;
  }

  private disposeAllOpencodeBackends(info: BackendTermination): void {
    const backendKeys = [...this.opencode.keys()];
    for (const backendKey of backendKeys) {
      this.disposeOpencodeBackend(backendKey, info);
    }
    this.disposeOpencodeServerIfRunning();
  }
}

function isOpencodeFetchConnectionFailure(err: unknown): boolean {
  const msg = err instanceof Error ? err.message : String(err);
  if (!msg) return false;
  // Node's fetch() often wraps connection errors as "TypeError: fetch failed" with a cause.
  if (msg.includes("fetch failed") && msg.includes("127.0.0.1")) return true;
  if (msg.includes("ECONNREFUSED")) return true;
  if (msg.includes("ECONNRESET")) return true;
  if (msg.includes("EPIPE")) return true;
  if (msg.includes("socket hang up")) return true;
  const cause = err && typeof err === "object" ? (err as any).cause : null;
  const code = typeof cause?.code === "string" ? String(cause.code) : "";
  if (code === "ECONNREFUSED" || code === "ECONNRESET" || code === "EPIPE")
    return true;
  return false;
}

function extractOpencodeText(msg: OpencodeMessageWithParts): string {
  const parts = Array.isArray(msg.parts) ? msg.parts : [];
  const out: string[] = [];
  for (const p of parts) {
    if (
      p &&
      (p as any).type === "text" &&
      typeof (p as any).text === "string"
    ) {
      out.push(String((p as any).text));
    }
  }
  return out.join("");
}

function opencodePartItemId(args: {
  messageID: string;
  partType: string;
  index: number;
}): string {
  return `${args.messageID}:part:${args.partType}:${String(args.index)}`;
}

function opencodePartItemIdKey(args: {
  messageID: string;
  partType: string;
  key: string;
}): string {
  const cleaned = args.key.trim() || "unknown";
  return `${args.messageID}:part:${args.partType}:${cleaned}`;
}

function opencodePartItemIdFromPart(args: {
  messageID: string;
  partType: string;
  partId: string | null;
  index: number;
}): string {
  const key = args.partId?.trim() ?? "";
  if (key)
    return opencodePartItemIdKey({
      messageID: args.messageID,
      partType: args.partType,
      key,
    });
  return opencodePartItemId({
    messageID: args.messageID,
    partType: args.partType,
    index: args.index,
  });
}

function opencodeStepItemId(args: {
  messageID: string;
  snapshot: string | null;
  partId: string | null;
}): string {
  const key = args.snapshot?.trim() || args.partId?.trim() || "unknown";
  return `${args.messageID}:step:${key}`;
}

function opencodeToolItemId(args: {
  messageID: string;
  callID: string | null;
  partId: string | null;
}): string {
  const key = args.callID?.trim() || args.partId?.trim() || "unknown";
  return `${args.messageID}:tool:${key}`;
}

function opencodeToolUiStatus(rawStatus: string | null): string {
  if (rawStatus === "completed") return "completed";
  if (rawStatus === "error" || rawStatus === "failed") return "failed";
  // opencode emits: pending / running / completed
  return "inProgress";
}

function opencodePartToThreadItem(
  part: Record<string, unknown>,
  itemId: string,
): ThreadItem {
  const type = String(part.type ?? "part");
  if (type === "reasoning") {
    const summaryRaw =
      typeof (part as any).summary === "string"
        ? String((part as any).summary)
        : "";
    const contentRaw =
      typeof (part as any).text === "string" ? String((part as any).text) : "";
    const merged = contentRaw.trim() ? contentRaw : summaryRaw;
    return {
      type: "reasoning",
      id: itemId,
      // Prefer opencode web's behavior: render reasoning as markdown text (not a raw JSON blob).
      // The VSCode UI uses `summaryParts` as markdown and `rawParts` as a nested "Raw" <pre>.
      // Keep `content` empty so we don't render a "Raw" section for reasoning by default.
      summary: merged ? [merged] : [],
      content: [],
    } as any;
  }

  return {
    type: "mcpToolCall",
    id: itemId,
    server: "opencode",
    tool: type,
    status: "completed",
    result: {
      content: [
        {
          type: "text",
          text: JSON.stringify(part, null, 2),
          text_elements: [],
        },
      ],
    },
    error: null,
  } as any;
}

function buildOpencodeItemsFromParts(
  messageID: string,
  parts: any[],
): ThreadItem[] {
  const out: ThreadItem[] = [];
  let activeStepId: string | null = null;
  let reasoningItem: ThreadItem | null = null;

  for (let i = 0; i < parts.length; i += 1) {
    const p = parts[i];
    if (!p || typeof p !== "object") continue;
    const type = String((p as any).type ?? "part");
    if (type === "text") continue;

    if (type === "step-start") {
      const snapshot =
        typeof (p as any).snapshot === "string"
          ? String((p as any).snapshot)
          : null;
      const stepId = opencodeStepItemId({
        messageID,
        snapshot,
        partId:
          typeof (p as any).id === "string" ? String((p as any).id) : null,
      });
      activeStepId = stepId;
      out.push({
        type: "opencodeStep",
        id: stepId,
        status: "inProgress",
        messageID,
        snapshot,
        reason: null,
        cost: null,
        tokens: null,
      } as any);
      continue;
    }

    if (type === "step-finish") {
      const snapshot =
        typeof (p as any).snapshot === "string"
          ? String((p as any).snapshot)
          : null;
      const stepId: string =
        activeStepId ??
        opencodeStepItemId({
          messageID,
          snapshot,
          partId:
            typeof (p as any).id === "string" ? String((p as any).id) : null,
        });
      out.push({
        type: "opencodeStep",
        id: stepId,
        status: "completed",
        messageID,
        snapshot,
        reason:
          typeof (p as any).reason === "string"
            ? String((p as any).reason)
            : null,
        cost:
          typeof (p as any).cost === "number" &&
          Number.isFinite((p as any).cost)
            ? Number((p as any).cost)
            : null,
        tokens:
          typeof (p as any).tokens === "object" && (p as any).tokens !== null
            ? ((p as any).tokens as Record<string, unknown>)
            : null,
      } as any);
      if (activeStepId === stepId) activeStepId = null;
      continue;
    }

    if (type === "tool") {
      const callID =
        typeof (p as any).callID === "string"
          ? String((p as any).callID)
          : null;
      const toolId = opencodeToolItemId({
        messageID,
        callID,
        partId:
          typeof (p as any).id === "string" ? String((p as any).id) : null,
      });
      const toolName =
        typeof (p as any).tool === "string" ? String((p as any).tool) : "tool";
      const rawStatus =
        typeof (p as any).state?.status === "string"
          ? String((p as any).state.status)
          : null;
      const status = opencodeToolUiStatus(rawStatus);
      const title =
        typeof (p as any).state?.input?.description === "string"
          ? String((p as any).state.input.description)
          : typeof (p as any).title === "string"
            ? String((p as any).title)
            : null;
      const input =
        typeof (p as any).state?.input !== "undefined"
          ? (p as any).state.input
          : null;
      const output =
        typeof (p as any).state?.output !== "undefined"
          ? (p as any).state.output
          : typeof (p as any).state?.metadata?.output !== "undefined"
            ? (p as any).state.metadata.output
            : null;
      out.push({
        type: "opencodeTool",
        id: toolId,
        stepId: activeStepId,
        messageID,
        callID,
        tool: toolName,
        status,
        title,
        input,
        output,
        raw: p,
      } as any);
      continue;
    }

    if (type === "reasoning") {
      // opencode can emit multiple reasoning parts per assistant message; aggregate into
      // a single UI block to avoid clutter (mirrors opencode web which renders it as one section).
      const text =
        typeof (p as any).text === "string" ? String((p as any).text) : "";
      const summary =
        typeof (p as any).summary === "string"
          ? String((p as any).summary)
          : "";
      const merged = text.trim() ? text : summary;
      if (!merged.trim()) continue;

      if (!reasoningItem) {
        const itemId = `${messageID}:reasoning`;
        reasoningItem = opencodePartToThreadItem(p as any, itemId);
        out.push(reasoningItem);
      } else {
        const next = reasoningItem as any;
        if (!Array.isArray(next.summary)) next.summary = [];
        next.summary.push(merged);
      }
      continue;
    }

    const partIndexRaw = (p as any).index;
    const partIndex =
      typeof partIndexRaw === "number" && Number.isFinite(partIndexRaw)
        ? Math.trunc(partIndexRaw)
        : i;
    out.push(
      opencodePartToThreadItem(
        p as any,
        opencodePartItemId({ messageID, partType: type, index: partIndex }),
      ),
    );
  }

  return out;
}

function buildTurnsFromOpencodeMessages(
  messages: OpencodeMessageWithParts[],
): Turn[] {
  const out: Turn[] = [];
  let current: { id: string; items: ThreadItem[] } | null = null;

  const pushCurrent = () => {
    if (!current) return;
    out.push({
      id: current.id,
      items: current.items,
      status: "completed",
      error: null,
    });
    current = null;
  };

  for (const m of messages) {
    const role = (m as any)?.info?.role;
    const messageID =
      typeof (m as any)?.info?.id === "string" ? (m as any).info.id : "";
    if (!messageID) continue;
    const parts = Array.isArray((m as any).parts)
      ? ((m as any).parts as any[])
      : [];
    if (role === "user") {
      pushCurrent();
      const userText = extractOpencodeText(m);
      current = { id: `turn:${messageID}`, items: [] };
      current.items.push(...buildOpencodeItemsFromParts(messageID, parts));
      current.items.push({
        type: "userMessage",
        id: messageID,
        content: userText
          ? [{ type: "text", text: userText, text_elements: [] }]
          : [],
      } as any);
      continue;
    }
    if (role === "assistant") {
      if (!current) {
        current = { id: `turn:${messageID}`, items: [] };
      }
      current.items.push(...buildOpencodeItemsFromParts(messageID, parts));
      const text = extractOpencodeText(m);
      if (text.trim()) {
        current.items.push({
          type: "agentMessage",
          id: messageID,
          text,
        } as any);
      }
      continue;
    }
  }
  pushCurrent();
  return out;
}

type V2ApprovalRequest = Extract<
  ServerRequest,
  {
    method:
      | "item/commandExecution/requestApproval"
      | "item/fileChange/requestApproval";
  }
>;

type V2ApprovalDecision =
  | CommandExecutionApprovalDecision
  | FileChangeApprovalDecision;

function parseOpencodeDefaultModelKey(
  cfg: Record<string, unknown> | null,
): string | null {
  if (!cfg) return null;
  const raw =
    typeof cfg["model"] === "string" ? String(cfg["model"]).trim() : "";
  if (!raw) return null;

  // opencode config commonly uses `provider/modelID` (e.g. "openai/gpt-5.2").
  const slash = raw.indexOf("/");
  if (slash > 0 && slash < raw.length - 1) {
    const providerID = raw.slice(0, slash).trim();
    const modelID = raw.slice(slash + 1).trim();
    if (providerID && modelID) return `${providerID}:${modelID}`;
  }

  // Some surfaces may already use `provider:modelID`.
  const colon = raw.indexOf(":");
  if (colon > 0 && colon < raw.length - 1) {
    const providerID = raw.slice(0, colon).trim();
    const modelID = raw.slice(colon + 1).trim();
    if (providerID && modelID) return `${providerID}:${modelID}`;
  }

  return null;
}

function resolveOpencodeDefaultAgentName(
  cfg: Record<string, unknown> | null,
): string | null {
  if (!cfg) return null;

  const rawDefault =
    typeof cfg["default_agent"] === "string"
      ? String(cfg["default_agent"]).trim()
      : "";
  if (rawDefault) return rawDefault;

  // Best-effort: match opencode's default behavior where "build" is default unless disabled.
  const agent = cfg["agent"];
  if (agent && typeof agent === "object" && !Array.isArray(agent)) {
    const build = (agent as any)["build"];
    const buildDisabled = Boolean(build && (build as any).disable === true);
    if (buildDisabled) return "plan";
  }

  return "build";
}

function parseOpencodeEnabledProviders(
  cfg: Record<string, unknown> | null,
): Set<string> | null {
  if (!cfg) return null;
  const raw = cfg["enabled_providers"];
  if (!Array.isArray(raw)) return null;
  const out = new Set<string>();
  for (const v of raw) {
    const s = typeof v === "string" ? v.trim() : "";
    if (s) out.add(s);
  }
  return out.size > 0 ? out : null;
}

function parseOpencodeDisabledProviders(
  cfg: Record<string, unknown> | null,
): Set<string> | null {
  if (!cfg) return null;
  const raw = cfg["disabled_providers"];
  if (!Array.isArray(raw)) return null;
  const out = new Set<string>();
  for (const v of raw) {
    const s = typeof v === "string" ? v.trim() : "";
    if (s) out.add(s);
  }
  return out.size > 0 ? out : null;
}

function parseOpencodeProviderAllowlist(
  cfg: Record<string, unknown> | null,
): Set<string> | null {
  // Prefer enabled_providers (explicit allowlist).
  const enabled = parseOpencodeEnabledProviders(cfg);
  if (enabled) return enabled;

  // Legacy/alt: config.provider object keys.
  if (!cfg) return null;
  const raw = cfg["provider"];
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
  const keys = Object.keys(raw as Record<string, unknown>)
    .map((k) => k.trim())
    .filter(Boolean);
  return keys.length > 0 ? new Set(keys) : null;
}

function resolveOpencodeDefaultModelKey(
  cfg: Record<string, unknown> | null,
  providers: OpencodeProviderListResponse,
): string | null {
  const fromConfig = parseOpencodeDefaultModelKey(cfg);
  if (fromConfig) return fromConfig;

  const providerAllowlist = parseOpencodeProviderAllowlist(cfg);
  const providerBlocklist = parseOpencodeDisabledProviders(cfg);

  const connected = Array.isArray(providers?.connected)
    ? providers.connected
    : [];
  const all = Array.isArray(providers?.all) ? providers.all : [];
  const defaultByProvider =
    typeof providers?.default === "object" && providers.default !== null
      ? providers.default
      : {};

  const hasProvider = (providerID: string): boolean => {
    if (!providerID) return false;
    if (providerBlocklist && providerBlocklist.has(providerID)) return false;
    if (providerAllowlist && !providerAllowlist.has(providerID)) return false;
    return all.some((p) => String((p as any)?.id ?? "") === providerID);
  };

  const providerID =
    connected.find(hasProvider) ??
    all.map((p) => String((p as any)?.id ?? "")).find(hasProvider) ??
    null;
  if (!providerID) return null;

  const modelID =
    typeof (defaultByProvider as any)[providerID] === "string"
      ? String((defaultByProvider as any)[providerID]).trim()
      : "";
  if (!modelID) return null;

  return `${providerID}:${modelID}`;
}

function summarizeItem(
  phase: "started" | "completed",
  item: ThreadItem,
): string {
  const type = (item as any)?.type as string;
  const prefix = `[item ${phase}] ${type}`;
  switch (type) {
    case "commandExecution": {
      const it = item as any;
      const status = phase === "completed" ? ` status=${it.status}` : "";
      const exitCode = it.exitCode !== null ? ` exitCode=${it.exitCode}` : "";
      return `${prefix}${status}${exitCode}\n$ ${it.command}\n`;
    }
    case "fileChange": {
      const it = item as any;
      const files = (it.changes ?? []).map((c: any) => c.path).join(", ");
      const status = phase === "completed" ? ` status=${it.status}` : "";
      return `${prefix}${status}\nfiles: ${files}\n`;
    }
    case "mcpToolCall": {
      const it = item as any;
      const status = phase === "completed" ? ` status=${it.status}` : "";
      return `${prefix}${status}\n${it.server}.${it.tool}\n`;
    }
    case "webSearch": {
      const it = item as any;
      return `${prefix}\nquery: ${it.query}\n`;
    }
    case "reasoning": {
      return `${prefix}\n`;
    }
    case "agentMessage": {
      return `${prefix}\n`;
    }
    case "imageView": {
      const it = item as any;
      return `${prefix}\npath: ${it.path}\n`;
    }
    case "userMessage": {
      return `${prefix}\n`;
    }
    case "enteredReviewMode":
    case "exitedReviewMode": {
      return `${prefix}\n`;
    }
    case "opencodeStep": {
      const anyItem = item as any;
      const status =
        phase === "completed" ? ` status=${String(anyItem.status ?? "")}` : "";
      const snapshot =
        typeof anyItem.snapshot === "string"
          ? ` snapshot=${String(anyItem.snapshot)}`
          : "";
      const reason =
        typeof anyItem.reason === "string"
          ? ` reason=${String(anyItem.reason)}`
          : "";
      return `${prefix}${status}${snapshot}${reason}\n`;
    }
    case "opencodeTool": {
      const anyItem = item as any;
      const status =
        phase === "completed" ? ` status=${String(anyItem.status ?? "")}` : "";
      const tool = typeof anyItem.tool === "string" ? String(anyItem.tool) : "";
      const callID =
        typeof anyItem.callID === "string"
          ? ` callID=${String(anyItem.callID)}`
          : "";
      return `${prefix}${status}\n${tool}${callID}\n`;
    }
    default: {
      return `${prefix}\n`;
    }
  }
}
