import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import * as vscode from "vscode";

import type { ClientNotification } from "../generated/ClientNotification";
import type { InitializeParams } from "../generated/InitializeParams";
import type { InitializeResponse } from "../generated/InitializeResponse";
import type { RequestId } from "../generated/RequestId";
import type { ServerRequest } from "../generated/ServerRequest";
import type { ThreadStartParams } from "../generated/v2/ThreadStartParams";
import type { ThreadStartResponse } from "../generated/v2/ThreadStartResponse";
import type { ThreadResumeParams } from "../generated/v2/ThreadResumeParams";
import type { ThreadResumeResponse } from "../generated/v2/ThreadResumeResponse";
import type { ThreadCompactParams } from "../generated/v2/ThreadCompactParams";
import type { ThreadCompactResponse } from "../generated/v2/ThreadCompactResponse";
import type { TurnStartParams } from "../generated/v2/TurnStartParams";
import type { TurnStartResponse } from "../generated/v2/TurnStartResponse";
import type { TurnInterruptParams } from "../generated/v2/TurnInterruptParams";
import type { TurnInterruptResponse } from "../generated/v2/TurnInterruptResponse";
import type { SkillsListParams } from "../generated/v2/SkillsListParams";
import type { SkillsListResponse } from "../generated/v2/SkillsListResponse";
import type { ModelListParams } from "../generated/v2/ModelListParams";
import type { ModelListResponse } from "../generated/v2/ModelListResponse";
import type { ThreadArchiveParams } from "../generated/v2/ThreadArchiveParams";
import type { ThreadArchiveResponse } from "../generated/v2/ThreadArchiveResponse";
import type { ThreadUnarchiveParams } from "../generated/v2/ThreadUnarchiveParams";
import type { ThreadUnarchiveResponse } from "../generated/v2/ThreadUnarchiveResponse";
import type { ThreadRollbackParams } from "../generated/v2/ThreadRollbackParams";
import type { ThreadRollbackResponse } from "../generated/v2/ThreadRollbackResponse";
import type { ThreadListParams } from "../generated/v2/ThreadListParams";
import type { ThreadListResponse } from "../generated/v2/ThreadListResponse";
import type { AppsListParams } from "../generated/v2/AppsListParams";
import type { AppsListResponse } from "../generated/v2/AppsListResponse";
import type { CollaborationModeListParams } from "../generated/v2/CollaborationModeListParams";
import type { CollaborationModeListResponse } from "../generated/v2/CollaborationModeListResponse";
import type { ListMcpServerStatusParams } from "../generated/v2/ListMcpServerStatusParams";
import type { ListMcpServerStatusResponse } from "../generated/v2/ListMcpServerStatusResponse";
import type { GetAccountParams } from "../generated/v2/GetAccountParams";
import type { GetAccountRateLimitsResponse } from "../generated/v2/GetAccountRateLimitsResponse";
import type { GetAccountResponse } from "../generated/v2/GetAccountResponse";
import type { LoginAccountParams } from "../generated/v2/LoginAccountParams";
import type { LoginAccountResponse } from "../generated/v2/LoginAccountResponse";
import type { ListAccountsResponse } from "../generated/v2/ListAccountsResponse";
import type { LogoutAccountResponse } from "../generated/v2/LogoutAccountResponse";
import type { SwitchAccountParams } from "../generated/v2/SwitchAccountParams";
import type { SwitchAccountResponse } from "../generated/v2/SwitchAccountResponse";
import type { CommandExecutionApprovalDecision } from "../generated/v2/CommandExecutionApprovalDecision";
import type { FileChangeApprovalDecision } from "../generated/v2/FileChangeApprovalDecision";
import type { CommandExecutionRequestApprovalResponse } from "../generated/v2/CommandExecutionRequestApprovalResponse";
import type { FileChangeRequestApprovalResponse } from "../generated/v2/FileChangeRequestApprovalResponse";
import type { ToolRequestUserInputResponse } from "../generated/v2/ToolRequestUserInputResponse";
import type { FuzzyFileSearchParams } from "../generated/FuzzyFileSearchParams";
import type { FuzzyFileSearchResponse } from "../generated/FuzzyFileSearchResponse";
import type { ApplyPatchApprovalResponse } from "../generated/ApplyPatchApprovalResponse";
import type { ExecCommandApprovalResponse } from "../generated/ExecCommandApprovalResponse";
import type { ReviewDecision } from "../generated/ReviewDecision";
import { RpcClient } from "./rpc";
import type { AnyServerNotification } from "./types";
import { promptRequestUserInput } from "./request_user_input";

