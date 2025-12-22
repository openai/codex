import { parse as parseToml } from "@iarna/toml";
import * as crypto from "node:crypto";
import { spawn } from "node:child_process";
import * as fs from "node:fs/promises";
import * as os from "node:os";
import * as path from "node:path";
import { parse as shellParse } from "shell-quote";
import * as vscode from "vscode";
import { BackendManager } from "./backend/manager";
import { listAgentsFromDisk } from "./agents_disk";
import type { AnyServerNotification } from "./backend/types";
import type { CommandAction } from "./generated/v2/CommandAction";
import type { Model } from "./generated/v2/Model";
import type { RateLimitSnapshot } from "./generated/v2/RateLimitSnapshot";
import type { RateLimitWindow } from "./generated/v2/RateLimitWindow";
import type { Thread } from "./generated/v2/Thread";
import type { ThreadItem } from "./generated/v2/ThreadItem";
import type { ThreadTokenUsage } from "./generated/v2/ThreadTokenUsage";
import type { Turn } from "./generated/v2/Turn";
import type { Session } from "./sessions";
import { SessionStore } from "./sessions";
import {
  ChatViewProvider,
  getSessionModelState,
  setSessionModelState,
  type ChatBlock,
  type ChatViewState,
} from "./ui/chat_view";
import { DiffDocumentProvider, makeDiffUri } from "./ui/diff_provider";
import { SessionTreeDataProvider } from "./ui/session_tree";

let backendManager: BackendManager | null = null;
let sessions: SessionStore | null = null;
let sessionTree: SessionTreeDataProvider | null = null;
let diffProvider: DiffDocumentProvider | null = null;
let chatView: ChatViewProvider | null = null;
let activeSessionId: string | null = null;
let extensionContext: vscode.ExtensionContext | null = null;
let outputChannel: vscode.OutputChannel | null = null;

const HIDDEN_TAB_SESSIONS_KEY = "codexMine.hiddenTabSessions.v1";
const hiddenTabSessionIds = new Set<string>();
const mcpStatusByServer = new Map<string, string>();
const cliVariantByBackendKey = new Map<
  string,
  "unknown" | "codex" | "codex-mine"
>();
const defaultTitleRe = /^(.*)\s+\([0-9a-f]{8}\)$/i;

type CustomPromptSummary = {
  name: string;
  description: string | null;
  argumentHint: string | null;
  content: string;
  source: "disk" | "server";
};

type SessionRuntime = {
  blocks: ChatBlock[];
  latestDiff: string | null;
  statusText: string | null;
  tokenUsage: ThreadTokenUsage | null;
  sending: boolean;
  activeTurnId: string | null;
  lastTurnStartedAtMs: number | null;
  lastTurnCompletedAtMs: number | null;
  blockIndexById: Map<string, number>;
  legacyPatchTargetByCallId: Map<string, string>;
  legacyWebSearchTargetByCallId: Map<string, string>;
  pendingApprovals: Map<
    string,
    { title: string; detail: string; canAcceptForSession: boolean }
  >;
  approvalResolvers: Map<
    string,
    (decision: "accept" | "acceptForSession" | "decline" | "cancel") => void
  >;
};

const runtimeBySessionId = new Map<string, SessionRuntime>();
const globalRuntime: Pick<SessionRuntime, "blocks" | "blockIndexById"> = {
  blocks: [],
  blockIndexById: new Map<string, number>(),
};
let globalStatusText: string | null = null;
let customPrompts: CustomPromptSummary[] = [];
let sessionModelState: {
  model: string | null;
  provider: string | null;
  reasoning: string | null;
} = { model: null, provider: null, reasoning: null };
type ModelState = typeof sessionModelState;
const pendingModelFetchByBackend = new Map<string, Promise<void>>();
const PROMPTS_CMD_PREFIX = "prompts";

export function activate(context: vscode.ExtensionContext): void {
  extensionContext = context;
  const output = vscode.window.createOutputChannel("Codex UI");
  outputChannel = output;
  output.appendLine(`[debug] extensionPath=${context.extensionPath}`);
  void loadInitialModelState(output);

  sessions = new SessionStore();
  loadSessions(context, sessions);
  for (const s of sessions.listAll()) ensureRuntime(s.id);
  loadRuntimes(context, sessions);
  loadHiddenTabSessions(context);
  refreshCustomPromptsFromDisk();

  backendManager = new BackendManager(output, sessions);
  backendManager.onServerEvent = (session, n) => {
    if (session) applyServerNotification(session.id, n);
    else applyGlobalNotification(n);
  };
  backendManager.onSessionAdded = (s) => {
    saveSessions(context, sessions!);
    sessionTree?.refresh();
    setActiveSession(s.id);
    refreshCustomPromptsFromDisk();
    void ensureModelsFetched(s);
    void showCodexMineViewContainer();
  };
  backendManager.onApprovalRequest = async (session, req) => {
    const requestKey = requestKeyFromId(req.id);
    const rt = ensureRuntime(session.id);

    const item =
      backendManager?.getItem(session.threadId, req.params.itemId) ?? null;
    const reason = req.params.reason ?? null;
    const title =
      req.method === "item/commandExecution/requestApproval"
        ? "Command approval required"
        : "File change approval required";
    const detail = formatApprovalDetail(req.method, item, reason);

    rt.pendingApprovals.set(requestKey, {
      title,
      detail,
      canAcceptForSession: true,
    });
    chatView?.refresh();
    void showCodexMineViewContainer();

    return await new Promise((resolve) => {
      rt.approvalResolvers.set(requestKey, resolve);
    });
  };

  diffProvider = new DiffDocumentProvider();
  context.subscriptions.push(
    vscode.workspace.registerTextDocumentContentProvider(
      "codex-mine-diff",
      diffProvider,
    ),
  );
  context.subscriptions.push(diffProvider);

  sessionTree = new SessionTreeDataProvider(sessions);
  context.subscriptions.push(sessionTree);
  context.subscriptions.push(
    vscode.window.createTreeView("codexMine.sessionsView", {
      treeDataProvider: sessionTree,
    }),
  );

  chatView = new ChatViewProvider(
    context,
    () => buildChatState(),
    async (text) => {
      if (!backendManager) throw new Error("backendManager is not initialized");
      if (!sessions) throw new Error("sessions is not initialized");

      const session = activeSessionId
        ? sessions.getById(activeSessionId)
        : null;
      if (!session) {
        void vscode.window.showErrorMessage("No session selected.");
        return;
      }

      const slashHandled = await handleSlashCommand(context, session, text);
      if (slashHandled) return;

      const expanded = await expandMentions(session, text);
      if (!expanded.ok) {
        void vscode.window.showErrorMessage(expanded.error);
        return;
      }
      await sendUserText(session, expanded.text);
    },
    async () => {
      if (!sessions) throw new Error("sessions is not initialized");
      const session = activeSessionId
        ? sessions.getById(activeSessionId)
        : null;
      if (!session) {
        void vscode.window.showErrorMessage("No session selected.");
        return;
      }
      await vscode.commands.executeCommand("codexMine.openLatestDiff", {
        sessionId: session.id,
      });
    },
  );
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider(
      ChatViewProvider.viewType,
      chatView,
    ),
  );

  context.subscriptions.push(output);

  context.subscriptions.push(
    vscode.commands.registerCommand("codexMine.startBackend", async () => {
      if (!backendManager) throw new Error("backendManager is not initialized");

      const folder = await pickWorkspaceFolder();
      if (!folder) return;

      await backendManager.startForWorkspaceFolder(folder);
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("codexMine.newSession", async () => {
      if (!backendManager) throw new Error("backendManager is not initialized");

      const folder = await pickWorkspaceFolder();
      if (!folder) return;

      await ensureBackendMatchesConfiguredCli(folder, "newSession");
      const session = await backendManager.newSession(
        folder,
        getSessionModelState(),
      );
      setActiveSession(session.id);
      void ensureModelsFetched(session);
      await showCodexMineViewContainer();
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("codexMine.resumeFromHistory", async () => {
      if (!backendManager) throw new Error("backendManager is not initialized");
      if (!sessions) throw new Error("sessions is not initialized");
      if (!extensionContext) throw new Error("extensionContext is not set");

      const folder = await pickWorkspaceFolder();
      if (!folder) return;

      await ensureBackendMatchesConfiguredCli(folder, "newSession");
      const wantedCwd = normalizeFsPathForCompare(folder.uri.fsPath);

      let cursor: string | null = null;
      const collected: Thread[] = [];

      for (;;) {
        let res: { data: Thread[]; nextCursor: string | null };
        try {
          res = await backendManager.listThreadsForWorkspaceFolder(folder, {
            cursor,
            limit: 50,
            modelProviders: null,
          });
        } catch (err) {
          output.appendLine(`[resume] Failed to list threads: ${String(err)}`);
          void vscode.window.showErrorMessage("Failed to list history.");
          return;
        }

        const filtered = res.data.filter(
          (t) => normalizeFsPathForCompare(t.cwd) === wantedCwd,
        );
        collected.push(...filtered);

        const items = collected.map((t) => ({
          label: `${formatThreadWhen(t.createdAt)}  ${formatThreadLabel(t.preview)}`,
          thread: t,
          kind: "thread" as const,
        }));

        const hasMore = Boolean(res.nextCursor);
        const picked = await vscode.window.showQuickPick(
          [
            ...items,
            ...(hasMore
              ? [
                  {
                    label: "Load more…",
                    description: "",
                    detail: "",
                    kind: "more" as const,
                    nextCursor: res.nextCursor,
                  },
                ]
              : []),
          ] as any,
          {
            title: "Codex UI: Pick a thread to resume",
            matchOnDescription: true,
            matchOnDetail: true,
          },
        );

        if (!picked) return;
        if ((picked as any).kind === "more") {
          cursor = (picked as any).nextCursor ?? null;
          if (!cursor) return;
          continue;
        }

        const thread = (picked as any).thread as Thread;
        const session: Session = {
          id: crypto.randomUUID(),
          backendKey: folder.uri.toString(),
          workspaceFolderUri: folder.uri.toString(),
          title: normalizeSessionTitle(thread.preview || "Resumed"),
          threadId: thread.id,
        };

        sessions.add(session.backendKey, session);
        saveSessions(extensionContext, sessions);
        ensureRuntime(session.id);
        sessionTree?.refresh();

        // Don't override the recorded thread model on resume. Users can still
        // change the model via the UI for subsequent turns.
        const resumed = await backendManager.resumeSession(session);
        void ensureModelsFetched(session);
        hydrateRuntimeFromThread(session.id, resumed.thread);
        setActiveSession(session.id);
        refreshCustomPromptsFromDisk();
        await showCodexMineViewContainer();
        return;
      }
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("codexMine.interruptTurn", async () => {
      if (!backendManager) throw new Error("backendManager is not initialized");
      if (!sessions) throw new Error("sessions is not initialized");

      const session = activeSessionId ? sessions.getById(activeSessionId) : null;
      if (!session) return;

      const rt = ensureRuntime(session.id);
      const turnId = rt.activeTurnId;
      if (!turnId) return;

      try {
        await backendManager.interruptTurn(session, turnId);
      } catch (err) {
        output.appendLine(`[turn] Failed to interrupt: ${String(err)}`);
        upsertBlock(session.id, {
          id: newLocalId("error"),
          type: "error",
          title: "Interrupt failed",
          text: String(err),
        });
        chatView?.refresh();
      }
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("codexMine.showStatus", async () => {
      if (!backendManager) throw new Error("backendManager is not initialized");
      if (!sessions) throw new Error("sessions is not initialized");

      const session = activeSessionId ? sessions.getById(activeSessionId) : null;
      if (!session) {
        void vscode.window.showErrorMessage("No session selected.");
        return;
      }

      const rt = ensureRuntime(session.id);
      const settings = getSessionModelState();

      let rateLimits: RateLimitSnapshot | null = null;
      try {
        const res = await backendManager.readRateLimits(session);
        rateLimits = res.rateLimits;
      } catch (err) {
        output.appendLine(`[status] Failed to read rate limits: ${String(err)}`);
      }

      let accountLine: string | null = null;
      let planLine: string | null = null;
      try {
        const res = await backendManager.readAccount(session);
        const a = res.account;
        if (!a) accountLine = "Account: (unknown)";
        else if (a.type === "chatgpt") {
          accountLine = `Account: ${a.email} (${a.planType})`;
        } else {
          accountLine = "Account: apiKey";
          // For API key auth, planType may only be available from rate limits.
          const planFromLimits = rateLimits?.planType ?? null;
          planLine = planFromLimits ? `Plan: ${planFromLimits}` : null;
        }
      } catch (err) {
        output.appendLine(`[status] Failed to read account: ${String(err)}`);
      }

      const directory = (() => {
        try {
          return vscode.Uri.parse(session.workspaceFolderUri).fsPath;
        } catch {
          return null;
        }
      })();

      const modelLine = `Model: ${settings.model ?? "default"} (reasoning ${settings.reasoning ?? "default"})`;
      const sessionLine = `Session: ${session.threadId}`;
      const dirLine = directory ? `Directory: ${directory}` : null;
      if (!planLine) {
        // If we couldn't infer plan from account, fall back to rate limits.
        const planFromLimits = rateLimits?.planType ?? null;
        planLine = planFromLimits ? `Plan: ${planFromLimits}` : null;
        // Avoid duplicating plan if account already includes it.
        if (accountLine && accountLine.includes("(") && accountLine.includes(")")) {
          planLine = null;
        }
      }

      const contextLine = (() => {
        const usage = rt.tokenUsage;
        const ctx = usage?.modelContextWindow ?? null;
        const used = usage?.total?.totalTokens ?? null;
        if (!ctx || !used || ctx <= 0) return null;
        const remaining = Math.max(0, ctx - used);
        const remainingPct = Math.max(
          0,
          Math.min(100, Math.round((remaining / ctx) * 100)),
        );
        return `Context window: ${remainingPct}% left (${formatHumanCount(used)} used / ${formatHumanCount(ctx)})`;
      })();

      const limitLines = rateLimits
        ? formatRateLimitLines(rateLimits)
        : [];

      const text = [
        sessionLine,
        dirLine,
        accountLine,
        planLine,
        "",
        modelLine,
        contextLine,
        ...limitLines,
      ]
        .filter((v): v is string => typeof v === "string" && v.trim().length > 0)
        .join("\n");

      upsertBlock(rt, {
        id: newLocalId("status"),
        type: "info",
        title: "Status",
        text: "```text\n" + (text || "(no details)") + "\n```",
      });
      chatView?.refresh();
      schedulePersistRuntime(session.id);
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("codexMine.showSkills", async (args?: unknown) => {
      if (!backendManager) throw new Error("backendManager is not initialized");
      if (!sessions) throw new Error("sessions is not initialized");

      const session =
        parseSessionArg(args, sessions) ??
        (activeSessionId ? sessions.getById(activeSessionId) : null);
      if (!session) {
        void vscode.window.showErrorMessage("No session selected.");
        return;
      }

      let entries;
      try {
        entries = await backendManager.listSkillsForSession(session);
      } catch (err) {
        output.appendLine(`[skills] Failed to list skills: ${String(err)}`);
        void vscode.window.showErrorMessage("Failed to list skills.");
        return;
      }

      const entry = entries[0] ?? null;
      const skills = entry?.skills ?? [];
      const errors = entry?.errors ?? [];

      if (skills.length === 0) {
        const msg =
          errors.length > 0
            ? "No skills found (some skills failed to load)."
            : "No skills found. Enable [features].skills=true in $CODEX_HOME/config.toml.";
        void vscode.window.showInformationMessage(msg);
        return;
      }

      const picked = await vscode.window.showQuickPick(
        skills.map((s) => ({
          label: s.name,
          description: s.description,
          detail: `${s.scope} • ${s.path}`,
          skill: s,
        })),
        {
          title: "Codex UI: Skills",
          matchOnDescription: true,
          matchOnDetail: true,
        },
      );
      if (!picked) return;

      chatView?.insertIntoInput(`$${picked.skill.name} `);
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("codexMine.showAgents", async (args?: unknown) => {
      if (!sessions) throw new Error("sessions is not initialized");

      const session =
        parseSessionArg(args, sessions) ??
        (activeSessionId ? sessions.getById(activeSessionId) : null);
      if (!session) {
        void vscode.window.showErrorMessage("No session selected.");
        return;
      }

      const folder = resolveWorkspaceFolderForSession(session);
      if (!folder) {
        void vscode.window.showErrorMessage("WorkspaceFolder not found for session.");
        return;
      }

      await ensureBackendMatchesConfiguredCli(folder, "agents");

      const v = cliVariantByBackendKey.get(session.backendKey) ?? "unknown";
      if (v !== "codex-mine") {
        void vscode.window.showInformationMessage(
          "Agents are available only when running codex-mine. Click Settings (⚙) and select codex-mine, then restart the backend.",
        );
        return;
      }

      const { agents, errors, gitRoot } = await listAgentsFromDisk(folder.uri.fsPath);
      if (errors.length > 0) {
        output.appendLine(
          `[agents] scanned cwd=${folder.uri.fsPath} gitRoot=${gitRoot ?? "(none)"}`,
        );
        for (const e of errors) output.appendLine(`[agents] ${e}`);
      }

      if (agents.length === 0) {
        const msg =
          errors.length > 0
            ? "No agents found (some agent files failed to load)."
            : "No agents found. Add <git root>/.codex/agents/<name>.md or $CODEX_HOME/agents/<name>.md.";
        void vscode.window.showInformationMessage(msg);
        return;
      }

      const picked = await vscode.window.showQuickPick(
        agents.map((a) => ({
          label: a.name,
          description: a.description,
          detail: `${a.source} • ${a.path}`,
          agent: a,
        })),
        {
          title: "Codex UI: Agents",
          matchOnDescription: true,
          matchOnDetail: true,
        },
      );
      if (!picked) return;
      chatView?.insertIntoInput(`@${picked.agent.name} `);
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("codexMine.selectCliVariant", async () => {
      if (!backendManager) throw new Error("backendManager is not initialized");

      // Global default: do not prompt for a directory. The selected CLI becomes the default
      // for subsequent sessions/backends, and `New` will use it (auto-restarting if needed).
      const cfg = vscode.workspace.getConfiguration("codexMine");
      const mineCmd =
        cfg.get<string>("cli.commands.codexMine") ??
        cfg.get<string>("cli.commands.mine") ??
        "codex-mine";
      const codexCmd =
        cfg.get<string>("cli.commands.codex") ??
        cfg.get<string>("cli.commands.upstream") ??
        "codex";
      const current = normalizeCliVariant(cfg.get<string>("cli.variant") ?? "auto");

      const mineProbe = await probeCliVersion(mineCmd);
      const mineDetected = mineProbe.ok && mineProbe.version.includes("-mine.");

      const items: Array<{
        label: string;
        detail: string;
        variant: "auto" | "codex" | "codex-mine";
        disabledReason?: string;
      }> = [
        {
          label: "Auto",
          detail: "Use codexMine.backend.command (existing behavior)",
          variant: "auto",
        },
        {
          label: "codex",
          detail: `Command: ${codexCmd}`,
          variant: "codex",
        },
        {
          label: "codex-mine",
          detail: mineDetected
            ? `Command: ${mineCmd} (${mineProbe.version})`
            : mineProbe.ok
              ? `Command: ${mineCmd} (detected: ${mineProbe.version}, not a mine build)`
              : `Command: ${mineCmd} (not detected)`,
          variant: "codex-mine",
          disabledReason: mineDetected ? undefined : "codex-mine not detected",
        },
      ];

      const picked = await vscode.window.showQuickPick(
        items.map((it) => ({
          label: it.label + (it.variant === current ? " (current)" : ""),
          detail: it.detail,
          it,
        })),
        { title: "Codex UI: Select CLI" },
      );
      if (!picked) return;

      if (picked.it.variant === "codex-mine" && picked.it.disabledReason) {
        void vscode.window.showErrorMessage(picked.it.disabledReason);
        return;
      }

      await cfg.update("cli.variant", picked.it.variant, vscode.ConfigurationTarget.Global);

      const restart = await vscode.window.showInformationMessage(
        "CLI setting updated. Restart running backends now to apply?",
        "Restart all",
        "Later",
      );
      if (restart === "Restart all") {
        const folders = vscode.workspace.workspaceFolders ?? [];
        for (const f of folders) {
          await ensureBackendMatchesConfiguredCli(f, "newSession");
        }
      } else {
        void vscode.window.showInformationMessage(
          "Change will take effect the next time the backend starts.",
        );
      }
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "codexMine.sessionMenu",
      async (args?: unknown) => {
        if (!sessions) throw new Error("sessions is not initialized");
        const session = parseSessionArg(args, sessions);
        if (!session) {
          void vscode.window.showErrorMessage("Session not found.");
          return;
        }

        const picked = await vscode.window.showQuickPick(
          [
            { label: "Rename", action: "rename" as const },
            { label: "Close Tab (Hide)", action: "hide" as const },
          ],
          { title: session.title },
        );
        if (!picked) return;

        if (picked.action === "rename") {
          await vscode.commands.executeCommand("codexMine.renameSession", {
            sessionId: session.id,
          });
          return;
        }

        await vscode.commands.executeCommand("codexMine.hideSessionTab", {
          sessionId: session.id,
        });
      },
    ),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "codexMine.hideSessionTab",
      async (args?: unknown) => {
        if (!sessions) throw new Error("sessions is not initialized");
        const session = parseSessionArg(args, sessions);
        if (!session) {
          void vscode.window.showErrorMessage("Session not found.");
          return;
        }

        hiddenTabSessionIds.add(session.id);
        saveHiddenTabSessions(context);

        if (activeSessionId === session.id) {
          const visible = sessions
            .listAll()
            .filter((s) => !hiddenTabSessionIds.has(s.id));
          const next =
            visible.find((s) => s.backendKey === session.backendKey) ??
            visible[0] ??
            null;
          if (next) setActiveSession(next.id);
          else activeSessionId = null;
        }

        chatView?.refresh();
      },
    ),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "codexMine.closeSession",
      async (args?: unknown) => {
        if (!sessions) throw new Error("sessions is not initialized");
        if (!extensionContext) throw new Error("extensionContext is not set");

        const session = parseSessionArg(args, sessions);
        if (!session) {
          void vscode.window.showErrorMessage("Session not found.");
          return;
        }

        sessions.remove(session.id);
        saveSessions(extensionContext, sessions);

        runtimeBySessionId.delete(session.id);
        hiddenTabSessionIds.delete(session.id);
        saveHiddenTabSessions(extensionContext);

        if (activeSessionId === session.id) {
          const visible = sessions.listAll();
          const next =
            visible.find((s) => s.backendKey === session.backendKey) ??
            visible[0] ??
            null;
          if (next) setActiveSession(next.id);
          else activeSessionId = null;
        }

        sessionTree?.refresh();
        chatView?.refresh();
      },
    ),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "codexMine.sendMessage",
      async (args?: unknown) => {
        if (!backendManager)
          throw new Error("backendManager is not initialized");
        if (!sessions) throw new Error("sessions is not initialized");

        const sessionFromArgs = parseSessionArg(args, sessions);
        let session: Session;
        if (sessionFromArgs) {
          session = sessionFromArgs;
        } else {
          const folder = await pickWorkspaceFolder();
          if (!folder) return;
          session =
            (await backendManager.pickSession(folder)) ??
            (await backendManager.newSession(folder, getSessionModelState()));
        }

        setActiveSession(session.id);
        await showCodexMineViewContainer();
      },
    ),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "codexMine.openSession",
      async (args?: unknown) => {
        if (!backendManager)
          throw new Error("backendManager is not initialized");
        if (!sessions) throw new Error("sessions is not initialized");

        const session = parseSessionArg(args, sessions);
        if (!session) {
          void vscode.window.showErrorMessage("Session not found.");
          return;
      }

      const res = await backendManager.resumeSession(
        session,
      );
      void ensureModelsFetched(session);
      hydrateRuntimeFromThread(session.id, res.thread);
      setActiveSession(session.id);
      refreshCustomPromptsFromDisk();
      await showCodexMineViewContainer();
      },
    ),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "codexMine.openLatestDiff",
      async (args?: unknown) => {
        if (!backendManager)
          throw new Error("backendManager is not initialized");
        if (!sessions) throw new Error("sessions is not initialized");
        if (!diffProvider) throw new Error("diffProvider is not initialized");

        const session = parseSessionArg(args, sessions);
        if (!session) {
          void vscode.window.showErrorMessage("Session not found.");
          return;
        }

        const diff = backendManager.latestDiff(session);
        if (!diff) {
          void vscode.window.showInformationMessage("No diff available yet.");
          return;
        }

        const uri = makeDiffUri(session.id);
        diffProvider.set(uri, { title: session.title, diff });
        const doc = await vscode.workspace.openTextDocument(uri);
        await vscode.window.showTextDocument(doc, { preview: true });
      },
    ),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "codexMine.selectSession",
      async (args?: unknown) => {
        if (!backendManager)
          throw new Error("backendManager is not initialized");
        if (!sessions) throw new Error("sessions is not initialized");

        const session = parseSessionArg(args, sessions);
      if (!session) {
        void vscode.window.showErrorMessage("Session not found.");
        return;
      }

      const res = await backendManager.resumeSession(
        session,
      );
      void ensureModelsFetched(session);
      hydrateRuntimeFromThread(session.id, res.thread);
      setActiveSession(session.id);
      await showCodexMineViewContainer();
    },
    ),
  );

  // NOTE: Deliberately not implementing "archive session" in the VS Code extension.
  // Archiving moves sessions under ~/.codex/archived_sessions, which is unexpected here.

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "codexMine.renameSession",
      async (args?: unknown) => {
        if (!sessions) throw new Error("sessions is not initialized");

        const session = args ? parseSessionArg(args, sessions) : null;
        const active =
          session ??
          (activeSessionId ? sessions.getById(activeSessionId) : null);
        if (!active) {
          void vscode.window.showErrorMessage(
            "No session selected.",
          );
          return;
        }

        const next = await vscode.window.showInputBox({
          title: "Codex UI: Rename session",
          value: active.title,
          prompt: "Change the title shown in the chat tabs and Sessions list.",
          validateInput: (v) =>
            v.trim() ? null : "Title cannot be empty.",
        });
        if (next === undefined) return;

        sessions.rename(active.id, next.trim());
        saveSessions(context, sessions);
        sessionTree?.refresh();
        chatView?.refresh();
      },
    ),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "codexMine.respondApproval",
      async (args?: unknown) => {
        if (typeof args !== "object" || args === null) return;
        const o = args as Record<string, unknown>;
        const requestKey = o["requestKey"];
        const decision = o["decision"];
        if (typeof requestKey !== "string") return;
        if (
          decision !== "accept" &&
          decision !== "acceptForSession" &&
          decision !== "decline" &&
          decision !== "cancel"
        ) {
          return;
        }

        for (const rt of runtimeBySessionId.values()) {
          const resolver = rt.approvalResolvers.get(requestKey);
          if (!resolver) continue;
          rt.approvalResolvers.delete(requestKey);
          rt.pendingApprovals.delete(requestKey);
          chatView?.refresh();
          resolver(decision);
          break;
        }
      },
    ),
  );
}

export function deactivate(): void {
  backendManager?.dispose();
  backendManager = null;
  sessions = null;
  sessionTree = null;
  diffProvider = null;
  chatView = null;
  outputChannel = null;
  runtimeBySessionId.clear();
  activeSessionId = null;
}

async function pickWorkspaceFolder(): Promise<vscode.WorkspaceFolder | null> {
  const folders = vscode.workspace.workspaceFolders ?? [];
  if (folders.length === 0) {
    void vscode.window.showErrorMessage(
      "No workspace folder found. Open a folder and try again.",
    );
    return null;
  }
  if (folders.length === 1) return folders[0] ?? null;

  const picked = await vscode.window.showQuickPick(
    folders.map((f) => ({
      label: f.name,
      description: f.uri.fsPath,
      folder: f,
    })),
    { title: "Codex UI: Select a workspace folder" },
  );
  return picked?.folder ?? null;
}

function parseSessionArg(args: unknown, store: SessionStore): Session | null {
  if (typeof args !== "object" || args === null) return null;

  const rec = args as Record<string, unknown>;

  const sessionId = rec["sessionId"];
  if (typeof sessionId === "string") return store.getById(sessionId);

  // Tree view context: the element itself is passed as args.
  // See `ui/session_tree.ts` where nodes include `{ kind: "session", session: Session }`.
  const kind = rec["kind"];
  const session = rec["session"];
  if (kind === "session" && typeof session === "object" && session !== null) {
    const id = (session as Record<string, unknown>)["id"];
    if (typeof id === "string") return store.getById(id);
  }

  // Fallback for commands that might pass `{ session: { id } }` or `{ id }`.
  if (typeof session === "object" && session !== null) {
    const id = (session as Record<string, unknown>)["id"];
    if (typeof id === "string") return store.getById(id);
  }
  const id = rec["id"];
  if (typeof id === "string") return store.getById(id);

  return null;
}

type PromptExpansion =
  | { kind: "none" }
  | { kind: "expanded"; text: string }
  | { kind: "error"; message: string };

function parseSlashName(line: string): { name: string; rest: string } | null {
  const trimmed = line.trimStart();
  if (!trimmed.startsWith("/")) return null;
  const stripped = trimmed.slice(1);
  let nameEnd = stripped.length;
  for (let i = 0; i < stripped.length; i += 1) {
    if (/\s/.test(stripped[i] || "")) {
      nameEnd = i;
      break;
    }
  }
  const name = stripped.slice(0, nameEnd);
  if (!name) return null;
  const rest = stripped.slice(nameEnd).trimStart();
  return { name, rest };
}

function splitArgs(input: string): string[] {
  const out: string[] = [];
  const parts = shellParse(input);
  for (const part of parts) {
    if (typeof part === "string") {
      if (part) out.push(part);
      continue;
    }
    if (part && typeof part === "object" && "op" in part) {
      const op = (part as { op?: unknown }).op;
      if (typeof op === "string" && op) out.push(op);
      continue;
    }
    if (part != null) out.push(String(part));
  }
  return out;
}

function promptArgumentNames(content: string): string[] {
  const names: string[] = [];
  const seen = new Set<string>();
  const re = /\$[A-Z][A-Z0-9_]*/g;
  for (const match of content.matchAll(re)) {
    const idx = match.index ?? 0;
    if (idx > 0 && content[idx - 1] === "$") continue;
    const name = match[0]?.slice(1) ?? "";
    if (!name || name === "ARGUMENTS") continue;
    if (seen.has(name)) continue;
    seen.add(name);
    names.push(name);
  }
  return names;
}

function expandNumericPlaceholders(content: string, args: string[]): string {
  let out = "";
  let i = 0;
  let cachedArgs: string | null = null;
  while (i < content.length) {
    const off = content.indexOf("$", i);
    if (off === -1) {
      out += content.slice(i);
      break;
    }
    out += content.slice(i, off);
    const rest = content.slice(off);
    const b1 = rest[1];
    if (b1 === "$") {
      out += "$$";
      i = off + 2;
      continue;
    }
    if (b1 && b1 >= "1" && b1 <= "9") {
      const idx = b1.charCodeAt(0) - "1".charCodeAt(0);
      out += args[idx] ?? "";
      i = off + 2;
      continue;
    }
    if (rest.slice(1).startsWith("ARGUMENTS")) {
      if (args.length > 0) {
        if (!cachedArgs) cachedArgs = args.join(" ");
        out += cachedArgs;
      }
      i = off + 1 + "ARGUMENTS".length;
      continue;
    }
    out += "$";
    i = off + 1;
  }
  return out;
}

function expandCustomPromptIfAny(
  text: string,
  prompts: CustomPromptSummary[],
): PromptExpansion {
  const parsed = parseSlashName(text);
  if (!parsed) return { kind: "none" };
  const { name, rest } = parsed;
  const prefix = `${PROMPTS_CMD_PREFIX}:`;
  if (!name.startsWith(prefix)) return { kind: "none" };
  const promptName = name.slice(prefix.length);
  if (!promptName) return { kind: "none" };
  const prompt = prompts.find((p) => p.name === promptName);
  if (!prompt) return { kind: "none" };
  if (!prompt.content) {
    return {
      kind: "error",
      message: `Prompt '/${name}' is missing content.`,
    };
  }

  const required = promptArgumentNames(prompt.content);
  if (required.length > 0) {
    const inputs = new Map<string, string>();
    if (rest.trim()) {
      for (const token of splitArgs(rest)) {
        const eq = token.indexOf("=");
        if (eq < 0) {
          return {
            kind: "error",
            message:
              `Could not parse /${name}: expected key=value but found '${token}'. ` +
              "Wrap values in double quotes if they contain spaces.",
          };
        }
        const key = token.slice(0, eq);
        const value = token.slice(eq + 1);
        if (!key) {
          return {
            kind: "error",
            message: `Could not parse /${name}: expected a name before '=' in '${token}'.`,
          };
        }
        inputs.set(key, value);
      }
    }
    const missing = required.filter((k) => !inputs.has(k));
    if (missing.length > 0) {
      return {
        kind: "error",
        message:
          `Missing required args for /${name}: ${missing.join(", ")}. ` +
          "Provide as key=value (quote values with spaces).",
      };
    }
    const re = /\$[A-Z][A-Z0-9_]*/g;
    const replaced = prompt.content.replace(re, (match, offset) => {
      if (offset > 0 && prompt.content[offset - 1] === "$") return match;
      const key = match.slice(1);
      return inputs.get(key) ?? match;
    });
    return { kind: "expanded", text: replaced };
  }

  const posArgs = splitArgs(rest);
  const expanded = expandNumericPlaceholders(prompt.content, posArgs);
  return { kind: "expanded", text: expanded };
}

async function sendUserText(session: Session, text: string): Promise<void> {
  if (!backendManager) throw new Error("backendManager is not initialized");
  const rt = ensureRuntime(session.id);
  rt.sending = true;
  upsertBlock(session.id, { id: newLocalId("user"), type: "user", text });
  chatView?.refresh();
  schedulePersistRuntime(session.id);

  try {
    await backendManager.sendMessageWithModel(session, text, getSessionModelState());
  } catch (err) {
    rt.sending = false;
    upsertBlock(session.id, {
      id: newLocalId("error"),
      type: "error",
      title: "Send failed",
      text: String(err),
    });
    chatView?.refresh();
    schedulePersistRuntime(session.id);
    throw err;
  }
  schedulePersistRuntime(session.id);
}

async function handleSlashCommand(
  context: vscode.ExtensionContext,
  session: Session,
  text: string,
): Promise<boolean> {
  const trimmed = text.trim();
  if (!trimmed.startsWith("/")) return false;

  const [cmd, ...rest] = trimmed.slice(1).split(/\s+/);
  const arg = rest.join(" ").trim();

  const expandedPrompt = expandCustomPromptIfAny(trimmed, customPrompts);
  if (expandedPrompt.kind === "expanded") {
    await sendUserText(session, expandedPrompt.text);
    return true;
  }
  if (expandedPrompt.kind === "error") {
    const rt = ensureRuntime(session.id);
    upsertBlock(rt, {
      id: newLocalId("promptError"),
      type: "error",
      title: "Custom prompt error",
      text: expandedPrompt.message,
    });
    chatView?.refresh();
    schedulePersistRuntime(session.id);
    return true;
  }

  if (cmd === "new") {
    await vscode.commands.executeCommand("codexMine.newSession");
    return true;
  }
  if (cmd === "resume") {
    await vscode.commands.executeCommand("codexMine.resumeFromHistory");
    return true;
  }
  if (cmd === "diff") {
    await vscode.commands.executeCommand("codexMine.openLatestDiff", {
      sessionId: session.id,
    });
    return true;
  }
  if (cmd === "rename") {
    if (arg) {
      if (!sessions) throw new Error("sessions is not initialized");
      sessions.rename(session.id, arg);
      saveSessions(context, sessions);
      sessionTree?.refresh();
      chatView?.refresh();
      return true;
    }
    await vscode.commands.executeCommand("codexMine.renameSession", {
      sessionId: session.id,
    });
    return true;
  }
  if (cmd === "skills") {
    await vscode.commands.executeCommand("codexMine.showSkills", {
      sessionId: session.id,
    });
    return true;
  }
  if (cmd === "agents") {
    await vscode.commands.executeCommand("codexMine.showAgents", {
      sessionId: session.id,
    });
    return true;
  }
  if (cmd === "help") {
    const rt = ensureRuntime(session.id);
    const customList = customPrompts
      .map((p) => {
        const hint = p.argumentHint ? " " + p.argumentHint : "";
        return "- /prompts:" + p.name + hint;
      })
      .join("\n");
    upsertBlock(rt, {
      id: newLocalId("help"),
      type: "system",
      title: "Help",
      text: [
        "Slash commands:",
        "- /new: New session",
        "- /resume: Resume from history",
        "- /diff: Open Latest Diff",
        "- /rename <title>: Rename session",
        "- /skills: Browse skills",
        "- /agents: Browse agents (codex-mine)",
        "- /help: Show help",
        customList ? "\nCustom prompts:" : null,
        customList || null,
        "",
        "Mentions:",
        "- @selection: Insert selected file path + line range",
        "- @relative/path: Send file path (does not inline contents)",
        "- @file:relative/path: (legacy) Same as @relative/path",
      ]
        .filter(Boolean)
        .join("\n"),
    });
    chatView?.refresh();
    return true;
  }

  return false;
}

function formatThreadLabel(preview: string): string {
  const v = String(preview || "").trim();
  return v.length > 0 ? v : "(no preview)";
}

function formatThreadWhen(createdAtSec: number): string {
  const ms = Math.max(0, createdAtSec) * 1000;
  const d = new Date(ms);
  const pad2 = (n: number): string => String(n).padStart(2, "0");
  const yyyy = d.getFullYear();
  const mm = pad2(d.getMonth() + 1);
  const dd = pad2(d.getDate());
  const hh = pad2(d.getHours());
  const mi = pad2(d.getMinutes());
  return `${yyyy}-${mm}-${dd} ${hh}:${mi}`;
}

function normalizeFsPathForCompare(p: string): string {
  const resolved = path.resolve(p);
  // Windows: treat paths case-insensitively.
  return process.platform === "win32" ? resolved.toLowerCase() : resolved;
}

type ExpandMentionsResult =
  | { ok: true; text: string }
  | { ok: false; error: string };

async function expandMentions(
  session: Session,
  text: string,
): Promise<ExpandMentionsResult> {
  let out = text;

  if (out.includes("@selection")) {
    const editor = vscode.window.activeTextEditor ?? null;
    const sel = editor?.selection ?? null;
    const selected = sel ? (editor?.document.getText(sel) ?? "") : "";
    if (!selected.trim()) {
      return {
        ok: false,
        error: "@selection is empty (select a range first).",
      };
    }

    const folder = resolveWorkspaceFolderForSession(session);
    if (!folder) {
      return {
        ok: false,
        error:
          "Cannot expand @selection because no workspace folder is available.",
      };
    }

    const docUri = editor?.document?.uri ?? null;
    if (!docUri) {
      return {
        ok: false,
        error:
          "Cannot expand @selection because there is no active editor.",
      };
    }

    const folderFsPath = folder.uri.fsPath;
    const docFsPath = docUri.fsPath;
    let relPath = path.relative(folderFsPath, docFsPath);
    relPath = relPath.split(path.sep).join("/");
    if (!relPath || relPath.startsWith("../") || path.isAbsolute(relPath)) {
      return {
        ok: false,
        error: " file is outside the workspace.",
      };
    }

    const startLine = (sel?.start?.line ?? 0) + 1;
    let endLine = (sel?.end?.line ?? 0) + 1;
    const endChar = sel?.end?.character ?? 0;
    const endLine0 = sel?.end?.line ?? 0;
    const startLine0 = sel?.start?.line ?? 0;
    if (endChar === 0 && endLine0 > startLine0) endLine = endLine0;

    const range =
      startLine === endLine ? `#L${startLine}` : `#L${startLine}-L${endLine}`;
    const replacement = `@${relPath}${range}`;
    out = out.replaceAll("@selection", replacement);
  }

  // Support both "@relative/path" and legacy "@file:relative/path".
  // Match CLI-like rule: token must start at whitespace boundary.
  const fileRe = /(^|[\s])@([^\s]+)/g;
  const matches = [...out.matchAll(fileRe)];
  if (matches.length === 0) return { ok: true, text: out };

  const folder = resolveWorkspaceFolderForSession(session);
  if (!folder) {
    return {
      ok: false,
      error: "Cannot validate @ mentions because no workspace folder is available.",
    };
  }

  const unique = new Set<string>();
  for (const m of matches) {
    const raw = m[2];
    if (!raw) continue;
    if (raw === "selection") continue;

    const withoutFrag = raw.includes("#") ? (raw.split("#")[0] ?? "") : raw;
    const rel = withoutFrag.toLowerCase().startsWith("file:")
      ? withoutFrag.slice("file:".length)
      : withoutFrag;
    if (rel) unique.add(rel);
  }
  if (unique.size === 0) return { ok: true, text: out };

  for (const rel of unique) {
    if (rel.startsWith("/") || rel.includes(":")) {
      return {
        ok: false,
        error: `@ mentions only support relative paths: ${rel}`,
      };
    }

    const uri = vscode.Uri.joinPath(folder.uri, rel);
    try {
      const stat = await vscode.workspace.fs.stat(uri);
      if ((stat.type & vscode.FileType.File) === 0) {
        return {
          ok: false,
          error: `@ mentions only support files: ${rel}`,
        };
      }
    } catch (err) {
      return {
        ok: false,
        error: `Failed to resolve @ mention: ${rel} (${String(err)})`,
      };
    }
  }

  // NOTE: @ mentions send file paths only. Do not expand file contents here.
  return { ok: true, text: out };
}

function resolveWorkspaceFolderForSession(
  session: Session,
): vscode.WorkspaceFolder | null {
  const uri = vscode.Uri.parse(session.workspaceFolderUri);
  return vscode.workspace.getWorkspaceFolder(uri) ?? null;
}

function formatAsAttachment(
  label: string,
  content: string,
  path: string | null,
): string {
  const lang = path ? languageFromPath(path) : "";
  const fence = lang ? `\`\`\`${lang}` : "```";
  return `\n\n[attachment:${label}]\n${fence}\n${content}\n\`\`\`\n`;
}

function languageFromPath(path: string): string {
  const lower = path.toLowerCase();
  if (lower.endsWith(".ts")) return "ts";
  if (lower.endsWith(".tsx")) return "tsx";
  if (lower.endsWith(".js")) return "js";
  if (lower.endsWith(".jsx")) return "jsx";
  if (lower.endsWith(".json")) return "json";
  if (lower.endsWith(".rs")) return "rust";
  if (lower.endsWith(".md")) return "md";
  if (lower.endsWith(".yml") || lower.endsWith(".yaml")) return "yaml";
  if (lower.endsWith(".toml")) return "toml";
  if (lower.endsWith(".sh")) return "sh";
  if (lower.endsWith(".py")) return "py";
  return "";
}

function formatCommandActions(actions: CommandAction[]): string | null {
  const lines: string[] = [];
  for (const a of actions) {
    if (!a) continue;
    if (a.type === "read") {
      lines.push(`read: ${a.path}`);
      continue;
    }
    if (a.type === "listFiles") {
      lines.push(`listFiles: ${a.path ?? "."}`);
      continue;
    }
    if (a.type === "search") {
      const q = a.query ? JSON.stringify(a.query) : "(unknown)";
      lines.push(`search: ${q} in ${a.path ?? "."}`);
      continue;
    }
    if (a.type === "unknown") {
      // Keep unknown terse; command string might be long.
      lines.push("action: unknown");
      continue;
    }
    lines.push("action: unknown");
  }
  const text = lines.join("\n").trim();
  return text ? text : null;
}

async function showCodexMineViewContainer(): Promise<void> {
  await vscode.commands.executeCommand("workbench.view.extension.codexMine");
}

function setActiveSession(sessionId: string): void {
  activeSessionId = sessionId;
  ensureRuntime(sessionId);
  // If a hidden tab session is selected (e.g. via Sessions tree), show it again.
  if (hiddenTabSessionIds.delete(sessionId)) {
    if (extensionContext) saveHiddenTabSessions(extensionContext);
  }
  const s = sessions ? sessions.getById(sessionId) : null;
  if (s) void ensureModelsFetched(s);
  chatView?.refresh();
}

function loadHiddenTabSessions(context: vscode.ExtensionContext): void {
  const raw = context.workspaceState.get<unknown>(HIDDEN_TAB_SESSIONS_KEY);
  if (!Array.isArray(raw)) return;
  for (const v of raw) {
    if (typeof v === "string" && v) hiddenTabSessionIds.add(v);
  }
}

function saveHiddenTabSessions(context: vscode.ExtensionContext): void {
  void context.workspaceState.update(HIDDEN_TAB_SESSIONS_KEY, [
    ...hiddenTabSessionIds,
  ]);
}

function setCustomPrompts(next: CustomPromptSummary[]): void {
  customPrompts = next;
  chatView?.refresh();
}

async function loadInitialModelState(
  output: vscode.OutputChannel,
): Promise<void> {
  const fromHome = await readModelStateFromCodexHomeConfig(output);
  const picked = fromHome;
  if (!picked) {
    output.appendLine(
      "[config] config.toml not found in CODEX_HOME; using defaults",
    );
    return;
  }
  setSessionModelState(picked.state);
  output.appendLine(`[config] Loaded model settings from ${picked.path}`);
  chatView?.refresh();
}

async function readModelStateFromCodexHomeConfig(
  output: vscode.OutputChannel,
): Promise<{ state: ModelState; path: string } | null> {
  const candidate = path.join(resolveCodexHome(), "config.toml");
  const loaded = await readModelStateFromConfig(candidate, output);
  return loaded ? { state: loaded, path: candidate } : null;
}

async function readModelStateFromConfig(
  filePath: string,
  output: vscode.OutputChannel,
): Promise<ModelState | null> {
  try {
    const raw = await fs.readFile(filePath, "utf8");
    const parsed = parseToml(raw) as Record<string, unknown>;
    const model = pickString(parsed["model"]);
    const provider = pickString(parsed["model_provider"]);
    const reasoning = pickString(parsed["model_reasoning_effort"]);
    if (!model && !provider && !reasoning) return null;
    return { model, provider, reasoning };
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") return null;
    output.appendLine(
      `[config] Failed to read ${filePath}: ${String((err as Error).message)}`,
    );
    return null;
  }
}

function pickString(v: unknown): string | null {
  return typeof v === "string" && v.trim() ? v.trim() : null;
}

function formatHumanCount(n: number): string {
  if (!Number.isFinite(n)) return String(n);
  if (n >= 1_000_000) return `${Math.round(n / 100_000) / 10}M`;
  if (n >= 1_000) return `${Math.round(n / 100) / 10}K`;
  return String(n);
}

function formatRateLimitLines(rateLimits: RateLimitSnapshot): string[] {
  const lines: string[] = [];
  if (rateLimits.primary) {
    lines.push(formatRateLimitLine("Primary", rateLimits.primary));
  }
  if (rateLimits.secondary) {
    lines.push(formatRateLimitLine("Secondary", rateLimits.secondary));
  }
  return lines.filter(Boolean);
}

function formatRateLimitLine(labelFallback: string, w: RateLimitWindow): string {
  const mins = w.windowDurationMins ?? null;
  const label = mins ? rateLimitLabelFromMinutes(mins) : labelFallback;
  const used = Math.max(0, Math.min(100, w.usedPercent));
  const remaining = Math.max(0, Math.min(100, 100 - used));
  const bar = formatBar(remaining, 20);
  const reset = w.resetsAt ? formatResetsAt(w.resetsAt) : null;
  const resetText = reset ? ` (resets ${reset})` : "";
  return `${label}: [${bar}] ${remaining}% left${resetText}`;
}

function rateLimitLabelFromMinutes(mins: number): string {
  if (mins === 300) return "5h limit";
  if (mins === 10080) return "Weekly limit";
  if (mins === 1440) return "Daily limit";
  if (mins % 60 === 0) return `${mins / 60}h limit`;
  return `${mins}m limit`;
}

function formatBar(remainingPercent: number, width: number): string {
  const pct = Math.max(0, Math.min(100, remainingPercent));
  const filled = Math.max(0, Math.min(width, Math.round((pct / 100) * width)));
  return "█".repeat(filled) + "░".repeat(Math.max(0, width - filled));
}

function formatResetsAt(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000);
  const now = new Date();
  const isSameDay =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate();
  const pad2 = (n: number): string => String(n).padStart(2, "0");
  const hhmm = `${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
  if (isSameDay) return hhmm;
  return `${pad2(d.getMonth() + 1)}/${pad2(d.getDate())} ${hhmm}`;
}

function resolveCodexHome(): string {
  const env = process.env["CODEX_HOME"];
  if (env && env.trim()) return env.trim();
  return path.join(os.homedir(), ".codex");
}

function inferCliVariantFromCliVersion(
  cliVersion: string | null,
): "unknown" | "codex" | "codex-mine" {
  if (!cliVersion) return "unknown";
  return cliVersion.includes("-mine.") ? "codex-mine" : "codex";
}

function backendKeyForCwd(cwd: string | null): string | null {
  if (!cwd) return null;
  const folders = vscode.workspace.workspaceFolders ?? [];
  const target = path.resolve(cwd);
  for (const f of folders) {
    const fsPath = f.uri.fsPath;
    if (!fsPath) continue;
    if (path.resolve(fsPath) === target) return f.uri.toString();
  }
  return null;
}

function normalizeCliVariant(raw: string | null): "auto" | "codex" | "codex-mine" {
  const v = (raw ?? "auto").trim();
  if (v === "mine") return "codex-mine";
  if (v === "upstream") return "codex";
  if (v === "codex" || v === "codex-mine" || v === "auto") return v;
  return "auto";
}

function desiredCliCommandFromConfig(cfg: vscode.WorkspaceConfiguration): {
  variant: "auto" | "codex" | "codex-mine";
  command: string | null;
} {
  const variant = normalizeCliVariant(cfg.get<string>("cli.variant") ?? "auto");
  const codexCmd =
    cfg.get<string>("cli.commands.codex") ??
    cfg.get<string>("cli.commands.upstream") ??
    "codex";
  const mineCmd =
    cfg.get<string>("cli.commands.codexMine") ??
    cfg.get<string>("cli.commands.mine") ??
    "codex-mine";
  const backendCmd = cfg.get<string>("backend.command") ?? null;

  if (variant === "codex") return { variant, command: codexCmd };
  if (variant === "codex-mine") return { variant, command: mineCmd };
  return { variant, command: backendCmd };
}

async function ensureBackendMatchesConfiguredCli(
  folder: vscode.WorkspaceFolder,
  reason: "newSession" | "agents",
): Promise<void> {
  if (!backendManager) throw new Error("backendManager is not initialized");
  if (!outputChannel) throw new Error("outputChannel is not initialized");

  const cfg = vscode.workspace.getConfiguration("codexMine", folder.uri);
  const desired = desiredCliCommandFromConfig(cfg);
  if (desired.variant === "auto" || !desired.command) return;

  const running = backendManager.getRunningCommand(folder);
  if (!running) {
    cliVariantByBackendKey.set(
      folder.uri.toString(),
      desired.variant === "codex-mine" ? "codex-mine" : "codex",
    );
    return;
  }
  if (running === desired.command) return;

  outputChannel.appendLine(
    `[cli] Restarting backend to match cli.variant=${desired.variant} (reason=${reason}) running=${running} desired=${desired.command}`,
  );
  await backendManager.restartForWorkspaceFolder(folder);
  cliVariantByBackendKey.set(
    folder.uri.toString(),
    desired.variant === "codex-mine" ? "codex-mine" : "codex",
  );
  void vscode.window.showInformationMessage(
    `Backend restarted to use ${desired.variant}.`,
  );
}

async function probeCliVersion(command: string): Promise<
  | { ok: true; version: string }
  | { ok: false; error: string }
> {
  return await new Promise((resolve) => {
    let stdout = "";
    let stderr = "";
    const child = spawn(command, ["--version"], {
      stdio: ["ignore", "pipe", "pipe"],
      env: process.env,
    });
    child.stdout?.on("data", (buf) => {
      stdout += String(buf);
    });
    child.stderr?.on("data", (buf) => {
      stderr += String(buf);
    });
    child.on("error", (err) => {
      resolve({ ok: false, error: String((err as Error).message ?? err) });
    });
    child.on("close", (code) => {
      const out = (stdout || stderr || "").trim();
      if (code !== 0) {
        resolve({
          ok: false,
          error: out || `exit code ${String(code)}`,
        });
        return;
      }
      if (!out) {
        resolve({ ok: false, error: "empty version output" });
        return;
      }
      resolve({ ok: true, version: out.split(/\r?\n/)[0] ?? out });
    });
  });
}

// Agents are read from disk only when running codex-mine.

function parsePromptFrontmatter(content: string): {
  description: string | null;
  argumentHint: string | null;
  body: string;
} {
  const lines = content.split(/\r?\n/);
  if ((lines[0] ?? "").trim() !== "---") {
    return { description: null, argumentHint: null, body: content };
  }

  let desc: string | null = null;
  let hint: string | null = null;
  let i = 1;
  for (; i < lines.length; i += 1) {
    const raw = lines[i] ?? "";
    const trimmed = raw.trim();
    if (trimmed === "---") {
      i += 1;
      break;
    }
    if (!trimmed || trimmed.startsWith("#")) continue;
    const idx = trimmed.indexOf(":");
    if (idx <= 0) continue;
    const key = trimmed.slice(0, idx).trim().toLowerCase();
    let val = trimmed.slice(idx + 1).trim();
    if (val.length >= 2) {
      const first = val[0];
      const last = val[val.length - 1];
      if ((first === "\"" && last === "\"") || (first === "'" && last === "'")) {
        val = val.slice(1, -1);
      }
    }
    if (key === "description") desc = val;
    if (key === "argument-hint" || key === "argument_hint") hint = val;
  }

  if (i <= 1 || i > lines.length) {
    return { description: null, argumentHint: null, body: content };
  }

  const body = lines.slice(i).join("\n");
  return { description: desc, argumentHint: hint, body };
}

async function loadCustomPromptsFromDisk(): Promise<CustomPromptSummary[]> {
  const dir = path.join(resolveCodexHome(), "prompts");
  try {
    const entries = await fs.readdir(dir, { withFileTypes: true });
    const out: CustomPromptSummary[] = [];
    for (const entry of entries) {
      if (!entry.isFile()) continue;
      const ext = path.extname(entry.name);
      if (!ext || ext.toLowerCase() !== ".md") continue;
      const name = path.parse(entry.name).name.trim();
      if (!name) continue;
      const fullPath = path.join(dir, entry.name);
      const content = await fs.readFile(fullPath, "utf8").catch(() => null);
      if (content === null) continue;
      const parsed = parsePromptFrontmatter(content);
      out.push({
        name,
        description: parsed.description,
        argumentHint: parsed.argumentHint,
        content: parsed.body,
        source: "disk",
      });
    }
    out.sort((a, b) => a.name.localeCompare(b.name));
    return out;
  } catch {
    return [];
  }
}

function refreshCustomPromptsFromDisk(): void {
  void loadCustomPromptsFromDisk()
    .then((next) => {
      if (customPrompts.some((p) => p.source === "server")) return;
      setCustomPrompts(next);
    })
    .catch(() => {});
}

function ensureRuntime(sessionId: string): SessionRuntime {
  const existing = runtimeBySessionId.get(sessionId);
  if (existing) return existing;
  const rt: SessionRuntime = {
    blocks: [],
    latestDiff: null,
    statusText: null,
    tokenUsage: null,
    sending: false,
    activeTurnId: null,
    lastTurnStartedAtMs: null,
    lastTurnCompletedAtMs: null,
    blockIndexById: new Map(),
    legacyPatchTargetByCallId: new Map(),
    legacyWebSearchTargetByCallId: new Map(),
    pendingApprovals: new Map(),
    approvalResolvers: new Map(),
  };
  runtimeBySessionId.set(sessionId, rt);
  return rt;
}

function getModelOptionsForSession(session: Session | null): Model[] | null {
  if (!session || !backendManager) return null;
  return backendManager.getCachedModels(session);
}

async function ensureModelsFetched(session: Session): Promise<void> {
  if (!backendManager) return;
  const backendKey = session.backendKey;
  if (backendManager.getCachedModels(session)) return;
  const pending = pendingModelFetchByBackend.get(backendKey);
  if (pending) {
    await pending;
    return;
  }
  const promise = backendManager
    .listModelsForSession(session)
    .then(() => chatView?.refresh())
    .catch((err) => {
      outputChannel?.appendLine(
        `[models] Failed to list models: ${String((err as Error).message ?? err)}`,
      );
    })
    .finally(() => pendingModelFetchByBackend.delete(backendKey));
  pendingModelFetchByBackend.set(backendKey, promise);
  await promise;
}

function buildChatState(): ChatViewState {
  const promptSummaries = customPrompts.map((p) => ({
    name: p.name,
    description: p.description,
    argumentHint: p.argumentHint,
    source: p.source,
  }));
  const capsForBackendKey = (backendKey: string | null): {
    agents: boolean;
    cliVariant: "unknown" | "codex" | "codex-mine";
  } => {
    const detected =
      backendKey ? cliVariantByBackendKey.get(backendKey) ?? "unknown" : "unknown";
    if (detected !== "unknown") {
      return { agents: detected === "codex-mine", cliVariant: detected };
    }
    if (!backendKey) return { agents: false, cliVariant: "unknown" };

    // No detected runtime variant yet (e.g. backend not started). Use config as a hint.
    const folderUri = vscode.Uri.parse(backendKey);
    const cfg = vscode.workspace.getConfiguration("codexMine", folderUri);
    const raw = cfg.get<string>("cli.variant") ?? "auto";
    const normalized =
      raw === "mine"
        ? "codex-mine"
        : raw === "upstream"
          ? "codex"
          : raw;
    if (normalized === "codex-mine")
      return { agents: true, cliVariant: "codex-mine" };
    if (normalized === "codex") return { agents: false, cliVariant: "codex" };
    return { agents: false, cliVariant: "unknown" };
  };
  if (!sessions)
    return {
      globalBlocks: globalRuntime.blocks,
      capabilities: capsForBackendKey(null),
      sessions: [],
      activeSession: null,
      blocks: [],
      latestDiff: null,
      sending: false,
      statusText: globalStatusText,
      modelState: getSessionModelState(),
      models: null,
      approvals: [],
      customPrompts: promptSummaries,
    };

  const tabSessionsRaw = sessions
    .listAll()
    .filter((s) => !hiddenTabSessionIds.has(s.id));
  const activeRaw = activeSessionId ? sessions.getById(activeSessionId) : null;
  if (!activeRaw)
    return {
      globalBlocks: globalRuntime.blocks,
      capabilities: capsForBackendKey(null),
      sessions: tabSessionsRaw,
      activeSession: null,
      blocks: [],
      latestDiff: null,
      sending: false,
      statusText: globalStatusText,
      modelState: getSessionModelState(),
      approvals: [],
      customPrompts: promptSummaries,
    };

  const rt = ensureRuntime(activeRaw.id);
  const baseStatusText = rt.statusText ?? null;
  const suffix: string[] = [];
  if (rt.sending) suffix.push("sending…");
  const worked = computeWorkedSeconds(rt);
  if (worked !== null) suffix.push(`worked=${worked}s`);
  if (rt.pendingApprovals.size > 0)
    suffix.push(`approvals=${rt.pendingApprovals.size}`);
  const statusText =
    baseStatusText && suffix.length > 0
      ? `${baseStatusText} • ${suffix.join(" • ")}`
      : baseStatusText || (suffix.length > 0 ? suffix.join(" • ") : null);
  return {
    globalBlocks: globalRuntime.blocks,
    capabilities: capsForBackendKey(activeRaw.backendKey),
    sessions: tabSessionsRaw,
    activeSession: activeRaw,
    blocks: rt.blocks,
    latestDiff: rt.latestDiff,
    sending: rt.sending,
    statusText: statusText ?? globalStatusText,
    modelState: getSessionModelState(),
    models: getModelOptionsForSession(activeRaw),
    approvals: [...rt.pendingApprovals.entries()].map(([requestKey, v]) => ({
      requestKey,
      title: v.title,
      detail: v.detail,
      canAcceptForSession: v.canAcceptForSession,
    })),
    customPrompts: promptSummaries,
  };
}

function normalizeSessionTitle(title: string): string {
  const trimmed = title.trim();
  if (!trimmed) return "(untitled)";
  const withoutNumber = trimmed.replace(/\s+#\d+$/, "").trim();
  const withoutShortId = withoutNumber
    .replace(/\s+\([0-9a-f]{8}\)$/i, "")
    .trim();
  return withoutShortId || "(untitled)";
}

function applyServerNotification(
  sessionId: string,
  n: AnyServerNotification,
): void {
  const rt = ensureRuntime(sessionId);
  schedulePersistRuntime(sessionId);
  switch (n.method) {
    case "rawResponseItem/completed":
      // Internal-only (Codex Cloud). Avoid flooding "Other events (debug)".
      return;
    case "thread/started":
      return;
    case "deprecationNotice": {
      const p = (n as any).params as { summary?: unknown; details?: unknown };
      const summary = String(p?.summary ?? "").trim();
      const details =
        typeof p?.details === "string" ? String(p.details).trim() : "";
      const id = deprecationNoticeId(summary, details);
      upsertGlobal({
        id,
        type: "info",
        title: "Deprecation notice",
        text: details ? `${summary}\n\n${details}` : summary,
      });
      chatView?.refresh();
      return;
    }
    case "thread/compacted": {
      const turnId = String((n as any).params?.turnId ?? "");
      const workedSeconds = computeWorkedSeconds(rt);
      const headline =
        workedSeconds !== null ? `Worked for ${workedSeconds}s` : "Context";
      const line = makeDividerLine(headline);
      upsertBlock(rt, {
        id: `compacted:${turnId || Date.now()}`,
        type: "divider",
        text: `${line}\n• Context compacted`,
      });
      chatView?.refresh();
      return;
    }
    case "turn/started":
      rt.sending = true;
      rt.lastTurnStartedAtMs = Date.now();
      rt.lastTurnCompletedAtMs = null;
      rt.activeTurnId = String((n as any).params?.turn?.id ?? "") || null;
      chatView?.refresh();
      return;
    case "turn/completed":
      rt.sending = false;
      rt.lastTurnCompletedAtMs = Date.now();
      rt.activeTurnId = null;
      chatView?.refresh();
      return;
    case "thread/tokenUsage/updated":
      rt.tokenUsage = (n as any).params.tokenUsage as ThreadTokenUsage;
      rt.statusText = formatTokenUsageStatus(rt.tokenUsage);
      chatView?.refresh();
      return;
    case "item/agentMessage/delta": {
      const id = (n as any).params.itemId as string;
      const block = getOrCreateBlock(rt, id, () => ({
        id,
        type: "assistant",
        text: "",
      }));
      if (block.type === "assistant")
        block.text += (n as any).params.delta as string;
      chatView?.refresh();
      return;
    }
    case "item/reasoning/summaryTextDelta": {
      const id = (n as any).params.itemId as string;
      const block = getOrCreateBlock(rt, id, () => ({
        id,
        type: "reasoning",
        summaryParts: [],
        rawParts: [],
        status: "inProgress",
      }));
      if (block.type === "reasoning") {
        const p = (n as any).params as { summaryIndex: number; delta: string };
        ensureParts(block.summaryParts, p.summaryIndex);
        block.summaryParts[p.summaryIndex] += p.delta;
      }
      chatView?.refresh();
      return;
    }
    case "item/reasoning/summaryPartAdded": {
      const id = (n as any).params.itemId as string;
      const block = getOrCreateBlock(rt, id, () => ({
        id,
        type: "reasoning",
        summaryParts: [],
        rawParts: [],
        status: "inProgress",
      }));
      if (block.type === "reasoning") {
        ensureParts(
          block.summaryParts,
          (n as any).params.summaryIndex as number,
        );
      }
      chatView?.refresh();
      return;
    }
    case "item/reasoning/textDelta": {
      const id = (n as any).params.itemId as string;
      const block = getOrCreateBlock(rt, id, () => ({
        id,
        type: "reasoning",
        summaryParts: [],
        rawParts: [],
        status: "inProgress",
      }));
      if (block.type === "reasoning") {
        const p = (n as any).params as { contentIndex: number; delta: string };
        ensureParts(block.rawParts, p.contentIndex);
        block.rawParts[p.contentIndex] += p.delta;
      }
      chatView?.refresh();
      return;
    }
    case "item/commandExecution/outputDelta": {
      const id = (n as any).params.itemId as string;
      const block = getOrCreateBlock(rt, id, () => ({
        id,
        type: "command",
        title: "Command",
        status: "inProgress",
        command: "",
        cwd: null,
        exitCode: null,
        durationMs: null,
        terminalStdin: [],
        output: "",
      }));
      if (block.type === "command")
        block.output += (n as any).params.delta as string;
      chatView?.refresh();
      return;
    }
    case "item/commandExecution/terminalInteraction": {
      const id = (n as any).params.itemId as string;
      const block = getOrCreateBlock(rt, id, () => ({
        id,
        type: "command",
        title: "Command",
        status: "inProgress",
        command: "",
        cwd: null,
        exitCode: null,
        durationMs: null,
        terminalStdin: [],
        output: "",
      }));
      if (block.type === "command")
        block.terminalStdin.push((n as any).params.stdin as string);
      chatView?.refresh();
      return;
    }
    case "item/fileChange/outputDelta": {
      const id = (n as any).params.itemId as string;
      const block = getOrCreateBlock(rt, id, () => ({
        id,
        type: "fileChange",
        title: "Changes",
        status: "inProgress",
        files: [],
        detail: "",
        hasDiff: rt.latestDiff != null,
        diffs: [],
      }));
      if (block.type === "fileChange")
        block.detail += (n as any).params.delta as string;
      if (block.type === "fileChange")
        block.diffs = diffsForFiles(block.files, rt.latestDiff);
      chatView?.refresh();
      return;
    }
    case "item/mcpToolCall/progress": {
      const id = (n as any).params.itemId as string;
      const block = getOrCreateBlock(rt, id, () => ({
        id,
        type: "mcp",
        title: "MCP Tool",
        status: "inProgress",
        server: "",
        tool: "",
        detail: "",
      }));
      if (block.type === "mcp")
        block.detail += `${String((n as any).params.message ?? "")}\n`;
      chatView?.refresh();
      return;
    }
    case "turn/plan/updated": {
      const p = (n as any).params as {
        turnId: string;
        plan: Array<{ status: string; step: string }>;
        explanation: string | null;
      };
      const id = `plan:${p.turnId}`;
      const steps = p.plan
        .map((p) => `${formatPlanStatus(p.status)} ${p.step}`)
        .join("\n");
      const text = p.explanation ? `${p.explanation}\n${steps}` : steps;
      upsertBlock(sessionId, { id, type: "plan", title: "Plan", text });
      chatView?.refresh();
      return;
    }
    case "turn/diff/updated": {
      rt.latestDiff = (n as any).params.diff as string;
      // Mark existing fileChange blocks as having a diff.
      for (const b of rt.blocks) {
        if (b.type === "fileChange") {
          b.hasDiff = true;
          b.diffs = diffsForFiles(b.files, rt.latestDiff);
        }
      }
      chatView?.refresh();
      return;
    }
    case "error": {
      upsertBlock(sessionId, {
        id: newLocalId("error"),
        type: "error",
        title: "Error",
        text: String((n as any).params?.error?.message ?? ""),
      });
      chatView?.refresh();
      return;
    }
    case "item/started":
    case "item/completed": {
      applyItemLifecycle(
        rt,
        sessionId,
        String((n as any).params.threadId ?? ""),
        (n as any).params.item as ThreadItem,
        n.method === "item/completed",
      );
      chatView?.refresh();
      return;
    }
    default:
      if (n.method.startsWith("codex/event/")) {
        applyCodexEvent(rt, sessionId, n.method, (n as any).params);
        chatView?.refresh();
        return;
      }

      appendUnhandledEvent(
        rt,
        `Unhandled event: ${n.method}`,
        (n as any).params,
      );
      chatView?.refresh();
      return;
  }
}

function applyItemLifecycle(
  rt: SessionRuntime,
  sessionId: string,
  threadId: string,
  item: ThreadItem,
  completed: boolean,
): void {
  const statusText = completed ? "completed" : "started";
  switch (item.type) {
    case "reasoning": {
      const block = getOrCreateBlock(rt, item.id, () => ({
        id: item.id,
        type: "reasoning",
        summaryParts: [...item.summary],
        rawParts: [...item.content],
        status: completed ? "completed" : "inProgress",
      }));
      if (block.type === "reasoning") {
        block.status = completed ? "completed" : "inProgress";
        if (completed) {
          block.summaryParts = [...item.summary];
          block.rawParts = [...item.content];
        }
      }
      break;
    }
    case "commandExecution": {
      const block = getOrCreateBlock(rt, item.id, () => ({
        id: item.id,
        type: "command",
        title: "Command",
        status: item.status,
        command: item.command,
        actionsText: formatCommandActions(item.commandActions),
        cwd: item.cwd ?? null,
        exitCode: item.exitCode,
        durationMs: item.durationMs,
        terminalStdin: [],
        output: item.aggregatedOutput ?? "",
      }));
      if (block.type === "command") {
        block.status = item.status;
        block.command = item.command;
        block.actionsText = formatCommandActions(item.commandActions);
        block.cwd = item.cwd ?? null;
        block.exitCode = item.exitCode;
        block.durationMs = item.durationMs;
        if (completed && item.aggregatedOutput)
          block.output = item.aggregatedOutput;
      }
      break;
    }
    case "fileChange": {
      const workspaceFolderFsPath = (() => {
        const s = sessions?.getById(sessionId);
        if (!s) return null;
        try {
          return vscode.Uri.parse(s.workspaceFolderUri).fsPath;
        } catch {
          return null;
        }
      })();
      const files = item.changes.map((c) =>
        formatPathForSession(c.path, workspaceFolderFsPath),
      );
      const block = getOrCreateBlock(rt, item.id, () => ({
        id: item.id,
        type: "fileChange",
        title: "Changes",
        status: item.status,
        files,
        detail: "",
        hasDiff: true,
        diffs: diffsForFiles(files, rt.latestDiff),
      }));
      if (block.type === "fileChange") {
        block.status = item.status;
        block.files = files;
        block.hasDiff = true;
        block.diffs = diffsForFiles(files, rt.latestDiff);
      }
      break;
    }
    case "mcpToolCall": {
      const block = getOrCreateBlock(rt, item.id, () => ({
        id: item.id,
        type: "mcp",
        title: "MCP Tool",
        status: item.status,
        server: item.server,
        tool: item.tool,
        detail: "",
      }));
      if (block.type === "mcp") {
        block.status = item.status;
        block.server = item.server;
        block.tool = item.tool;
        if (completed && item.result)
          block.detail += `\nresult: ${JSON.stringify(item.result)}\n`;
        if (completed && item.error)
          block.detail += `\nerror: ${JSON.stringify(item.error)}\n`;
      }
      break;
    }
    case "webSearch": {
      // If a legacy web_search_* already produced a webSearch card for the same query,
      // prefer v2 and drop the legacy one to avoid duplicates.
      const legacyIdsToDrop: string[] = [];
      for (const b of rt.blocks) {
        if (!b || b.type !== "webSearch") continue;
        const id = String(b.id || "");
        if (!id.startsWith("legacyWebSearch:")) continue;
        if (b.query.trim() !== item.query.trim()) continue;
        legacyIdsToDrop.push(id);
      }
      if (legacyIdsToDrop.length > 0) {
        for (const legacyId of legacyIdsToDrop) {
          const idx = rt.blockIndexById.get(legacyId);
          if (idx === undefined) continue;
          rt.blocks.splice(idx, 1);
          rt.blockIndexById.clear();
          for (let i = 0; i < rt.blocks.length; i++) {
            rt.blockIndexById.set(rt.blocks[i]!.id, i);
          }
          for (const [k, v] of rt.legacyWebSearchTargetByCallId.entries()) {
            if (v === legacyId) rt.legacyWebSearchTargetByCallId.delete(k);
          }
        }
      }

      upsertBlock(rt, {
        id: item.id,
        type: "webSearch",
        query: item.query,
        status: completed ? "completed" : "inProgress",
      });
      break;
    }
    case "imageView": {
      upsertBlock(rt, {
        id: item.id,
        type: "system",
        title: `Image view (${statusText})`,
        text: item.path,
      });
      break;
    }
    case "enteredReviewMode": {
      upsertBlock(rt, {
        id: item.id,
        type: "system",
        title: `Entered review mode (${statusText})`,
        text: item.review,
      });
      break;
    }
    case "exitedReviewMode": {
      upsertBlock(rt, {
        id: item.id,
        type: "system",
        title: `Exited review mode (${statusText})`,
        text: item.review,
      });
      break;
    }
    default:
      // Hide userMessage/agentMessage lifecycle; handled elsewhere.
      break;
  }
}

function upsertBlock(
  sessionIdOrRt: string | SessionRuntime,
  block: ChatBlock,
): void {
  const rt =
    typeof sessionIdOrRt === "string"
      ? ensureRuntime(sessionIdOrRt)
      : sessionIdOrRt;
  const idx = rt.blockIndexById.get(block.id);
  if (idx === undefined) {
    rt.blockIndexById.set(block.id, rt.blocks.length);
    rt.blocks.push(block);
    return;
  }
  rt.blocks[idx] = block;
}

function getOrCreateBlock(
  rt: SessionRuntime,
  id: string,
  create: () => ChatBlock,
): ChatBlock {
  const idx = rt.blockIndexById.get(id);
  if (idx === undefined) {
    const block = create();
    rt.blockIndexById.set(id, rt.blocks.length);
    rt.blocks.push(block);
    return block;
  }
  return rt.blocks[idx]!;
}

function newLocalId(prefix: string): string {
  return `${prefix}:${Date.now()}:${Math.random().toString(16).slice(2)}`;
}

function ensureParts(parts: string[], index: number): void {
  while (parts.length <= index) parts.push("");
}

function requestKeyFromId(id: string | number): string {
  return typeof id === "number" ? `n:${id}` : `s:${id}`;
}

function formatK(n: number): string {
  const v = Math.max(0, Math.round(n));
  if (v < 1000) return String(v);
  return `${Math.round(v / 1000)}k`;
}

function deprecationNoticeId(summary: string, details: string): string {
  const key = `${summary}\n${details}`.trim();
  const hash = crypto.createHash("sha1").update(key).digest("hex").slice(0, 10);
  return `global:deprecationNotice:${hash}`;
}

function formatTokenUsageStatus(tokenUsage: ThreadTokenUsage): string {
  const { total, modelContextWindow } = tokenUsage;
  if (modelContextWindow !== null && modelContextWindow > 0) {
    const used = total.totalTokens;
    const remaining = Math.max(0, modelContextWindow - used);
    const remainingPct = Math.max(
      0,
      Math.min(100, Math.round((remaining / modelContextWindow) * 100)),
    );
    return `remaining=${remainingPct}% (${formatK(remaining)}/${formatK(modelContextWindow)})`;
  }
  return `tokens used=${formatK(total.totalTokens)}`;
}

function computeWorkedSeconds(rt: SessionRuntime): number | null {
  const started = rt.lastTurnStartedAtMs;
  if (started === null) return null;
  const ended = rt.lastTurnCompletedAtMs ?? Date.now();
  const diffMs = Math.max(0, ended - started);
  return Math.max(0, Math.round(diffMs / 1000));
}

function makeDividerLine(label: string): string {
  const prefix = `─ ${label} `;
  const targetWidth = 56;
  const remaining = Math.max(0, targetWidth - prefix.length);
  return `${prefix}${"─".repeat(remaining)}`;
}

function formatParamsForDisplay(params: unknown): string {
  let json = "";
  try {
    json = JSON.stringify(params, null, 2);
  } catch {
    return String(params);
  }

  const limit = 10_000;
  if (json.length <= limit) return json;
  return `${json.slice(0, limit)}\n...(truncated ${json.length - limit} chars)`;
}

function removeGlobalWhere(pred: (b: ChatBlock) => boolean): void {
  const next: ChatBlock[] = [];
  for (const b of globalRuntime.blocks) {
    if (!pred(b)) next.push(b);
  }
  globalRuntime.blocks.length = 0;
  globalRuntime.blocks.push(...next);
  globalRuntime.blockIndexById.clear();
  for (let i = 0; i < next.length; i++) {
    const b = next[i];
    if (!b) continue;
    globalRuntime.blockIndexById.set(b.id, i);
  }
}

function applyGlobalNotification(n: AnyServerNotification): void {
  switch (n.method) {
    case "rawResponseItem/completed":
      // Internal-only (Codex Cloud). Avoid flooding "Other events (debug)".
      return;
    case "deprecationNotice": {
      const p = (n as any).params as { summary?: unknown; details?: unknown };
      const summary = String(p?.summary ?? "").trim();
      const details =
        typeof p?.details === "string" ? String(p.details).trim() : "";
      const id = deprecationNoticeId(summary, details);
      upsertGlobal({
        id,
        type: "info",
        title: "Deprecation notice",
        text: details ? `${summary}\n\n${details}` : summary,
      });
      chatView?.refresh();
      return;
    }
    case "thread/started": {
      const thread = (n as any).params?.thread as {
        id?: unknown;
        cwd?: unknown;
        cliVersion?: unknown;
        gitInfo?: { originUrl?: unknown } | null;
      } | null;
      const id = typeof thread?.id === "string" ? thread.id : null;
      const cwd = typeof thread?.cwd === "string" ? thread.cwd : null;
      const cliVersion =
        typeof thread?.cliVersion === "string" ? thread.cliVersion : null;
      const originUrl =
        typeof thread?.gitInfo?.originUrl === "string"
          ? thread.gitInfo.originUrl
          : null;

      if (!id) {
        appendUnhandledGlobalEvent(
          `Unhandled global event: ${n.method}`,
          (n as any).params,
        );
        chatView?.refresh();
        return;
      }

      const lines: string[] = [];
      if (cwd) lines.push(`Working directory: \`${cwd}\``);
      if (cliVersion) lines.push(`CLI version: \`${cliVersion}\``);
      if (originUrl) lines.push(`Git origin: ${originUrl}`);
      const mcpLine = formatMcpStatusSummary();
      if (mcpLine) lines.push(mcpLine);

      const backendKey = backendKeyForCwd(cwd);
      if (backendKey) {
        const next = inferCliVariantFromCliVersion(cliVersion);
        cliVariantByBackendKey.set(backendKey, next);
      }

      // De-dupe: `New` creates a new thread and emits `thread/started` again, but for the same cwd we only
      // want one "Thread started" notice.
      const globalId = cwd
        ? `global:threadStarted:cwd:${cwd}`
        : `global:threadStarted:thread:${id}`;
      removeGlobalWhere(
        (b) =>
          b.id.startsWith("global:threadStarted:") &&
          b.id !== globalId &&
          b.type === "info" &&
          b.title === "Thread started",
      );
      upsertGlobal({
        id: globalId,
        type: "info",
        title: "Thread started",
        text: lines.join("\n") || "(no details)",
      });
      chatView?.refresh();
      return;
    }
    case "windows/worldWritableWarning": {
      const p = (n as any).params as {
        samplePaths: string[];
        extraCount: number;
        failedScan: boolean;
      };
      upsertGlobal({
        id: newLocalId("notice"),
        type: "system",
        title: "Windows world-writable warning",
        text: `failedScan=${String(p.failedScan)}\nextraCount=${String(p.extraCount)}\npaths:\n${(p.samplePaths ?? []).join("\n")}`,
      });
      chatView?.refresh();
      return;
    }
    case "account/updated": {
      globalStatusText = `authMode=${String((n as any).params.authMode ?? "null")}`;
      chatView?.refresh();
      return;
    }
    case "account/rateLimits/updated": {
      const rateLimits: RateLimitSnapshot = (n as any).params
        .rateLimits as RateLimitSnapshot;
      const p = rateLimits.primary;
      const s = rateLimits.secondary;
      const plan = rateLimits.planType ?? null;
      const primary = p
        ? `primary=${Math.round(p.usedPercent * 100) / 100}%`
        : "primary=null";
      const secondary = s
        ? `secondary=${Math.round(s.usedPercent * 100) / 100}%`
        : "secondary=null";
      globalStatusText = `${primary} ${secondary} plan=${String(plan)}`;
      chatView?.refresh();
      return;
    }
    case "mcpServer/oauthLogin/completed": {
      const p = (n as any).params as {
        name: string;
        success: boolean;
        error?: string;
      };
      if (!p.success) {
        upsertGlobal({
          id: newLocalId("mcpOauth"),
          type: "system",
          title: "MCP OAuth login failed",
          text: `server=${p.name}\nerror=${String(p.error ?? "null")}`,
        });
      }
      chatView?.refresh();
      return;
    }
    case "account/login/completed": {
      const p = (n as any).params as { success?: boolean; provider?: string };
      upsertGlobal({
        id: newLocalId("auth"),
        type: p?.success ? "info" : "error",
        title: p?.success ? "Login succeeded" : "Login failed",
        text: `provider=${String(p?.provider ?? "unknown")}`,
      });
      chatView?.refresh();
      return;
    }
    case "authStatusChange":
    case "loginChatGptComplete": {
      const p = (n as any).params as { authMode?: string; user?: string };
      upsertGlobal({
        id: newLocalId("authStatus"),
        type: "info",
        title: "Auth status changed",
        text: `mode=${String(p?.authMode ?? "unknown")}${p?.user ? `\nuser=${p.user}` : ""}`,
      });
      chatView?.refresh();
      return;
    }
    case "sessionConfigured": {
      const p = (n as any).params as Record<string, unknown>;
      upsertGlobal({
        id: newLocalId("sessionConfigured"),
        type: "info",
        title: "Session configured",
        text: formatSessionConfigForDisplay(p),
      });
      chatView?.refresh();
      return;
    }
    default: {
      if (n.method.startsWith("codex/event/")) {
        applyGlobalCodexEvent(n.method, (n as any).params);
        chatView?.refresh();
        return;
      }

      appendUnhandledGlobalEvent(
        `Unhandled global event: ${n.method}`,
        (n as any).params,
      );
      chatView?.refresh();
      return;
    }
  }
}

function upsertGlobal(block: ChatBlock): void {
  const idx = globalRuntime.blockIndexById.get(block.id);
  if (idx === undefined) {
    globalRuntime.blockIndexById.set(block.id, globalRuntime.blocks.length);
    globalRuntime.blocks.push(block);
    return;
  }
  globalRuntime.blocks[idx] = block;
}

function appendUnhandledGlobalEvent(title: string, params: unknown): void {
  const id = "global:unhandled";
  const existing = globalRuntime.blocks.find((b) => b.id === id);
  const line = `${title}\n${formatParamsForDisplay(params)}\n`;
  if (existing && existing.type === "system") {
    existing.text = `${existing.text}\n${line}`.trim();
    upsertGlobal(existing);
    return;
  }

  upsertGlobal({
    id,
    type: "system",
    title: "Other events (debug)",
    text: line.trim(),
  });
}

function formatMcpStatusSummary(): string | null {
  if (mcpStatusByServer.size === 0) return null;
  const icon = (state: string): string =>
    state === "ready" ? "✓" : state === "starting" ? "…" : "•";
  const lines = [...mcpStatusByServer.entries()].map(
    ([server, state]) => `${icon(state)} ${server}`,
  );
  return ["MCP servers:", ...lines].join("\n");
}

function formatSessionConfigForDisplay(params: Record<string, unknown>): string {
  const model = typeof params.model === "string" ? params.model : "default";
  const provider =
    typeof params.modelProvider === "string" ? params.modelProvider : "default";
  const sandbox =
    typeof params.sandbox === "string" ? params.sandbox : "default";
  const plan = typeof params.planType === "string" ? params.planType : "default";
  return `model=${model}\nprovider=${provider}\nsandbox=${sandbox}\nplan=${plan}`;
}

function updateThreadStartedBlocks(): void {
  const summary = formatMcpStatusSummary();
  let changed = false;
  for (let i = 0; i < globalRuntime.blocks.length; i++) {
    const b = globalRuntime.blocks[i];
    if (!b) continue;
    if (b.type !== "info" || b.title !== "Thread started") continue;
    const lines = b.text
      .split("\n")
      .filter(
        (l) =>
          !l.startsWith("MCP servers:") &&
          !/^\s*-?\s*[✓…•]/.test(l),
      );
    if (summary) lines.push(summary);
    const nextText = lines.join("\n");
    if (nextText !== b.text) {
      globalRuntime.blocks[i] = { ...b, text: nextText };
      changed = true;
    }
  }
  if (changed) chatView?.refresh();
}

function appendUnhandledEvent(
  rt: SessionRuntime,
  title: string,
  params: unknown,
): void {
  const id = "unhandled";
  const block = getOrCreateBlock(rt, id, () => ({
    id,
    type: "system",
    title: "Other events (debug)",
    text: "",
  }));
  if (block.type !== "system") return;
  block.text =
    `${block.text}\n${title}\n${formatParamsForDisplay(params)}\n`.trim();
}

function applyGlobalCodexEvent(method: string, params: unknown): void {
  const p = params as any;
  const msg = p?.msg as any;
  const type = typeof msg?.type === "string" ? msg.type : null;
  if (type === "token_count") {
    const info =
      msg.info?.total_token_usage ?? msg.info?.last_token_usage ?? null;
    const ctx =
      msg.info?.model_context_window ?? msg.model_context_window ?? null;
    if (info) {
      if (typeof ctx === "number" && ctx > 0) {
        const used = info.total_tokens;
        const remaining = Math.max(0, ctx - used);
        const remainingPct = Math.max(
          0,
          Math.min(100, Math.round((remaining / ctx) * 100)),
        );
        globalStatusText = `remaining=${remainingPct}% (${remaining}/${ctx})`;
      } else {
        globalStatusText = `tokens used=${info.total_tokens}`;
      }
    } else if (ctx) {
      globalStatusText = `ctx=${String(ctx)}`;
    }
    return;
  }

  if (type === "web_search_begin" || type === "web_search_end") {
    // Web search events are session-scoped when possible; avoid duplicating at global level.
    return;
  }

  if (type === "stream_error") {
    // Prefer the dedicated v2 error notification block; avoid showing a noisy legacy dump.
    return;
  }

  if (
    type === "exec_command_begin" ||
    type === "exec_command_output_delta" ||
    type === "terminal_interaction" ||
    type === "exec_command_end"
  ) {
    // Command events are session-scoped when possible; avoid duplicating at global level.
    return;
  }

  if (type === "mcp_startup_complete") {
    const failed = Array.isArray(msg.failed) ? msg.failed : [];
    const cancelled = Array.isArray(msg.cancelled) ? msg.cancelled : [];
    if (failed.length === 0 && cancelled.length === 0) return;
    upsertGlobal({
      id: newLocalId("mcpStartup"),
      type: "system",
      title: "MCP startup issues",
      text: formatParamsForDisplay(msg),
    });
    return;
  }

  if (type === "mcp_startup_update") {
    const server = typeof msg.server === "string" ? msg.server : "(unknown)";
    const status = typeof msg.status === "object" && msg.status !== null ? msg.status : {};
    const state = typeof (status as any).state === "string" ? (status as any).state : "unknown";
    if (server !== "(unknown)") mcpStatusByServer.set(server, state);
    updateThreadStartedBlocks();
    return;
  }

  if (type === "turn_aborted") {
    // Prefer v2 turn lifecycle events; don't spam global "Other events (debug)" on interrupts.
    return;
  }

  // Ignore noisy per-token / per-item legacy events.
  if (
    type === "task_started" ||
    type === "task_complete" ||
    type === "item_started" ||
    type === "item_completed" ||
    type === "user_message" ||
    type === "agent_message" ||
    type === "agent_message_delta" ||
    type === "agent_message_content_delta" ||
    type === "agent_message" ||
    type === "token_count" ||
    type === "agent_reasoning" ||
    type === "agent_reasoning_delta" ||
    type === "agent_reasoning_raw_content" ||
    type === "agent_reasoning_raw_content_delta" ||
    type === "agent_reasoning_section_break" ||
    type === "reasoning_content_delta" ||
    type === "reasoning_raw_content_delta"
  ) {
    return;
  }

  appendUnhandledGlobalEvent(`Legacy event: ${method}`, params);
}

function applyCodexEvent(
  rt: SessionRuntime,
  sessionId: string,
  method: string,
  params: unknown,
): void {
  const p = params as any;
  const msg = p?.msg as any;
  const type = typeof msg?.type === "string" ? msg.type : null;
  if (!type) {
    appendUnhandledEvent(rt, `Legacy event: ${method}`, params);
    return;
  }

  if (type === "stream_error") {
    // Prefer the dedicated v2 error notification block; avoid showing a noisy legacy dump.
    return;
  }

  if (type === "list_custom_prompts_response") {
    const raw = Array.isArray(msg.custom_prompts)
      ? (msg.custom_prompts as Array<{
          name?: unknown;
          description?: unknown;
          argument_hint?: unknown;
          content?: unknown;
        }>)
      : [];
    const next = raw
      .map((p) => ({
        name: typeof p?.name === "string" ? p.name.trim() : "",
        description: typeof p?.description === "string" ? p.description : null,
        argumentHint: typeof p?.argument_hint === "string" ? p.argument_hint : null,
        content: typeof p?.content === "string" ? p.content : "",
      }))
      .filter((p) => !!p.name)
      .map((p) => ({ ...p, source: "server" as const }));
    setCustomPrompts(next);
    return;
  }

  if (type === "mcp_startup_update") {
    // グローバル側で表示するのでセッションスコープでは重複表示しない。
    const server = typeof msg.server === "string" ? msg.server : "(unknown)";
    const status =
      typeof msg.status === "object" && msg.status !== null ? msg.status : {};
    const state = typeof (status as any).state === "string" ? (status as any).state : "unknown";
    if (server !== "(unknown)") {
      mcpStatusByServer.set(server, state);
      updateThreadStartedBlocks();
    }
    return;
  }

  const workspaceFolderFsPath = (() => {
    const s = sessions?.getById(sessionId) ?? null;
    if (!s) return null;
    try {
      return vscode.Uri.parse(s.workspaceFolderUri).fsPath;
    } catch {
      return null;
    }
  })();

  if (type === "exec_command_begin") {
    const callId = String(msg.call_id ?? "");
    if (!callId) return;
    const id = `legacyCmd:${callId}`;
    const command = Array.isArray(msg.command)
      ? msg.command.map(String).join(" ")
      : String(msg.command ?? "");
    const cwd = typeof msg.cwd === "string" ? msg.cwd : null;
    const block = getOrCreateBlock(rt, id, () => ({
      id,
      type: "command",
      title: "Command",
      status: "inProgress",
      command,
      cwd,
      exitCode: null,
      durationMs: null,
      terminalStdin: [],
      output: "",
    }));
    if (block.type === "command") {
      block.status = "inProgress";
      if (command) block.command = command;
      block.cwd = cwd;
    }
    return;
  }

  if (type === "exec_command_output_delta") {
    const callId = String(msg.call_id ?? "");
    if (!callId) return;
    const id = `legacyCmd:${callId}`;
    const block = getOrCreateBlock(rt, id, () => ({
      id,
      type: "command",
      title: "Command",
      status: "inProgress",
      command: "",
      cwd: null,
      exitCode: null,
      durationMs: null,
      terminalStdin: [],
      output: "",
    }));
    if (block.type === "command") block.output += String(msg.chunk ?? "");
    return;
  }

  if (type === "terminal_interaction") {
    const callId = String(msg.call_id ?? "");
    if (!callId) return;
    const id = `legacyCmd:${callId}`;
    const block = getOrCreateBlock(rt, id, () => ({
      id,
      type: "command",
      title: "Command",
      status: "inProgress",
      command: "",
      cwd: null,
      exitCode: null,
      durationMs: null,
      terminalStdin: [],
      output: "",
    }));
    if (block.type === "command")
      block.terminalStdin.push(String(msg.stdin ?? ""));
    return;
  }

  if (type === "exec_command_end") {
    const callId = String(msg.call_id ?? "");
    if (!callId) return;
    const id = `legacyCmd:${callId}`;
    const command = Array.isArray(msg.command)
      ? msg.command.map(String).join(" ")
      : String(msg.command ?? "");
    const cwd = typeof msg.cwd === "string" ? msg.cwd : null;
    const exitCode = typeof msg.exit_code === "number" ? msg.exit_code : null;
    const durationMs =
      typeof msg.duration_ms === "number"
        ? msg.duration_ms
        : typeof msg.duration === "number"
          ? msg.duration
          : null;
    const output = String(msg.aggregated_output ?? msg.formatted_output ?? "");
    const block = getOrCreateBlock(rt, id, () => ({
      id,
      type: "command",
      title: "Command",
      status: "completed",
      command,
      cwd,
      exitCode,
      durationMs,
      terminalStdin: [],
      output,
    }));
    if (block.type === "command") {
      block.status = "completed";
      if (command) block.command = command;
      block.cwd = cwd;
      block.exitCode = exitCode;
      block.durationMs = durationMs;
      if (output) block.output = output;
    }
    return;
  }

  if (type === "token_count") {
    const info =
      msg.info?.total_token_usage ?? msg.info?.last_token_usage ?? null;
    const ctx =
      msg.info?.model_context_window ?? msg.model_context_window ?? null;
    if (info) {
      if (typeof ctx === "number" && ctx > 0) {
        const used = info.total_tokens;
        const remaining = Math.max(0, ctx - used);
        const remainingPct = Math.max(
          0,
          Math.min(100, Math.round((remaining / ctx) * 100)),
        );
        rt.statusText = `remaining=${remainingPct}% (${remaining}/${ctx})`;
      } else {
        rt.statusText = `tokens used=${info.total_tokens}`;
      }
    } else if (ctx) {
      rt.statusText = `ctx=${String(ctx)}`;
    }
    return;
  }

  if (type === "turn_aborted") {
    const reason = typeof msg.reason === "string" ? msg.reason : "unknown";
    rt.sending = false;
    rt.lastTurnCompletedAtMs = Date.now();
    rt.activeTurnId = null;
    upsertBlock(sessionId, {
      id: newLocalId("turnAborted"),
      type: "note",
      text: reason === "interrupted" ? "Interrupted" : `Aborted (${reason})`,
    });
    return;
  }

  if (type === "mcp_startup_complete") {
    const failed = Array.isArray(msg.failed) ? msg.failed : [];
    const cancelled = Array.isArray(msg.cancelled) ? msg.cancelled : [];
    if (failed.length === 0 && cancelled.length === 0) return;
    upsertBlock(rt, {
      id: newLocalId("mcpStartup"),
      type: "system",
      title: "MCP startup issues",
      text: formatParamsForDisplay(msg),
    });
    return;
  }

  if (type === "web_search_begin" || type === "web_search_end") {
    const callId = String(msg.call_id ?? "");
    if (!callId) return;
    const query = typeof msg.query === "string" ? msg.query : "";
    if (!query) return;
    const targetId =
      rt.legacyWebSearchTargetByCallId.get(callId) ??
      findRecentWebSearchBlockIdByQuery(rt, query) ??
      `legacyWebSearch:${callId}`;
    rt.legacyWebSearchTargetByCallId.set(callId, targetId);
    upsertBlock(rt, {
      id: targetId,
      type: "webSearch",
      query,
      status: type === "web_search_begin" ? "inProgress" : "completed",
    });
    return;
  }

  if (type === "patch_apply_begin") {
    const callId = String(msg.call_id ?? "");
    if (!callId) return;
    const changes = (msg.changes ?? {}) as Record<string, unknown>;
    const files = Object.keys(changes).map((p) =>
      formatPathForSession(p, workspaceFolderFsPath),
    );
    const targetId =
      rt.legacyPatchTargetByCallId.get(callId) ??
      findRecentFileChangeBlockIdByFiles(rt, files) ??
      `legacyPatch:${callId}`;
    rt.legacyPatchTargetByCallId.set(callId, targetId);

    const block = getOrCreateBlock(rt, targetId, () => ({
      id: targetId,
      type: "fileChange",
      title: "Changes",
      status: "inProgress",
      files,
      detail: "",
      hasDiff: rt.latestDiff != null,
      diffs: diffsForFiles(files, rt.latestDiff),
    }));
    if (block.type === "fileChange") {
      block.status = "inProgress";
      block.files = files;
      block.hasDiff = rt.latestDiff != null;
      block.diffs = diffsForFiles(files, rt.latestDiff);
      const auto = msg.auto_approved === true ? "auto_approved=true" : null;
      const turnId = typeof msg.turn_id === "string" ? msg.turn_id : null;
      const lines = [
        "Applying patch…",
        auto,
        turnId ? `turn_id=${turnId}` : null,
      ]
        .filter(Boolean)
        .join("\n");
      block.detail = lines;
    }
    return;
  }

  if (type === "patch_apply_end") {
    const callId = String(msg.call_id ?? "");
    if (!callId) return;
    const success = msg.success === true;
    const stdout = typeof msg.stdout === "string" ? msg.stdout : "";
    const stderr = typeof msg.stderr === "string" ? msg.stderr : "";
    const changes = (msg.changes ?? {}) as Record<string, unknown>;
    const files = Object.keys(changes).map((p) =>
      formatPathForSession(p, workspaceFolderFsPath),
    );
    const targetId =
      rt.legacyPatchTargetByCallId.get(callId) ??
      findRecentFileChangeBlockIdByFiles(rt, files) ??
      `legacyPatch:${callId}`;
    rt.legacyPatchTargetByCallId.set(callId, targetId);

    const block = getOrCreateBlock(rt, targetId, () => ({
      id: targetId,
      type: "fileChange",
      title: "Changes",
      status: success ? "completed" : "failed",
      files,
      detail: "",
      hasDiff: rt.latestDiff != null,
      diffs: diffsForFiles(files, rt.latestDiff),
    }));
    if (block.type === "fileChange") {
      block.status = success ? "completed" : "failed";
      block.files = files;
      block.hasDiff = rt.latestDiff != null;
      block.diffs = diffsForFiles(files, rt.latestDiff);
      const lines: string[] = [];
      if (stdout.trim()) lines.push(stdout.trimEnd());
      if (stderr.trim()) lines.push(stderr.trimEnd());
      block.detail = lines.join("\n\n");
    }
    return;
  }

  if (type === "turn_diff") {
    const diff = typeof msg.unified_diff === "string" ? msg.unified_diff : "";
    if (!diff) return;
    if (rt.latestDiff === diff) return; // de-dupe noisy repeats
    rt.latestDiff = diff;
    for (const b of rt.blocks) {
      if (b.type === "fileChange") {
        b.hasDiff = true;
        b.diffs = diffsForFiles(b.files, rt.latestDiff);
      }
    }
    return;
  }

  // Ignore noisy duplicates; the v2 notifications cover these.
  if (
    type === "plan_update" ||
    type === "task_started" ||
    type === "task_complete" ||
    type === "item_started" ||
    type === "item_completed" ||
    type === "user_message" ||
    type === "agent_message" ||
    type === "agent_message_delta" ||
    type === "agent_message_content_delta" ||
    type === "agent_reasoning" ||
    type === "agent_reasoning_delta" ||
    type === "agent_reasoning_raw_content" ||
    type === "agent_reasoning_raw_content_delta" ||
    type === "agent_reasoning_section_break" ||
    type === "reasoning_content_delta" ||
    type === "reasoning_raw_content_delta"
  ) {
    return;
  }

  appendUnhandledEvent(rt, `Legacy event: ${method}`, params);
}

function formatPlanStatus(status: string): string {
  const s = status.trim();
  if (s === "completed" || s === "done") return "✅";
  if (s === "inProgress" || s === "in_progress" || s === "in-progress")
    return "▶️";
  if (s === "pending" || s === "todo") return "⏳";
  if (s === "cancelled" || s === "canceled") return "🚫";
  if (s === "skipped") return "⏭️";
  return "•";
}

function formatPathForSession(
  filePath: string,
  workspaceFolderFsPath: string | null,
): string {
  if (!workspaceFolderFsPath) return filePath;
  if (!path.isAbsolute(filePath)) return filePath;

  const root = workspaceFolderFsPath;
  const prefix = root.endsWith(path.sep) ? root : root + path.sep;
  if (!filePath.startsWith(prefix)) return filePath;

  return path.relative(root, filePath).split(path.sep).join("/");
}

function splitUnifiedDiffByFile(unifiedDiff: string): Map<string, string> {
  const map = new Map<string, string>();
  const lines = unifiedDiff.split("\n");

  let curPath: string | null = null;
  let curLines: string[] = [];

  const flush = (): void => {
    if (!curPath) return;
    map.set(curPath, curLines.join("\n"));
  };

  for (const line of lines) {
    if (line.startsWith("diff --git ")) {
      flush();
      curLines = [line];
      curPath = null;

      const m = line.match(/^diff --git a\/(.+?) b\/(.+)$/);
      if (m) {
        const aPath = m[1] || "";
        const bPath = m[2] || "";
        curPath = bPath !== "/dev/null" ? bPath : aPath;
      }
      continue;
    }

    if (curLines.length === 0) continue; // ignore preface before first diff --git
    curLines.push(line);

    if (!curPath && line.startsWith("+++ ")) {
      const plus = line.slice(4);
      if (plus.startsWith("b/")) curPath = plus.slice(2);
      else if (plus.startsWith("a/")) curPath = plus.slice(2);
    }
  }

  flush();
  return map;
}

function diffsForFiles(
  files: string[],
  latestDiff: string | null,
): Array<{ path: string; diff: string }> {
  if (!latestDiff) return [];
  const byFile = splitUnifiedDiffByFile(latestDiff);
  const out: Array<{ path: string; diff: string }> = [];
  for (const f of files) {
    const norm = String(f || "").replace(/^\/+/, "");
    const diff = byFile.get(norm) ?? null;
    if (diff) out.push({ path: norm, diff });
  }
  return out;
}

function normalizeFileListForCompare(files: string[]): string[] {
  return files
    .map((f) => String(f || "").replace(/^\/+/, ""))
    .filter((f) => f.length > 0)
    .slice()
    .sort((a, b) => a.localeCompare(b));
}

function findRecentFileChangeBlockIdByFiles(
  rt: SessionRuntime,
  files: string[],
): string | null {
  const want = normalizeFileListForCompare(files);
  if (want.length === 0) return null;

  for (let i = rt.blocks.length - 1; i >= 0; i--) {
    const b = rt.blocks[i];
    if (!b || b.type !== "fileChange") continue;
    // Prefer v2 blocks; avoid binding to legacyPatch blocks unless it's the only one.
    if (String(b.id || "").startsWith("legacyPatch:")) continue;

    const have = normalizeFileListForCompare(b.files || []);
    if (have.length !== want.length) continue;
    let ok = true;
    for (let j = 0; j < want.length; j++) {
      if (want[j] !== have[j]) {
        ok = false;
        break;
      }
    }
    if (ok) return b.id;
  }

  return null;
}

function findRecentWebSearchBlockIdByQuery(
  rt: SessionRuntime,
  query: string,
): string | null {
  const q = query.trim();
  if (!q) return null;
  for (let i = rt.blocks.length - 1; i >= 0; i--) {
    const b = rt.blocks[i];
    if (!b || b.type !== "webSearch") continue;
    if (String(b.id || "").startsWith("legacyWebSearch:")) continue;
    if (b.query.trim() === q) return b.id;
  }
  return null;
}

function hydrateRuntimeFromThread(sessionId: string, thread: Thread): void {
  const rt = ensureRuntime(sessionId);

  const hasConversationBlocks = rt.blocks.some((b) => {
    switch (b.type) {
      case "user":
      case "assistant":
      case "command":
      case "fileChange":
      case "mcp":
      case "webSearch":
      case "reasoning":
      case "plan":
      case "divider":
        return true;
      default:
        return false;
    }
  });
  if (hasConversationBlocks) return;

  // Preserve non-conversation blocks that may have arrived before hydration (e.g. legacy warnings).
  const preserved = rt.blocks.filter((b) =>
    b.type === "info" ||
    b.type === "system" ||
    b.type === "note" ||
    b.type === "error",
  );

  rt.blocks.length = 0;
  rt.blockIndexById.clear();

  const turns: Turn[] = Array.isArray(thread.turns) ? thread.turns : [];
  for (const turn of turns) {
    for (const item of turn.items ?? []) {
      applyItemLifecycle(rt, sessionId, thread.id, item, true);
      if (item.type === "userMessage") {
        const text = item.content
          .filter((c) => c.type === "text")
          .map((c) => c.text)
          .join("\n");
        if (text)
          upsertBlock(rt, { id: item.id, type: "user", text });
      }
      if (item.type === "agentMessage") {
        if (item.text)
          upsertBlock(rt, { id: item.id, type: "assistant", text: item.text });
      }
    }
  }

  for (const b of preserved) upsertBlock(rt, b);
}

function formatApprovalDetail(
  method: string,
  item: unknown,
  reason: string | null,
): string {
  const lines: string[] = [];
  lines.push(`method: ${method}`);
  if (reason) lines.push(`reason: ${reason}`);

  if (typeof item === "object" && item !== null) {
    const anyItem = item as Record<string, unknown>;
    const type = anyItem["type"];
    if (type === "commandExecution") {
      const command = anyItem["command"];
      const cwd = anyItem["cwd"];
      if (typeof cwd === "string") lines.push(`cwd: ${cwd}`);
      if (typeof command === "string") lines.push(`$ ${command}`);
    } else if (type === "fileChange") {
      const changes = anyItem["changes"];
      if (Array.isArray(changes)) {
        const paths = changes
          .map((c) =>
            typeof c === "object" && c !== null ? (c as any).path : null,
          )
          .filter((p) => typeof p === "string") as string[];
        if (paths.length > 0) lines.push(`files: ${paths.join(", ")}`);
      }
    }
  }

  return lines.join("\n");
}

const SESSIONS_KEY = "codexMine.sessions.v1";
type PersistedSession = Pick<
  Session,
  "id" | "backendKey" | "workspaceFolderUri" | "title" | "threadId" | "customTitle"
>;

function loadSessions(
  context: vscode.ExtensionContext,
  store: SessionStore,
): void {
  const raw = context.workspaceState.get<unknown>(SESSIONS_KEY);
  if (!Array.isArray(raw)) return;

  for (const item of raw) {
    if (typeof item !== "object" || item === null) continue;
    const o = item as Record<string, unknown>;
    const id = o["id"];
    const backendKey = o["backendKey"];
    const workspaceFolderUri = o["workspaceFolderUri"];
    const title = o["title"];
    const customTitle = o["customTitle"];
    const threadId = o["threadId"];

    if (
      typeof id !== "string" ||
      typeof backendKey !== "string" ||
      typeof workspaceFolderUri !== "string" ||
      typeof title !== "string" ||
      typeof threadId !== "string"
    ) {
      continue;
    }

    store.add(backendKey, {
      id,
      backendKey,
      workspaceFolderUri,
      title,
      customTitle: typeof customTitle === "boolean" ? customTitle : false,
      threadId,
    });
  }
}

function saveSessions(
  context: vscode.ExtensionContext,
  store: SessionStore,
): void {
  const sessions = store.listAll().map<PersistedSession>(toPersistedSession);
  void context.workspaceState.update(SESSIONS_KEY, sessions);
}

function toPersistedSession(session: Session): PersistedSession {
  const { id, backendKey, workspaceFolderUri, title, customTitle, threadId } =
    session;
  return { id, backendKey, workspaceFolderUri, title, customTitle, threadId };
}

const RUNTIMES_KEY = "codexMine.sessionRuntime.v1";
type PersistedRuntime = {
  blocks: ChatBlock[];
  latestDiff: string | null;
  statusText: string | null;
  lastTurnStartedAtMs: number | null;
  lastTurnCompletedAtMs: number | null;
};

let persistedRuntimeCache: Record<string, PersistedRuntime> = {};
let persistRuntimeTimer: NodeJS.Timeout | null = null;
const dirtyRuntimeSessionIds = new Set<string>();

function loadRuntimes(
  context: vscode.ExtensionContext,
  store: SessionStore,
): void {
  const raw = context.workspaceState.get<unknown>(RUNTIMES_KEY);
  if (typeof raw !== "object" || raw === null) return;
  persistedRuntimeCache = raw as Record<string, PersistedRuntime>;

  for (const session of store.listAll()) {
    const persisted = persistedRuntimeCache[session.id];
    if (!persisted) continue;
    restoreRuntime(session.id, persisted);
  }
}

function restoreRuntime(sessionId: string, persisted: PersistedRuntime): void {
  const rt = ensureRuntime(sessionId);
  rt.blocks = Array.isArray(persisted.blocks) ? persisted.blocks : [];
  rt.latestDiff = persisted.latestDiff ?? null;
  rt.statusText = persisted.statusText ?? null;
  rt.lastTurnStartedAtMs = persisted.lastTurnStartedAtMs ?? null;
  rt.lastTurnCompletedAtMs = persisted.lastTurnCompletedAtMs ?? null;
  rt.sending = false;

  rt.blockIndexById.clear();
  for (let i = 0; i < rt.blocks.length; i++) {
    const b = rt.blocks[i];
    if (
      typeof b === "object" &&
      b !== null &&
      typeof (b as any).id === "string"
    ) {
      rt.blockIndexById.set((b as any).id as string, i);
    }
  }
}

function schedulePersistRuntime(sessionId: string): void {
  const context = extensionContext;
  if (!context) return;

  dirtyRuntimeSessionIds.add(sessionId);
  if (persistRuntimeTimer) return;

  persistRuntimeTimer = setTimeout(() => {
    persistRuntimeTimer = null;
    if (!sessions) return;

    for (const id of dirtyRuntimeSessionIds) {
      const rt = runtimeBySessionId.get(id);
      if (!rt) continue;
      // Only persist for sessions that still exist.
      if (!sessions.getById(id)) continue;
      persistedRuntimeCache[id] = {
        blocks: rt.blocks,
        latestDiff: rt.latestDiff,
        statusText: rt.statusText,
        lastTurnStartedAtMs: rt.lastTurnStartedAtMs,
        lastTurnCompletedAtMs: rt.lastTurnCompletedAtMs,
      };
    }
    dirtyRuntimeSessionIds.clear();
    void context.workspaceState.update(RUNTIMES_KEY, persistedRuntimeCache);
  }, 250);
}

function deletePersistedRuntime(
  context: vscode.ExtensionContext,
  sessionId: string,
): void {
  delete persistedRuntimeCache[sessionId];
  dirtyRuntimeSessionIds.delete(sessionId);
  void context.workspaceState.update(RUNTIMES_KEY, persistedRuntimeCache);
}