type SpawnOptions = {
  command: string;
  args: string[];
  cwd: string;
  logRpcPayloads: boolean;
  output: vscode.OutputChannel;
};

export type BackendExitInfo = {
  code: number | null;
  signal: NodeJS.Signals | null;
};

export class BackendProcess implements vscode.Disposable {
  private readonly exitHandlers = new Set<(info: BackendExitInfo) => void>();
  private readonly rpc: RpcClient;

  public onNotification: ((n: AnyServerNotification) => void) | null = null;
  public onApprovalRequest:
    | ((req: V2ApprovalRequest) => Promise<V2ApprovalDecision>)
    | null = null;

  private constructor(
    private readonly child: ChildProcessWithoutNullStreams,
    private readonly output: vscode.OutputChannel,
    private readonly logRpcPayloads: boolean,
    private readonly approvalsDefaultDecision: "prompt" | "decline" | "cancel",
    private readonly spawnCommand: string,
    private readonly spawnArgs: string[],
  ) {
    this.rpc = new RpcClient(child, output, logRpcPayloads);
    this.rpc.on("serverNotification", (n: AnyServerNotification) =>
      this.emitNotification(n),
    );
    this.rpc.on(
      "serverRequest",
      (r: ServerRequest) => void this.handleServerRequest(r),
    );
    this.rpc.on("exit", (info: BackendExitInfo) =>
      this.exitHandlers.forEach((h) => h(info)),
    );
  }

  public static async spawn(opts: SpawnOptions): Promise<BackendProcess> {
    const child = spawn(opts.command, opts.args, {
      cwd: opts.cwd,
      stdio: ["pipe", "pipe", "pipe"],
      env: process.env,
    });

    const cfg = vscode.workspace.getConfiguration("codez");
    const approvalsDefaultDecision =
      cfg.get<"prompt" | "decline" | "cancel">("approvals.defaultDecision") ??
      "prompt";

    const proc = new BackendProcess(
      child,
      opts.output,
      opts.logRpcPayloads,
      approvalsDefaultDecision,
      opts.command,
      opts.args,
    );
    await proc.initializeHandshake();
    return proc;
  }

  public getCommand(): string {
    return this.spawnCommand;
  }

  public getArgs(): string[] {
    return [...this.spawnArgs];
  }

  public onDidExit(handler: () => void): void {
    this.exitHandlers.add(() => handler());
  }

  public onDidExitWithInfo(handler: (info: BackendExitInfo) => void): void {
    this.exitHandlers.add(handler);
  }

  public dispose(): void {
    this.rpc.dispose();

    this.child.removeAllListeners();
    try {
      this.child.kill();
    } catch {
      // kill() can throw if already dead; surface via exit handlers instead.
    }
  }

  public kill(signal: NodeJS.Signals): void {
    try {
      this.child.kill(signal);
    } catch {
      // kill() can throw if already dead; surface via exit handlers instead.
    }
  }

  public async threadStart(
    params: ThreadStartParams,
  ): Promise<ThreadStartResponse> {
    return this.rpc.request<ThreadStartResponse>({
      method: "thread/start",
      params,
    });
  }

  public async threadResume(
    params: ThreadResumeParams,
  ): Promise<ThreadResumeResponse> {
    return this.rpc.request<ThreadResumeResponse>({
      method: "thread/resume",
      params,
    });
  }

  public async threadArchive(
    params: ThreadArchiveParams,
  ): Promise<ThreadArchiveResponse> {
    return this.rpc.request<ThreadArchiveResponse>({
      method: "thread/archive",
      params,
    });
  }

  public async threadUnarchive(
    params: ThreadUnarchiveParams,
  ): Promise<ThreadUnarchiveResponse> {
    return this.rpc.request<ThreadUnarchiveResponse>({
      method: "thread/unarchive",
      params,
    });
  }

  public async threadCompact(
    params: ThreadCompactParams,
  ): Promise<ThreadCompactResponse> {
    return this.rpc.request<ThreadCompactResponse>({
      method: "thread/compact",
      params,
    });
  }

  public async threadRollback(
    params: ThreadRollbackParams,
  ): Promise<ThreadRollbackResponse> {
    return this.rpc.request<ThreadRollbackResponse>({
      method: "thread/rollback",
      params,
    });
  }

  public async threadList(
    params: Partial<ThreadListParams> | undefined = undefined,
  ): Promise<ThreadListResponse> {
    const request: { method: "thread/list"; params: ThreadListParams } = {
      method: "thread/list",
      params: {
        cursor: params?.cursor ?? null,
        limit: params?.limit ?? null,
        sortKey: params?.sortKey ?? null,
        modelProviders: params?.modelProviders ?? null,
        sourceKinds: params?.sourceKinds ?? null,
        archived: params?.archived ?? null,
      },
    };
    return this.rpc.request<ThreadListResponse>(request);
  }

  public async appsList(
    params: Partial<AppsListParams> | undefined = undefined,
  ): Promise<AppsListResponse> {
    const request: { method: "app/list"; params: AppsListParams } = {
      method: "app/list",
      params: {
        cursor: params?.cursor ?? null,
        limit: params?.limit ?? null,
      },
    };
    return this.rpc.request<AppsListResponse>(request);
  }

  public async collaborationModeList(
    params: CollaborationModeListParams,
  ): Promise<CollaborationModeListResponse> {
    return this.rpc.request<CollaborationModeListResponse>({
      method: "collaborationMode/list",
      params,
    });
  }

  public async mcpServerStatusList(
    params: ListMcpServerStatusParams,
  ): Promise<ListMcpServerStatusResponse> {
    return this.rpc.request<ListMcpServerStatusResponse>({
      method: "mcpServerStatus/list",
      params,
    });
  }

  public async turnStart(params: TurnStartParams): Promise<TurnStartResponse> {
    return this.rpc.request<TurnStartResponse>({
      method: "turn/start",
      params,
    });
  }

  public async turnInterrupt(
    params: TurnInterruptParams,
  ): Promise<TurnInterruptResponse> {
    return this.rpc.request<TurnInterruptResponse>({
      method: "turn/interrupt",
      params,
    });
  }

  public async listModels(
    params: Partial<ModelListParams> | undefined = undefined,
  ): Promise<ModelListResponse> {
    const request: { method: "model/list"; params: ModelListParams } = {
      method: "model/list",
      params: {
        cursor: params?.cursor ?? null,
        limit: params?.limit ?? null,
      },
    };
    return this.rpc.request<ModelListResponse>(request);
  }

  public async skillsList(
    params: SkillsListParams,
  ): Promise<SkillsListResponse> {
    return this.rpc.request<SkillsListResponse>({
      method: "skills/list",
      params,
    });
  }

  public async accountRead(
    params: GetAccountParams,
  ): Promise<GetAccountResponse> {
    return this.rpc.request<GetAccountResponse>({
      method: "account/read",
      params,
    });
  }

  public async accountLoginStart(
    params: LoginAccountParams,
  ): Promise<LoginAccountResponse> {
    return this.rpc.request<LoginAccountResponse>({
      method: "account/login/start",
      params,
    });
  }

  public async accountList(): Promise<ListAccountsResponse> {
    return this.rpc.request<ListAccountsResponse>({
      method: "account/list",
      params: undefined,
    });
  }

  public async accountSwitch(
    params: SwitchAccountParams,
  ): Promise<SwitchAccountResponse> {
    return this.rpc.request<SwitchAccountResponse>({
      method: "account/switch",
      params,
    });
  }

  public async accountLogout(): Promise<LogoutAccountResponse> {
    return this.rpc.request<LogoutAccountResponse>({
      method: "account/logout",
      params: undefined,
    });
  }

  public async accountRateLimitsRead(): Promise<GetAccountRateLimitsResponse> {
    return this.rpc.request<GetAccountRateLimitsResponse>({
      method: "account/rateLimits/read",
      params: undefined,
    });
  }

  public async fuzzyFileSearch(
    params: FuzzyFileSearchParams,
  ): Promise<FuzzyFileSearchResponse> {
    return this.rpc.request<FuzzyFileSearchResponse>({
      method: "fuzzyFileSearch",
      params,
    });
  }

  private emitNotification(notification: AnyServerNotification): void {
    this.onNotification?.(notification);
  }

  private async initializeHandshake(): Promise<void> {
    const params: InitializeParams = {
      clientInfo: {
        name: "codez-vscode",
        title: "Codex UI VS Code Extension",
        version: "0.0.1",
      },
    };
    const result = await this.rpc.request<InitializeResponse>({
      method: "initialize",
      params,
    });
    this.output.appendLine(`Initialized (userAgent=${result.userAgent})`);

    const initialized: ClientNotification = { method: "initialized" };
    this.rpc.notify(initialized);
  }

  private async handleServerRequest(req: ServerRequest): Promise<void> {
    if (isV2ApprovalRequest(req) && this.onApprovalRequest) {
      try {
        const decision = await this.onApprovalRequest(req);
        this.respondV2Approval(req, decision);
        return;
      } catch (err) {
        this.output.appendLine(
          `Failed to handle approval request via UI, falling back to modal: ${String(err)}`,
        );
      }
    }

    if (isV2ToolRequestUserInputRequest(req)) {
      const questions = req.params.questions.map((q) => ({
        id: q.id,
        header: q.header,
        question: q.question,
        // Protocol currently doesn't expose an allowMultiple flag; default to single-select.
        allowMultiple: false,
        isOther: Boolean((q as any).isOther),
        isSecret: Boolean((q as any).isSecret),
        options: Array.isArray(q.options)
          ? q.options.map((o) => ({
              label: o.label,
              description: o.description,
            }))
          : null,
      }));

      const { answersById } = await promptRequestUserInput({
        title: "Codex: request_user_input",
        questions,
      });

      const answers: ToolRequestUserInputResponse["answers"] = {};
      for (const q of req.params.questions) {
        answers[q.id] = { answers: answersById[q.id] ?? [] };
      }
      this.respondV2ToolRequestUserInput(req.id, { answers });
      return;
    }

    if (this.approvalsDefaultDecision !== "prompt") {
      if (isV2ApprovalRequest(req)) {
        const decision =
          this.approvalsDefaultDecision === "decline" ? "decline" : "cancel";
        this.respondV2Approval(req, decision);
        return;
      }

      const decision =
        this.approvalsDefaultDecision === "decline" ? "denied" : "abort";
      this.respondV1Approval(req.id, decision);
      return;
    }

    const choice = await vscode.window.showWarningMessage(
      `Codex is requesting approval: `,
      {
        modal: true,
        detail: "Review the request and choose Accept or Decline.",
      },
      "Accept",
      "Accept (For Session)",
      "Decline",
      "Cancel",
    );

    if (choice === "Accept") {
      if (isV2ApprovalRequest(req)) this.respondV2Approval(req, "accept");
      else this.respondV1Approval(req.id, "approved");
      return;
    }
    if (choice === "Accept (For Session)") {
      if (isV2ApprovalRequest(req)) {
        this.respondV2Approval(req, "acceptForSession");
      } else {
        this.respondV1Approval(req.id, "approved_for_session");
      }
      return;
    }
    if (choice === "Decline") {
      if (isV2ApprovalRequest(req)) this.respondV2Approval(req, "decline");
      else this.respondV1Approval(req.id, "denied");
      return;
    }
    if (isV2ApprovalRequest(req)) this.respondV2Approval(req, "cancel");
    else this.respondV1Approval(req.id, "abort");
  }

  public respondV2Approval(
    req: V2ApprovalRequest,
    decision: V2ApprovalDecision,
  ): void {
    if (req.method === "item/commandExecution/requestApproval") {
      const result: CommandExecutionRequestApprovalResponse = {
        decision: decision as CommandExecutionApprovalDecision,
      };
      this.rpc.respond(req.id, result);
      return;
    }

    if (typeof decision === "object" && decision !== null) {
      throw new Error(
        "File change approvals do not support acceptWithExecpolicyAmendment",
      );
    }

    const result: FileChangeRequestApprovalResponse = {
      decision: decision as FileChangeApprovalDecision,
    };
    this.rpc.respond(req.id, result);
  }

  public respondV2ToolRequestUserInput(
    id: RequestId,
    result: ToolRequestUserInputResponse,
  ): void {
    this.rpc.respond(id, result);
  }

  public respondV1Approval(id: RequestId, decision: ReviewDecision): void {
    const result: ApplyPatchApprovalResponse | ExecCommandApprovalResponse = {
      decision,
    };
    this.rpc.respond(id, result);
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

type V2ApprovalDecision =
  | CommandExecutionApprovalDecision
  | FileChangeApprovalDecision;

type V2ToolRequestUserInputRequest = Extract<
  ServerRequest,
  { method: "item/tool/requestUserInput" }
>;

function isV2ApprovalRequest(req: ServerRequest): req is V2ApprovalRequest {
  return (
    req.method === "item/commandExecution/requestApproval" ||
    req.method === "item/fileChange/requestApproval"
  );
}

function isV2ToolRequestUserInputRequest(
  req: ServerRequest,
): req is V2ToolRequestUserInputRequest {
  return req.method === "item/tool/requestUserInput";
}
