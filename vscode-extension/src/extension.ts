import { parse as parseToml } from "@iarna/toml";
import * as crypto from "node:crypto";
import { spawn } from "node:child_process";
import * as fs from "node:fs/promises";
import * as os from "node:os";
import * as path from "node:path";
import * as util from "node:util";
import { parse as shellParse } from "shell-quote";
import * as vscode from "vscode";
import { BackendManager } from "./backend/manager";
import type { BackendTermination } from "./backend/manager";
import { listAgentsFromDisk } from "./agents_disk";
import type { AnyServerNotification } from "./backend/types";
import type { ContentBlock } from "./generated/ContentBlock";
import type { ImageContent } from "./generated/ImageContent";
import type { AskUserQuestionRequest } from "./generated/AskUserQuestionRequest";
import type { CommandAction } from "./generated/v2/CommandAction";
import type { Model } from "./generated/v2/Model";
import type { RateLimitSnapshot } from "./generated/v2/RateLimitSnapshot";
import type { RateLimitWindow } from "./generated/v2/RateLimitWindow";
import type { AskUserQuestionResponse } from "./generated/v2/AskUserQuestionResponse";
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
import { SessionPanelManager } from "./ui/session_panel_manager";
import { SessionTreeDataProvider } from "./ui/session_tree";

const REWIND_STEP_TIMEOUT_MS = 120_000;
const LAST_ACTIVE_SESSION_KEY = "codexMine.lastActiveSessionId.v1";
const DEFAULT_PROJECT_DOC_FILENAME = "AGENTS.md";

let backendManager: BackendManager | null = null;
let sessions: SessionStore | null = null;
let sessionTree: SessionTreeDataProvider | null = null;
let diffProvider: DiffDocumentProvider | null = null;
let chatView: ChatViewProvider | null = null;
let sessionPanels: SessionPanelManager | null = null;
let activeSessionId: string | null = null;
let extensionContext: vscode.ExtensionContext | null = null;
let outputChannel: vscode.OutputChannel | null = null;
let initPromptCache: string | null = null;

type StressUiJob = {
  sessionId: string;
  cancel: () => void;
};
let stressUiJob: StressUiJob | null = null;

type CachedImageMeta = {
  mimeType: string;
  byteLength: number;
  createdAtMs: number;
};

const IMAGE_CACHE_DIRNAME = "images.v2";
const IMAGE_CACHE_MAX_ITEMS = 500;
const IMAGE_CACHE_MAX_TOTAL_BYTES = 250_000_000;
const SESSION_IMAGE_AUTOLOAD_RECENT = 24;
const USER_INPUT_IMAGE_DIRNAME = "user-input-images.v1";

async function withTimeout<T>(
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

function formatUnknownError(err: unknown): string {
  if (err instanceof Error) return err.stack ?? err.message;
  if (typeof err === "string") return err;
  if (typeof err === "number" || typeof err === "boolean" || err === null)
    return String(err);

  if (typeof err === "object" && err !== null) {
    const record = err as Record<string, unknown>;
    const msg = record["message"];
    const code = record["code"];
    if (
      typeof msg === "string" &&
      (typeof code === "string" || typeof code === "number")
    ) {
      return `code=${String(code)} message=${msg}`;
    }
    if (typeof msg === "string") return msg;
  }

  const inspected = util.inspect(err, {
    depth: 6,
    breakLength: 120,
    maxArrayLength: 50,
    maxStringLength: 10_000,
  });
  const maxLen = 12_000;
  if (inspected.length <= maxLen) return inspected;
  return `${inspected.slice(0, maxLen)}…(truncated ${inspected.length - maxLen} chars)`;
}

function requireExtensionContext(): vscode.ExtensionContext {
  if (!extensionContext) throw new Error("extensionContext is not initialized");
  return extensionContext;
}

async function getInitPrompt(
  context: vscode.ExtensionContext,
): Promise<string> {
  if (initPromptCache !== null) return initPromptCache;
  const uri = vscode.Uri.joinPath(
    context.extensionUri,
    "resources",
    "prompt_for_init_command.md",
  );
  const bytes = await vscode.workspace.fs.readFile(uri);
  initPromptCache = Buffer.from(bytes).toString("utf8");
  return initPromptCache;
}

function imageCacheDirFsPath(context: vscode.ExtensionContext): string {
  const base = context.globalStorageUri?.fsPath;
  if (!base) throw new Error("globalStorageUri is not available");
  return path.join(base, IMAGE_CACHE_DIRNAME);
}

function userInputImageDirFsPath(context: vscode.ExtensionContext): string {
  const base = context.globalStorageUri?.fsPath;
  if (!base) throw new Error("globalStorageUri is not available");
  return path.join(base, USER_INPUT_IMAGE_DIRNAME);
}

async function ensureUserInputImageDir(
  context: vscode.ExtensionContext,
): Promise<string> {
  const dir = userInputImageDirFsPath(context);
  await fs.mkdir(dir, { recursive: true });
  return dir;
}

function imageCachePaths(
  context: vscode.ExtensionContext,
  imageKey: string,
): { metaPath: string; dataPath: string } {
  const dir = imageCacheDirFsPath(context);
  return {
    metaPath: path.join(dir, `${imageKey}.json`),
    dataPath: path.join(dir, `${imageKey}.bin`),
  };
}

async function ensureImageCacheDir(
  context: vscode.ExtensionContext,
): Promise<string> {
  const dir = imageCacheDirFsPath(context);
  await fs.mkdir(dir, { recursive: true });
  return dir;
}

function sanitizeImageKey(key: string): string {
  return key.replace(/[^a-zA-Z0-9_.-]/g, "_").slice(0, 160) || "img";
}

async function pruneImageCache(
  context: vscode.ExtensionContext,
): Promise<void> {
  const dir = imageCacheDirFsPath(context);
  let entries: string[];
  try {
    entries = await fs.readdir(dir);
  } catch (err) {
    // Directory may not exist yet; do not create it during prune.
    return;
  }

  const metas = entries.filter((n) => n.endsWith(".json"));
  const items: Array<{
    imageKey: string;
    metaPath: string;
    dataPath: string;
    createdAtMs: number;
    byteLength: number;
  }> = [];

  for (const metaName of metas) {
    const imageKey = metaName.slice(0, -".json".length);
    const { metaPath, dataPath } = imageCachePaths(context, imageKey);
    try {
      const metaRaw = await fs.readFile(metaPath, "utf8");
      const meta = JSON.parse(metaRaw) as CachedImageMeta;
      if (
        !meta ||
        typeof meta.mimeType !== "string" ||
        typeof meta.byteLength !== "number" ||
        typeof meta.createdAtMs !== "number"
      ) {
        throw new Error(`Invalid meta: ${metaPath}`);
      }
      items.push({
        imageKey,
        metaPath,
        dataPath,
        createdAtMs: meta.createdAtMs,
        byteLength: meta.byteLength,
      });
    } catch (err) {
      // Corrupted meta: remove both files so it doesn't linger indefinitely.
      outputChannel?.appendLine(
        `[images] Corrupted meta '${metaName}', removing: ${String(err)}`,
      );
      await fs.rm(metaPath, { force: true }).catch(() => null);
      await fs.rm(dataPath, { force: true }).catch(() => null);
    }
  }

  items.sort((a, b) => b.createdAtMs - a.createdAtMs);
  let totalBytes = items.reduce((sum, it) => sum + it.byteLength, 0);

  for (let i = 0; i < items.length; i++) {
    const it = items[i]!;
    const keepByCount = i < IMAGE_CACHE_MAX_ITEMS;
    const keepByBytes = totalBytes <= IMAGE_CACHE_MAX_TOTAL_BYTES;
    if (keepByCount && keepByBytes) continue;
    totalBytes -= it.byteLength;
    await fs.rm(it.metaPath, { force: true }).catch(() => null);
    await fs.rm(it.dataPath, { force: true }).catch(() => null);
  }
}

async function cacheImageBytes(args: {
  imageKey?: string;
  prefix: string;
  mimeType: string;
  bytes: Buffer;
}): Promise<{ imageKey: string; mimeType: string; byteLength: number }> {
  const context = requireExtensionContext();
  await ensureImageCacheDir(context);
  const imageKey =
    typeof args.imageKey === "string" && args.imageKey
      ? sanitizeImageKey(args.imageKey)
      : sanitizeImageKey(`${args.prefix}-${crypto.randomUUID()}`);
  const { metaPath, dataPath } = imageCachePaths(context, imageKey);
  const meta: CachedImageMeta = {
    mimeType: args.mimeType,
    byteLength: args.bytes.byteLength,
    createdAtMs: Date.now(),
  };
  await fs.writeFile(dataPath, args.bytes);
  await fs.writeFile(metaPath, JSON.stringify(meta));
  void pruneImageCache(context);
  return {
    imageKey,
    mimeType: args.mimeType,
    byteLength: args.bytes.byteLength,
  };
}

function parseDataUrl(dataUrl: string): { mimeType: string; base64: string } {
  const m = /^data:([^;]+);base64,(.*)$/s.exec(dataUrl);
  if (!m)
    throw new Error("Unsupported image URL (expected data:...;base64,...)");
  const mimeType = m[1] || "";
  const base64 = m[2] || "";
  if (!mimeType || !base64) throw new Error("Invalid data URL");
  return { mimeType, base64 };
}

function imageExtFromMimeType(mimeType: string): string | null {
  switch (mimeType) {
    case "image/png":
      return "png";
    case "image/jpeg":
      return "jpg";
    case "image/gif":
      return "gif";
    case "image/webp":
      return "webp";
    case "image/bmp":
      return "bmp";
    case "image/svg+xml":
      return "svg";
    case "image/tiff":
      return "tiff";
    default:
      return null;
  }
}

async function persistUserInputImageFile(args: {
  sessionId: string;
  mimeType: string;
  bytes: Buffer;
}): Promise<{ path: string }> {
  const context = requireExtensionContext();
  const dir = await ensureUserInputImageDir(context);
  const ext = imageExtFromMimeType(args.mimeType);
  if (!ext) throw new Error(`Unsupported image MIME type: ${args.mimeType}`);
  const fileName = `${sanitizeImageKey(`user-${args.sessionId}-${crypto.randomUUID()}`)}.${ext}`;
  const filePath = path.join(dir, fileName);
  await fs.writeFile(filePath, args.bytes);
  return { path: filePath };
}

async function cacheImageDataUrl(args: {
  prefix: string;
  dataUrl: string;
}): Promise<{ imageKey: string; mimeType: string; byteLength: number }> {
  const { mimeType, base64 } = parseDataUrl(args.dataUrl);
  const bytes = Buffer.from(base64, "base64");
  return await cacheImageBytes({ prefix: args.prefix, mimeType, bytes });
}

async function loadCachedImageBase64(imageKey: string): Promise<{
  mimeType: string;
  base64: string;
}> {
  const context = requireExtensionContext();
  const { metaPath, dataPath } = imageCachePaths(context, imageKey);
  const metaRaw = await fs.readFile(metaPath, "utf8");
  const meta = JSON.parse(metaRaw) as CachedImageMeta;
  if (!meta || typeof meta.mimeType !== "string") {
    throw new Error(`Invalid cached image meta: ${imageKey}`);
  }
  const data = await fs.readFile(dataPath);
  return { mimeType: meta.mimeType, base64: data.toString("base64") };
}

const HIDDEN_TAB_SESSIONS_KEY = "codexMine.hiddenTabSessions.v1";
const WORKSPACE_COLOR_OVERRIDES_KEY = "codexMine.workspaceColorOverrides.v1";
const LEGACY_RUNTIMES_KEY = "codexMine.sessionRuntime.v1";
const hiddenTabSessionIds = new Set<string>();
const unreadSessionIds = new Set<string>();
const WORKSPACE_COLOR_PALETTE = [
  "#1f6feb", // 青
  "#2ea043", // 緑
  "#d29922", // 黄
  "#db6d28", // オレンジ
  "#f85149", // 赤
  "#a371f7", // 紫
  "#ff7b72", // ピンク
  "#7ee787", // ミント
  "#ffa657", // アプリコット
  "#79c0ff", // 水色
  "#d2a8ff", // ラベンダー
  "#c9d1d9", // グレー
] as const;
let workspaceColorOverrides: Record<string, number> = {};
const mcpStatusByBackendKey = new Map<string, Map<string, string>>();
const cliVariantByBackendKey = new Map<
  string,
  "unknown" | "codex" | "codex-mine"
>();
const defaultTitleRe = /^(.*)\s+\([0-9a-f]{8}\)$/i;
type UiImageInput = { name: string; url: string };
type BackendImageInput =
  | { kind: "imageUrl"; url: string }
  | { kind: "localImage"; path: string };

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
  uiHydrationBlockedText: string | null;
  tokenUsage: ThreadTokenUsage | null;
  sending: boolean;
  reloading: boolean;
  compactInFlight: boolean;
  pendingCompactBlockId: string | null;
  pendingAssistantDeltas: Map<string, string>;
  pendingAssistantDeltaFlushTimer: NodeJS.Timeout | null;
  streamingAssistantItemIds: Set<string>;
  activeTurnId: string | null;
  pendingInterrupt: boolean;
  lastTurnStartedAtMs: number | null;
  lastTurnCompletedAtMs: number | null;
  v2NotificationsSeen: boolean;
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
let globalRateLimitStatusText: string | null = null;
let globalRateLimitStatusTooltip: string | null = null;
let customPrompts: CustomPromptSummary[] = [];
let sessionModelState: {
  model: string | null;
  provider: string | null;
  reasoning: string | null;
} = { model: null, provider: null, reasoning: null };
type ModelState = typeof sessionModelState;
const pendingModelFetchByBackend = new Map<string, Promise<void>>();
const PROMPTS_CMD_PREFIX = "prompts";
const loggedAgentScanErrors = new Set<string>();

export function activate(context: vscode.ExtensionContext): void {
  extensionContext = context;
  const output = vscode.window.createOutputChannel("Codex UI");
  outputChannel = output;
  output.appendLine(
    `[debug] Codex UI extension version=${String(context.extension.packageJSON.version || "")}`,
  );
  output.appendLine(`[debug] extensionPath=${context.extensionPath}`);
  void loadInitialModelState(output);

  sessionPanels = new SessionPanelManager(context);
  context.subscriptions.push(sessionPanels);

  sessions = new SessionStore();
  loadSessions(context, sessions);
  for (const s of sessions.listAll()) ensureRuntime(s.id);
  loadHiddenTabSessions(context);
  workspaceColorOverrides = loadWorkspaceColorOverrides(context);
  refreshCustomPromptsFromDisk();
  void cleanupLegacyRuntimeCache(context);

  backendManager = new BackendManager(output, sessions);
  backendManager.onBackendTerminated = (backendKey, info) =>
    handleBackendTerminated(backendKey, info);
  backendManager.onServerEvent = (backendKey, session, n) => {
    if (session) applyServerNotification(backendKey, session.id, n);
    else applyGlobalNotification(backendKey, n);
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
  backendManager.onAskUserQuestionRequest = async (session, req) => {
    if (!chatView) throw new Error("chatView is not initialized");
    if (!session || typeof session.id !== "string") {
      throw new Error("AskUserQuestion requires a valid session");
    }

    const params = (req as any).params as any;
    const callId = typeof params?.callId === "string" ? params.callId : null;
    const request = params?.request;
    if (!callId) throw new Error("AskUserQuestion missing callId");
    if (!request || typeof request !== "object") {
      throw new Error("AskUserQuestion missing request payload");
    }

    // Switch UI context to the requesting session so the prompt is visible.
    setActiveSession(session.id, { markRead: false });
    chatView.refresh();
    await showCodexMineViewContainer();
    // Ensure the webview is actually instantiated and visible.
    await vscode.commands.executeCommand("codexMine.chatView.focus");
    chatView.reveal();

    const response = await chatView.promptAskUserQuestion({
      requestKey: callId,
      request: request as AskUserQuestionRequest,
    });

    // Persist a concise summary in the chat history so the selection isn't lost
    // after the inline card is dismissed.
    const summaryText = formatAskUserQuestionSummary(
      request as AskUserQuestionRequest,
      response,
    );
    upsertBlock(session.id, {
      id: `askUserQuestion:${callId}`,
      type: "info",
      title: "AskUserQuestion",
      text: summaryText,
    });

    return response;
  };

  diffProvider = new DiffDocumentProvider();

  // NOTE: This is intentionally not contributed to the command palette in package.json.
  // It's a helper for local/dev workflows (e.g. taking docs screenshots) without requiring
  // an actual backend request.
  context.subscriptions.push(
    vscode.commands.registerCommand(
      "codexMine._dev.askUserQuestionDemo",
      async () => {
        if (!chatView) throw new Error("chatView is not initialized");
        await showCodexMineViewContainer();
        await vscode.commands.executeCommand("codexMine.chatView.focus");
        chatView.reveal();

        const response = await chatView.promptAskUserQuestion({
          requestKey: `demo:${Date.now()}`,
          request: {
            title: "Codex question",
            questions: [
              {
                id: "context",
                prompt: "Which context should I include?",
                type: "multi_select",
                allow_other: true,
                required: false,
                options: [
                  {
                    label: "Workspace files",
                    value: "files",
                    recommended: true,
                  },
                  { label: "Open editors", value: "editors" },
                  { label: "Terminal output", value: "terminal" },
                ],
              },
            ],
          },
        });
        void vscode.window.showInformationMessage(
          `AskUserQuestion demo result: ${JSON.stringify(response)}`,
        );
      },
    ),
  );
  context.subscriptions.push(
    vscode.workspace.registerTextDocumentContentProvider(
      "codex-mine-diff",
      diffProvider,
    ),
  );
  context.subscriptions.push(diffProvider);

  sessionTree = new SessionTreeDataProvider(
    context.extensionUri,
    sessions,
    (workspaceFolderUri) => colorIndexForWorkspaceFolderUri(workspaceFolderUri),
  );
  context.subscriptions.push(sessionTree);
  context.subscriptions.push(
    vscode.window.createTreeView("codexMine.sessionsView", {
      treeDataProvider: sessionTree,
    }),
  );

  chatView = new ChatViewProvider(
    context,
    () => buildChatState(),
    async (text, images = [], rewind = null) => {
      if (!backendManager) throw new Error("backendManager is not initialized");
      if (!sessions) throw new Error("sessions is not initialized");
      const bm = backendManager;

      const session = activeSessionId
        ? sessions.getById(activeSessionId)
        : null;
      if (!session) {
        void vscode.window.showErrorMessage("No session selected.");
        return;
      }

      const trimmed = text.trim();
      if (rewind && trimmed.startsWith("/")) {
        void vscode.window.showErrorMessage(
          "Rewind is not supported for slash commands.",
        );
        return;
      }
      if (trimmed.startsWith("/") && images.length > 0) {
        void vscode.window.showErrorMessage(
          "Slash commands do not support images yet.",
        );
        return;
      }
      if (trimmed.startsWith("/")) {
        const slashHandled = await handleSlashCommand(context, session, text);
        if (slashHandled) return;
      }

      const expanded = await expandMentions(session, text);
      if (!expanded.ok) {
        void vscode.window.showErrorMessage(expanded.error);
        return;
      }

      if (rewind) {
        if (!isMineSelectedForBackendKey(session.backendKey)) {
          void vscode.window.showErrorMessage(
            "Rewind は codex-mine 選択時のみ対応です。Settings (⚙) から codex-mine を選択し、必要ならバックエンドを再起動してください。",
          );
          return;
        }

        const folder = resolveWorkspaceFolderForSession(session);
        if (!folder) {
          void vscode.window.showErrorMessage(
            "WorkspaceFolder not found for session.",
          );
          return;
        }
        try {
          await ensureBackendMatchesConfiguredCli(folder, "mineFeature");
        } catch (err) {
          outputChannel?.appendLine(
            `[rewind] Backend configuration check failed: ${String(err)}`,
          );
          void vscode.window.showErrorMessage(
            "Backend configuration check failed. See Codex UI output channel.",
          );
          return;
        }

        const turnIndexRaw = (rewind as any).turnIndex;
        const turnIndex =
          typeof turnIndexRaw === "number" && Number.isFinite(turnIndexRaw)
            ? Math.trunc(turnIndexRaw)
            : 0;

        if (!turnIndex || turnIndex < 1) {
          void vscode.window.showErrorMessage("Invalid rewind request.");
          return;
        }

        const rt = ensureRuntime(session.id);
        if (rt.sending) {
          void vscode.window.showErrorMessage(
            "Cannot rewind while a turn is in progress.",
          );
          return;
        }

        const rewindBlockId = newLocalId("info");

        const runRewind = async (): Promise<void> => {
          upsertBlock(session.id, {
            id: rewindBlockId,
            type: "info",
            title: "Rewind requested",
            text: `Rewinding to turn #${turnIndex}…`,
          });
          chatView?.refresh();

          const resumed = await withTimeout(
            "thread/resume",
            bm.resumeSession(session),
            REWIND_STEP_TIMEOUT_MS,
          );
          const totalTurns = Array.isArray(resumed.thread.turns)
            ? resumed.thread.turns.length
            : 0;
          const numTurns = totalTurns - (turnIndex - 1);
          if (!Number.isFinite(numTurns) || numTurns < 1) {
            throw new Error(
              `Invalid rewind request: turnIndex=${turnIndex} totalTurns=${totalTurns}`,
            );
          }

          const rolledBack = await withTimeout(
            "thread/rollback",
            bm.threadRollback(session, { numTurns }),
            REWIND_STEP_TIMEOUT_MS,
          );
          hydrateRuntimeFromThread(session.id, rolledBack.thread, { force: true });

          upsertBlock(session.id, {
            id: rewindBlockId,
            type: "info",
            title: "Rewind completed",
            text: `Rewound to turn #${turnIndex}.`,
          });
          chatView?.refresh();
        };

        try {
          await runRewind();
        } catch (err) {
          const errText = formatUnknownError(err);
          outputChannel?.appendLine(
            `[rewind] Failed: threadId=${session.threadId} turnIndex=${turnIndex} err=${errText}`,
          );
          upsertBlock(session.id, {
            id: rewindBlockId,
            type: "error",
            title: "Rewind failed",
            text: `${errText}\n\nCheck 'Codex UI' output channel for backend logs.`,
          });
          chatView?.refresh();
          return;
        }
      }

      await sendUserInput(
        session,
        expanded.text,
        images,
        getSessionModelState(),
      );
    },
    async (session) => {
      if (!backendManager) throw new Error("backendManager is not initialized");
      return await backendManager.listAccounts(session);
    },
    async (session) => {
      if (!backendManager) throw new Error("backendManager is not initialized");
      return await backendManager.readAccount(session);
    },
    async (session, params) => {
      if (!backendManager) throw new Error("backendManager is not initialized");
      return await backendManager.switchAccount(session, params);
    },
    async (session) => {
      if (!backendManager) throw new Error("backendManager is not initialized");
      return await backendManager.logoutAccount(session);
    },
    async ({ variant, restartMode }) => {
      if (!backendManager) throw new Error("backendManager is not initialized");
      if (!outputChannel) throw new Error("outputChannel is not initialized");

      const cfg = vscode.workspace.getConfiguration("codexMine");
      const mineCmd =
        cfg.get<string>("cli.commands.codexMine") ??
        cfg.get<string>("cli.commands.mine") ??
        "codex-mine";

      const next = normalizeCliVariant(variant);
      if (next === "codex-mine") {
        const mineProbe = await probeCliVersion(mineCmd);
        const mineDetected = mineProbe.ok && mineProbe.version.includes("-mine.");
        if (!mineDetected) {
          throw new Error(
            mineProbe.ok
              ? `codex-mine not detected (found: ${mineProbe.version})`
              : `codex-mine not detected (${mineProbe.error})`,
          );
        }
      }

      await cfg.update("cli.variant", next, vscode.ConfigurationTarget.Global);

      const folders = vscode.workspace.workspaceFolders ?? [];
      if (restartMode === "restartAll") {
        for (const f of folders) {
          await ensureBackendMatchesConfiguredCli(f, "newSession", false);
        }
      } else if (restartMode === "forceRestartAll") {
        for (const f of folders) {
          await backendManager.restartForWorkspaceFolder(f);
          if (next !== "auto") {
            cliVariantByBackendKey.set(
              f.uri.toString(),
              next === "codex-mine" ? "codex-mine" : "codex",
            );
          }
        }
      }
    },
    async (sessionId, query, cancellationToken) => {
      if (!backendManager) throw new Error("backendManager is not initialized");
      if (!sessions) throw new Error("sessions is not initialized");
      const session = sessions.getById(sessionId);
      if (!session) throw new Error(`Session not found: ${sessionId}`);
      const res = await backendManager.fuzzyFileSearchForSession(
        session,
        query,
        cancellationToken,
      );
      return res.files.map((f) => String(f.path || "").replace(/\\\\/g, "/"));
    },
    async (sessionId) => {
      if (!sessions) throw new Error("sessions is not initialized");
      const session = sessions.getById(sessionId);
      if (!session) throw new Error(`Session not found: ${sessionId}`);
      const folder = resolveWorkspaceFolderForSession(session);
      if (!folder)
        throw new Error(`WorkspaceFolder not found for session: ${sessionId}`);

      if (!isMineSelectedForBackendKey(session.backendKey)) return [];

      const { agents } = await listAgentsFromDisk(folder.uri.fsPath);
      return agents
        .map((a) => String(a.name || "").trim())
        .filter((name) => name.length > 0);
    },
    async (sessionId) => {
      if (!backendManager) throw new Error("backendManager is not initialized");
      if (!sessions) throw new Error("sessions is not initialized");
      const session = sessions.getById(sessionId);
      if (!session) throw new Error(`Session not found: ${sessionId}`);

      const entries = await backendManager.listSkillsForSession(session);
      const entry = entries[0] ?? null;
      const skills = entry?.skills ?? [];
      return skills.map((s) => ({
        name: s.name,
        description: s.description,
        scope: s.scope,
        path: s.path,
      }));
    },
    async (imageKey) => {
      return await loadCachedImageBase64(imageKey);
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
    (message: string) => {
      output.appendLine(`[ui] ${message}`);
    },
    (message: string) => {
      void vscode.window.showErrorMessage(message);
      const session = activeSessionId
        ? sessions?.getById(activeSessionId)
        : null;
      if (!session) return;
      upsertBlock(session.id, {
        id: newLocalId("error"),
        type: "error",
        title: "UI Error",
        text: message,
      });
      chatView?.refresh();
      schedulePersistRuntime(session.id);
    },
  );
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider(
      ChatViewProvider.viewType,
      chatView,
    ),
  );

  context.subscriptions.push(output);

  // Best-effort: restore the last active session after extension reloads so users
  // don't need to re-select it from Sessions every time. This performs at most one
  // `thread/resume` and avoids any background rehydration while switching tabs.
  void (async () => {
    if (!backendManager) return;
    if (!sessions) return;
    if (activeSessionId) return;
    const lastSessionId = context.workspaceState.get<string>(
      LAST_ACTIVE_SESSION_KEY,
    );
    if (typeof lastSessionId !== "string" || !lastSessionId) return;
    const session = sessions.getById(lastSessionId);
    if (!session) return;
    try {
      // Ensure the view is visible so the user sees the restored conversation.
      await showCodexMineViewContainer();
      setActiveSession(session.id, { markRead: false });
      const res = await backendManager.resumeSession(session);
      void ensureModelsFetched(session);
      hydrateRuntimeFromThread(session.id, res.thread);
      setActiveSession(session.id);
      refreshCustomPromptsFromDisk();
    } catch (err) {
      output.appendLine(
        `[startup] Failed to restore last sessionId=${lastSessionId}: ${String(err)}`,
      );
    }
  })();

  context.subscriptions.push(
    vscode.commands.registerCommand("codexMine.startBackend", async () => {
      if (!backendManager) throw new Error("backendManager is not initialized");

      const folder = await pickWorkspaceFolder();
      if (!folder) return;

      await backendManager.startForWorkspaceFolder(folder);
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("codexMine.clearRuntimeCache", async () => {
      if (!extensionContext) throw new Error("extensionContext is not set");
      if (!sessions) throw new Error("sessions is not initialized");

      // This only clears in-memory state. Conversation history is re-hydrated
      // from `thread/resume` (backed by ~/.codex/sessions) when sessions are opened.
      await cleanupLegacyRuntimeCache(extensionContext);

      // Clear in-memory runtimes for existing sessions.
      for (const s of sessions.listAll()) {
        const rt = ensureRuntime(s.id);
        rt.blocks = [];
        rt.latestDiff = null;
        rt.statusText = null;
        rt.lastTurnStartedAtMs = null;
        rt.lastTurnCompletedAtMs = null;
        rt.sending = false;
        rt.blockIndexById.clear();
        rt.legacyPatchTargetByCallId.clear();
        rt.legacyWebSearchTargetByCallId.clear();
        rt.pendingApprovals.clear();
        rt.approvalResolvers.clear();
      }

      unreadSessionIds.clear();
      chatView?.refresh();

      void vscode.window.showInformationMessage(
        "Cleared Codex UI in-memory runtime cache. Reopen a session to re-hydrate history.",
      );
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "codexMine.pickWorkspaceColor",
      async (args?: unknown) => {
        const workspaceFolderUri =
          typeof (args as any)?.workspaceFolderUri === "string"
            ? String((args as any).workspaceFolderUri)
            : "";
        if (!workspaceFolderUri) {
          void vscode.window.showErrorMessage(
            "workspaceFolderUri が不正です。",
          );
          return;
        }

        let placeHolder = workspaceFolderUri;
        try {
          placeHolder = vscode.Uri.parse(workspaceFolderUri).fsPath;
        } catch {
          // Keep raw URI string.
        }

        const items: Array<{
          label: string;
          description: string;
          idx: number | null;
        }> = [
          {
            label: "自動",
            description: "ハッシュから自動で色を割り当て",
            idx: null,
          },
          ...WORKSPACE_COLOR_PALETTE.map((hex, idx) => {
            const name =
              idx === 0
                ? "青"
                : idx === 1
                  ? "緑"
                  : idx === 2
                    ? "黄"
                    : idx === 3
                      ? "オレンジ"
                      : idx === 4
                        ? "赤"
                        : idx === 5
                          ? "紫"
                          : idx === 6
                            ? "ピンク"
                            : idx === 7
                              ? "ミント"
                              : idx === 8
                                ? "アプリコット"
                                : idx === 9
                                  ? "水色"
                                  : idx === 10
                                    ? "ラベンダー"
                                    : "グレー";
            return {
              label: name,
              description: String(hex),
              idx,
            };
          }),
        ];

        const picked = await vscode.window.showQuickPick(items, {
          title: "プロジェクト色を選択",
          placeHolder,
        });
        if (!picked) return;

        const next = { ...workspaceColorOverrides };
        if (picked.idx === null) delete next[workspaceFolderUri];
        else next[workspaceFolderUri] = picked.idx;

        workspaceColorOverrides = next;
        await context.globalState.update(WORKSPACE_COLOR_OVERRIDES_KEY, next);
        sessionTree?.refresh();
        chatView?.refresh();
      },
    ),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "codexMine.newSession",
      async (args?: unknown) => {
        if (!backendManager)
          throw new Error("backendManager is not initialized");

        const folderFromArgs = ((): vscode.WorkspaceFolder | null => {
          if (typeof args !== "object" || args === null) return null;
          const anyArgs = args as Record<string, unknown>;
          const forcePickFolder = anyArgs["forcePickFolder"];
          if (typeof forcePickFolder === "boolean" && forcePickFolder) {
            return null;
          }
          const uriRaw = anyArgs["workspaceFolderUri"];
          if (typeof uriRaw !== "string" || !uriRaw) return null;
          try {
            const uri = vscode.Uri.parse(uriRaw);
            return vscode.workspace.getWorkspaceFolder(uri) ?? null;
          } catch {
            return null;
          }
        })();

        const folder =
          folderFromArgs ??
          (typeof args === "object" &&
          args !== null &&
          (args as Record<string, unknown>)["forcePickFolder"] === true
            ? null
            : (() => {
                if (!sessions) return null;
                const active = activeSessionId
                  ? sessions.getById(activeSessionId)
                  : null;
                if (!active) return null;
                return resolveWorkspaceFolderForSession(active);
              })()) ??
          (await pickWorkspaceFolder());
        if (!folder) return;

        await ensureBackendMatchesConfiguredCli(folder, "newSession");
        const session = await backendManager.newSession(
          folder,
          getSessionModelState(),
        );
        setActiveSession(session.id);
        void ensureModelsFetched(session);
        await showCodexMineViewContainer();
      },
    ),
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

      const session = activeSessionId
        ? sessions.getById(activeSessionId)
        : null;
      if (!session) return;

      const rt = ensureRuntime(session.id);
      let turnId =
        rt.activeTurnId ?? backendManager.getActiveTurnId(session.threadId);

      if (!turnId && rt.sending) {
        rt.pendingInterrupt = true;
        output.appendLine(
          "[turn] Interrupt requested before turnId is known; will interrupt on turn/started.",
        );
        chatView?.refresh();
        schedulePersistRuntime(session.id);
        return;
      }

      if (!turnId) {
        upsertBlock(session.id, {
          id: newLocalId("info"),
          type: "info",
          title: "Nothing to interrupt",
          text: "Interrupt was requested, but no in-progress turn was found for this session.",
        });
        chatView?.refresh();
        schedulePersistRuntime(session.id);
        return;
      }

      output.appendLine(`[turn] Interrupt requested: turnId=${turnId}`);
      void backendManager.interruptTurn(session, turnId).catch((err) => {
        output.appendLine(`[turn] Failed to interrupt: ${String(err)}`);
        upsertBlock(session.id, {
          id: newLocalId("error"),
          type: "error",
          title: "Interrupt failed",
          text: String(err),
        });
        chatView?.refresh();
      });
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("codexMine.reloadSession", async () => {
      if (!backendManager) throw new Error("backendManager is not initialized");
      if (!sessions) throw new Error("sessions is not initialized");

      const session = activeSessionId
        ? sessions.getById(activeSessionId)
        : null;
      if (!session) return;

      const folder = resolveWorkspaceFolderForSession(session);
      if (!folder) {
        void vscode.window.showErrorMessage(
          "WorkspaceFolder not found for session.",
        );
        return;
      }
      try {
        await ensureBackendMatchesConfiguredCli(folder, "mineFeature");
      } catch (err) {
        output.appendLine(
          `[session] Reload backend configuration check failed: ${String(err)}`,
        );
        void vscode.window.showErrorMessage(
          "Backend configuration check failed. See Codex UI output channel.",
        );
        return;
      }

      const rt = ensureRuntime(session.id);
      if (rt.sending) {
        void vscode.window.showErrorMessage(
          "Cannot reload while a turn is in progress.",
        );
        return;
      }
      if (rt.reloading) return;
      rt.reloading = true;
      rt.uiHydrationBlockedText = null;
      chatView?.refresh();
      chatView?.toast("info", "Reloading session…");

      output.appendLine(
        `[session] Reload requested: threadId=${session.threadId}`,
      );
      try {
        const res = await backendManager.reloadSession(
          session,
          getSessionModelState(),
        );
        hydrateRuntimeFromThread(session.id, res.thread, { force: true });
        schedulePersistRuntime(session.id);
        chatView?.refresh();
        chatView?.toast("success", "Reload completed.");
      } catch (err) {
        output.appendLine(`[session] Reload failed: ${String(err)}`);
        upsertBlock(session.id, {
          id: newLocalId("error"),
          type: "error",
          title: "Reload failed",
          text: String(err),
        });
        chatView?.refresh();
        chatView?.toast("error", "Reload failed.");
      } finally {
        rt.reloading = false;
        chatView?.refresh();
      }
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("codexMine.debug.stressUi", async () => {
      if (!sessions) throw new Error("sessions is not initialized");
      if (!outputChannel) throw new Error("outputChannel is not initialized");
      if (!chatView) throw new Error("chatView is not initialized");
      const output = outputChannel;
      const view = chatView;

      const session = activeSessionId
        ? sessions.getById(activeSessionId)
        : null;
      if (!session) {
        void vscode.window.showErrorMessage("No session selected.");
        return;
      }

      const totalRaw = await vscode.window.showInputBox({
        title: "Stress UI streaming",
        prompt: "Total characters to append",
        value: "2000000",
        validateInput: (v) => {
          const n = Number(v);
          if (!Number.isFinite(n) || n <= 0) return "Enter a positive number";
          return undefined;
        },
      });
      if (!totalRaw) return;
      const totalChars = Math.floor(Number(totalRaw));

      const chunkRaw = await vscode.window.showInputBox({
        title: "Stress UI streaming",
        prompt: "Chunk size (characters per tick)",
        value: "2000",
        validateInput: (v) => {
          const n = Number(v);
          if (!Number.isFinite(n) || n <= 0) return "Enter a positive number";
          if (n > 200_000) return "Too large; keep it <= 200000";
          return undefined;
        },
      });
      if (!chunkRaw) return;
      const chunkChars = Math.floor(Number(chunkRaw));

      const intervalRaw = await vscode.window.showInputBox({
        title: "Stress UI streaming",
        prompt: "Interval between ticks (ms)",
        value: "0",
        validateInput: (v) => {
          const n = Number(v);
          if (!Number.isFinite(n) || n < 0)
            return "Enter 0 or a positive number";
          if (n > 10_000) return "Too large; keep it <= 10000";
          return undefined;
        },
      });
      if (intervalRaw === undefined) return;
      const intervalMs = Math.floor(Number(intervalRaw));

      // Cancel any existing job.
      if (stressUiJob) {
        stressUiJob.cancel();
        stressUiJob = null;
      }

      const rt = ensureRuntime(session.id);
      const blockId = `debug:stressUi:${session.id}`;
      const block = getOrCreateBlock(rt, blockId, () => ({
        id: blockId,
        type: "assistant",
        text: "",
        streaming: true,
      }));
      if (block.type === "assistant") {
        block.text = "";
        (block as any).streaming = true;
      }
      view.postBlockUpsert(session.id, block);

      const baseChunk =
        chunkChars <= 1 ? "A" : `${"A".repeat(chunkChars - 1)}\n`;
      let remaining = totalChars;
      let cancelled = false;

      output.appendLine(
        `[debug] stressUi started: sessionId=${session.id} totalChars=${totalChars} chunkChars=${chunkChars} intervalMs=${intervalMs}`,
      );

      const tick = (): void => {
        if (cancelled) return;
        const nextLen = Math.min(remaining, baseChunk.length);
        const delta =
          nextLen === baseChunk.length
            ? baseChunk
            : baseChunk.slice(0, nextLen);
        remaining -= delta.length;

        const b = getOrCreateBlock(rt, blockId, () => ({
          id: blockId,
          type: "assistant",
          text: "",
          streaming: true,
        }));
        if (b.type === "assistant") {
          b.text += delta;
          (b as any).streaming = remaining > 0;
        }
        view.postBlockAppend(session.id, blockId, "assistantText", delta, {
          streaming: remaining > 0,
        });

        if (remaining <= 0) {
          output.appendLine(
            `[debug] stressUi completed: sessionId=${session.id}`,
          );
          stressUiJob = null;
          return;
        }
        setTimeout(tick, intervalMs);
      };

      tick();

      stressUiJob = {
        sessionId: session.id,
        cancel: () => {
          cancelled = true;
          const b = getOrCreateBlock(rt, blockId, () => ({
            id: blockId,
            type: "assistant",
            text: "",
            streaming: false,
          }));
          if (b.type === "assistant") (b as any).streaming = false;
          view.postBlockUpsert(session.id, b);
          output.appendLine(
            `[debug] stressUi cancelled: sessionId=${session.id}`,
          );
        },
      };
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "codexMine.debug.stopStressUi",
      async () => {
        if (!outputChannel) throw new Error("outputChannel is not initialized");
        if (!stressUiJob) {
          void vscode.window.showInformationMessage(
            "No UI stress job is running.",
          );
          return;
        }
        stressUiJob.cancel();
        stressUiJob = null;
      },
    ),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("codexMine.showStatus", async () => {
      if (!backendManager) throw new Error("backendManager is not initialized");
      if (!sessions) throw new Error("sessions is not initialized");

      const session = activeSessionId
        ? sessions.getById(activeSessionId)
        : null;
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
        output.appendLine(
          `[status] Failed to read rate limits: ${String(err)}`,
        );
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
        if (
          accountLine &&
          accountLine.includes("(") &&
          accountLine.includes(")")
        ) {
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

      const limitLines = rateLimits ? formatRateLimitLines(rateLimits) : [];

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
        .filter(
          (v): v is string => typeof v === "string" && v.trim().length > 0,
        )
        .join("\n");

      upsertBlock(session.id, {
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
    vscode.commands.registerCommand("codexMine.switchAccount", async () => {
      if (!backendManager) throw new Error("backendManager is not initialized");
      if (!sessions) throw new Error("sessions is not initialized");
      const bm = backendManager;

      const session = activeSessionId ? sessions.getById(activeSessionId) : null;
      if (!session) {
        void vscode.window.showErrorMessage("No session selected.");
        return;
      }

      const list = await bm.listAccounts(session);
      const active = list.activeAccount ?? null;

      type PickItem =
        | (vscode.QuickPickItem & { itemKind: "account"; name: string })
        | (vscode.QuickPickItem & { itemKind: "create" });

      const items: PickItem[] = list.accounts.map((a) => {
        const description =
          a.kind === "chatgpt"
            ? a.email
              ? `chatgpt (${a.email})`
              : "chatgpt"
            : a.kind === "apiKey"
              ? "apiKey"
              : undefined;

        return {
          itemKind: "account",
          name: a.name,
          label: a.name,
          description,
          detail: active === a.name ? "active" : undefined,
        };
      });
      items.push({
        itemKind: "create",
        label: "+ Create new account…",
        description: "Use [A-Za-z0-9_-], 1..64 chars",
      });

      const picked = await vscode.window.showQuickPick(items, {
        title: "Switch account",
        placeHolder: "Select an account",
      });
      if (!picked) return;

      const validateName = (name: string): string | null => {
        const trimmed = name.trim();
        if (trimmed.length === 0) return "Account name cannot be empty.";
        if (trimmed.length > 64) return "Account name is too long (max 64 chars).";
        if (!/^[A-Za-z0-9_-]+$/.test(trimmed))
          return "Invalid account name. Use only [A-Za-z0-9_-].";
        return null;
      };

      const doSwitch = async (
        name: string,
        createIfMissing: boolean,
      ): Promise<void> => {
        await bm.switchAccount(session, { name, createIfMissing });
        void vscode.window.showInformationMessage(
          `Switched active account to ${name}.`,
        );
      };

      if (picked.itemKind === "create") {
        const name = await vscode.window.showInputBox({
          title: "Create account",
          prompt: "Account name",
          placeHolder: "e.g. work, personal, team_a",
          validateInput: (value) => validateName(value) ?? undefined,
        });
        if (!name) return;
        const trimmed = name.trim();
        const err = validateName(trimmed);
        if (err) {
          void vscode.window.showErrorMessage(err);
          return;
        }
        await doSwitch(trimmed, true);
        return;
      }

      await doSwitch(picked.name, false);
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "codexMine.showSkills",
      async (args?: unknown) => {
        if (!backendManager)
          throw new Error("backendManager is not initialized");
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
      },
    ),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "codexMine.showAgents",
      async (args?: unknown) => {
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
          void vscode.window.showErrorMessage(
            "WorkspaceFolder not found for session.",
          );
          return;
        }

        if (!isMineSelectedForBackendKey(session.backendKey)) {
          void vscode.window.showInformationMessage(
            "Agents are available only when running codex-mine. Click Settings (⚙) and select codex-mine, then restart the backend.",
          );
          return;
        }

        const { agents, errors } = await listAgentsFromDisk(folder.uri.fsPath);
        if (errors.length > 0) {
          output.appendLine(`[agents] cwd=${folder.uri.fsPath}`);
          for (const e of errors) output.appendLine(`[agents] ${e}`);
        }

        try {
          // Ensure the backend is configured as codex-mine (purely for UX; listing is from disk).
          await ensureBackendMatchesConfiguredCli(folder, "agents");
        } catch (err) {
          output.appendLine(
            `[agents] Backend configuration check failed: ${String(err)}`,
          );
          void vscode.window.showErrorMessage("Failed to list agents.");
          return;
        }

        if (agents.length === 0) {
          const msg =
            errors.length > 0
              ? "No agents found (some agent files failed to load)."
              : "No agents found. Add <git root>/.codex/agents/<name>.md or $CODEX_HOME/agents/<name>.md, and ensure [agents].sources includes the desired locations.";
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
      },
    ),
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
      const current = normalizeCliVariant(
        cfg.get<string>("cli.variant") ?? "auto",
      );

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

      await cfg.update(
        "cli.variant",
        picked.it.variant,
        vscode.ConfigurationTarget.Global,
      );

      const restart = await vscode.window.showInformationMessage(
        "CLI setting updated. Restart running backends now to apply?",
        "Restart all",
        "Force restart all",
        "Later",
      );
      if (restart === "Restart all") {
        const folders = vscode.workspace.workspaceFolders ?? [];
        for (const f of folders) {
          await ensureBackendMatchesConfiguredCli(f, "newSession");
        }
      } else if (restart === "Force restart all") {
        const folders = vscode.workspace.workspaceFolders ?? [];
        for (const f of folders) {
          await backendManager.restartForWorkspaceFolder(f);
          if (picked.it.variant !== "auto") {
            cliVariantByBackendKey.set(
              f.uri.toString(),
              picked.it.variant === "codex-mine" ? "codex-mine" : "codex",
            );
          }
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
            { label: "Copy Session ID", action: "copySessionId" as const },
            { label: "Open in Editor Tab", action: "openPanel" as const },
            { label: "Close Tab (Hide)", action: "hide" as const },
          ],
          { title: session.title },
        );
        if (!picked) return;

        if (picked.action === "copySessionId") {
          await vscode.commands.executeCommand("codexMine.copySessionId", {
            sessionId: session.id,
          });
          return;
        }

        if (picked.action === "rename") {
          await vscode.commands.executeCommand("codexMine.renameSession", {
            sessionId: session.id,
          });
          return;
        }

        if (picked.action === "openPanel") {
          await vscode.commands.executeCommand("codexMine.openSessionPanel", {
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
        unreadSessionIds.delete(session.id);
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
      "codexMine.copySessionId",
      async (args?: unknown) => {
        if (!sessions) throw new Error("sessions is not initialized");
        const session = parseSessionArg(args, sessions);
        if (!session) {
          void vscode.window.showErrorMessage("Session not found.");
          return;
        }

        await vscode.env.clipboard.writeText(session.id);
        void vscode.window.showInformationMessage("Copied session ID.");
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

        const res = await backendManager.resumeSession(session);
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
      "codexMine.openSessionPanel",
      async (args?: unknown) => {
        if (!backendManager)
          throw new Error("backendManager is not initialized");
        if (!sessions) throw new Error("sessions is not initialized");
        if (!sessionPanels) throw new Error("sessionPanels is not initialized");

        const session = parseSessionArg(args, sessions);
        if (!session) {
          void vscode.window.showErrorMessage("Session not found.");
          return;
        }

        const res = await backendManager.resumeSession(session);
        void ensureModelsFetched(session);
        hydrateRuntimeFromThread(session.id, res.thread);
        setActiveSession(session.id);

        const rt = ensureRuntime(session.id);
        sessionPanels.open(session, {
          blocks: rt.blocks,
          latestDiff: rt.latestDiff,
        });
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

        // Session switching should be a pure UI operation. However, after a reload the UI may not
        // have hydrated blocks for a session yet. In that case, treat the click as an explicit
        // "open" and resume once (only if nothing is currently running).
        setActiveSession(session.id, { markRead: false });
        await showCodexMineViewContainer();

        const rt = ensureRuntime(session.id);
        if (hasConversationBlocks(rt)) {
          rt.uiHydrationBlockedText = null;
          setActiveSession(session.id);
          return;
        }

        const anyRunning = [...runtimeBySessionId.values()].some(
          (r) =>
            r.sending ||
            r.activeTurnId !== null ||
            r.streamingAssistantItemIds.size > 0 ||
            r.pendingApprovals.size > 0,
        );
        if (anyRunning) {
          rt.uiHydrationBlockedText =
            "This session has not been loaded in the UI yet. Wait for the running session to finish, then use Reload/Resume to load history.";
          setActiveSession(session.id);
          return;
        }

        try {
          const res = await backendManager.resumeSession(session);
          void ensureModelsFetched(session);
          hydrateRuntimeFromThread(session.id, res.thread);
          rt.uiHydrationBlockedText = null;
          setActiveSession(session.id);
        } catch (err) {
          outputChannel?.appendLine(
            `[selectSession] Failed to hydrate sessionId=${session.id}: ${
              err instanceof Error ? (err.stack ?? err.message) : String(err)
            }`,
          );
          upsertBlock(session.id, {
            id: newLocalId("error"),
            type: "error",
            title: "Failed to load session",
            text: String(err),
          });
          chatView?.refresh();
          setActiveSession(session.id);
        }
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
          void vscode.window.showErrorMessage("No session selected.");
          return;
        }

        const next = await vscode.window.showInputBox({
          title: "Codex UI: Rename session",
          value: active.title,
          prompt: "Change the title shown in the chat tabs and Sessions list.",
          validateInput: (v) => (v.trim() ? null : "Title cannot be empty."),
        });
        if (next === undefined) return;

        const renamed = sessions.rename(active.id, next.trim());
        if (renamed) sessionPanels?.updateTitle(renamed.id, renamed.title);
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

function handleBackendTerminated(
  backendKey: string,
  info: BackendTermination,
): void {
  if (!sessions) return;

  if (mcpStatusByBackendKey.delete(backendKey)) {
    updateThreadStartedBlocks();
  }

  const affectedSessions = sessions.list(backendKey);
  if (affectedSessions.length === 0) return;

  const folderLabel = (() => {
    try {
      return vscode.Uri.parse(backendKey).fsPath;
    } catch {
      return backendKey;
    }
  })();

  outputChannel?.appendLine(
    `[backend] terminated: cwd=${folderLabel} reason=${info.reason} code=${info.code ?? "null"} signal=${info.signal ?? "null"}`,
  );

  const backendHash = crypto
    .createHash("sha1")
    .update(backendKey)
    .digest("hex")
    .slice(0, 10);
  const title = info.reason === "exit" ? "Backend exited" : "Backend stopped";
  const detailParts: string[] = [`cwd=${folderLabel}`, `reason=${info.reason}`];
  if (info.code !== null) detailParts.push(`code=${info.code}`);
  if (info.signal !== null) detailParts.push(`signal=${info.signal}`);
  detailParts.push(`at=${new Date().toISOString()}`);
  upsertGlobal({
    id: `global:backendTerminated:${backendHash}`,
    type: info.reason === "exit" ? "error" : "info",
    title,
    text: detailParts.join(" • "),
  });

  for (const s of affectedSessions) {
    const rt = ensureRuntime(s.id);
    const wasRunning =
      rt.sending ||
      rt.activeTurnId !== null ||
      rt.streamingAssistantItemIds.size > 0 ||
      rt.pendingApprovals.size > 0;

    rt.sending = false;
    rt.lastTurnCompletedAtMs = Date.now();
    rt.activeTurnId = null;
    rt.pendingInterrupt = false;

    for (const id of rt.streamingAssistantItemIds) {
      const idx = rt.blockIndexById.get(id);
      if (idx === undefined) continue;
      const b = rt.blocks[idx];
      if (b && b.type === "assistant") (b as any).streaming = false;
    }
    rt.streamingAssistantItemIds.clear();

    // Any approval requests are now stale because the backend process is gone.
    for (const resolve of rt.approvalResolvers.values()) resolve("cancel");
    rt.approvalResolvers.clear();
    rt.pendingApprovals.clear();

    if (wasRunning && info.reason === "exit") {
      upsertBlock(s.id, {
        id: newLocalId("error"),
        type: "error",
        title: "Backend exited",
        text:
          `The backend process for this workspace folder exited. ` +
          `You may need to restart the backend and resume this session.`,
      });
    }

    schedulePersistRuntime(s.id);
  }

  chatView?.refresh();
}

export function deactivate(): void {
  backendManager?.dispose();
  backendManager = null;
  sessions = null;
  sessionTree = null;
  diffProvider = null;
  chatView = null;
  sessionPanels = null;
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
  await sendUserInput(session, text, [], getSessionModelState());
}

async function sendUserInput(
  session: Session,
  text: string,
  images: UiImageInput[],
  modelState: ModelState | null,
): Promise<void> {
  if (!backendManager) throw new Error("backendManager is not initialized");
  const rt = ensureRuntime(session.id);
  rt.sending = true;
  rt.pendingInterrupt = false;
  const backendImages: BackendImageInput[] = [];
  const trimmed = text.trim();
  if (trimmed) {
    upsertBlock(session.id, { id: newLocalId("user"), type: "user", text });
    sessionPanels?.addUserMessage(session.id, text);
  }
  if (images.length > 0) {
    const galleryImages: Array<{
      title: string;
      src: string;
      imageKey: string;
      mimeType: string;
      byteLength: number;
      autoLoad?: boolean;
      alt: string;
      caption: string | null;
    }> = [];
    const errors: string[] = [];

    for (let i = 0; i < images.length; i++) {
      const img = images[i]!;
      const rawName = String(img.name || "").trim();
      const name = rawName || `image-${i + 1}`;
      try {
        const { mimeType, base64 } = parseDataUrl(img.url);
        const bytes = Buffer.from(base64, "base64");
        const saved = await cacheImageBytes({
          prefix: `user-${session.id}`,
          mimeType,
          bytes,
        });
        const persisted = await persistUserInputImageFile({
          sessionId: session.id,
          mimeType,
          bytes,
        });
        galleryImages.push({
          title: name,
          src: "",
          imageKey: saved.imageKey,
          mimeType: saved.mimeType,
          byteLength: saved.byteLength,
          autoLoad: true,
          alt: name,
          caption: name,
        });
        backendImages.push({ kind: "localImage", path: persisted.path });
      } catch (err) {
        errors.push(`${name}: ${String(err)}`);
        backendImages.push({ kind: "imageUrl", url: img.url });
      }
    }

    if (galleryImages.length > 0) {
      const title =
        galleryImages.length === 1
          ? "Attached 1 image"
          : `Attached ${galleryImages.length} images`;
      upsertBlock(session.id, {
        id: newLocalId("user-image-gallery"),
        type: "imageGallery",
        title,
        images: galleryImages,
        role: "user",
      });
      enforceSessionImageAutoloadLimit(rt);
    }

    if (errors.length > 0) {
      upsertBlock(session.id, {
        id: newLocalId("user-image-cache-error"),
        type: "error",
        title: "Failed to cache input image(s)",
        text: errors.join("\n"),
      });
    }

    outputChannel?.appendLine(
      `[images] input images: total=${images.length} cached=${galleryImages.length} errors=${errors.length}`,
    );
  }
  chatView?.refresh();
  schedulePersistRuntime(session.id);

  try {
    await backendManager.sendMessageWithModelAndImages(
      session,
      text,
      backendImages,
      modelState,
    );
  } catch (err) {
    outputChannel?.appendLine(
      `[send] Failed: sessionId=${session.id} threadId=${session.threadId} err=${String(err)}`,
    );
    rt.sending = false;
    rt.pendingInterrupt = false;
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
    await sendUserInput(
      session,
      expandedPrompt.text,
      [],
      getSessionModelState(),
    );
    return true;
  }
  if (expandedPrompt.kind === "error") {
    const rt = ensureRuntime(session.id);
    upsertBlock(session.id, {
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
    await vscode.commands.executeCommand("codexMine.newSession", {
      workspaceFolderUri: session.workspaceFolderUri,
    });
    return true;
  }
  if (cmd === "status") {
    if (arg) {
      upsertBlock(session.id, {
        id: newLocalId("statusError"),
        type: "error",
        title: "Slash command error",
        text: "/status does not take arguments.",
      });
      chatView?.refresh();
      schedulePersistRuntime(session.id);
      return true;
    }
    await vscode.commands.executeCommand("codexMine.showStatus");
    return true;
  }
  if (cmd === "init") {
    if (arg) {
      upsertBlock(session.id, {
        id: newLocalId("initError"),
        type: "error",
        title: "Slash command error",
        text: "/init does not take arguments.",
      });
      chatView?.refresh();
      schedulePersistRuntime(session.id);
      return true;
    }

    const folder = resolveWorkspaceFolderForSession(session);
    if (!folder) {
      upsertBlock(session.id, {
        id: newLocalId("initNoFolder"),
        type: "error",
        title: "Init failed",
        text: "このセッションに紐づく workspace folder が見つかりません。",
      });
      chatView?.refresh();
      schedulePersistRuntime(session.id);
      return true;
    }

    const initTarget = path.join(
      folder.uri.fsPath,
      DEFAULT_PROJECT_DOC_FILENAME,
    );

    let exists = false;
    try {
      await fs.stat(initTarget);
      exists = true;
    } catch (err) {
      if (
        typeof err === "object" &&
        err !== null &&
        "code" in err &&
        (err as any).code === "ENOENT"
      ) {
        exists = false;
      } else {
        throw err;
      }
    }

    if (exists) {
      upsertBlock(session.id, {
        id: newLocalId("initSkip"),
        type: "info",
        title: "Init skipped",
        text: `${DEFAULT_PROJECT_DOC_FILENAME} が既に存在するため、上書き防止のため /init をスキップしました。`,
      });
      chatView?.refresh();
      schedulePersistRuntime(session.id);
      return true;
    }

    const prompt = await getInitPrompt(context);
    await sendUserInput(session, prompt, [], getSessionModelState());
    return true;
  }
  if (cmd === "compact") {
    if (arg) {
      upsertBlock(session.id, {
        id: newLocalId("compactError"),
        type: "error",
        title: "Slash command error",
        text: "/compact does not take arguments.",
      });
      chatView?.refresh();
      schedulePersistRuntime(session.id);
      return true;
    }

    if (!isMineSelectedForBackendKey(session.backendKey)) {
      upsertBlock(session.id, {
        id: newLocalId("compactUnsupported"),
        type: "error",
        title: "Compact unsupported",
        text: "/compact は codex-mine 選択時のみ対応です。Settings (⚙) から codex-mine を選択し、必要ならバックエンドを再起動してください。",
      });
      chatView?.refresh();
      schedulePersistRuntime(session.id);
      return true;
    }

    if (!backendManager) throw new Error("backendManager is not initialized");
    {
      const folder = resolveWorkspaceFolderForSession(session);
      if (!folder) {
        upsertBlock(session.id, {
          id: newLocalId("compactNoFolder"),
          type: "error",
          title: "Compact failed",
          text: "このセッションに紐づく workspace folder が見つかりません。",
        });
        chatView?.refresh();
        schedulePersistRuntime(session.id);
        return true;
      }
      try {
        await ensureBackendMatchesConfiguredCli(folder, "mineFeature");
      } catch (err) {
        outputChannel?.appendLine(
          `[compact] Backend configuration check failed: ${String(err)}`,
        );
        upsertBlock(session.id, {
          id: newLocalId("compactConfigError"),
          type: "error",
          title: "Compact failed",
          text: "バックエンド設定の確認に失敗しました。Output Channel (Codex UI) を確認してください。",
        });
        chatView?.refresh();
        schedulePersistRuntime(session.id);
        return true;
      }
    }

    const rt = ensureRuntime(session.id);
    if (rt.compactInFlight) {
      upsertBlock(session.id, {
        id: newLocalId("compactAlreadyRunning"),
        type: "error",
        title: "Compact already running",
        text: "A previous /compact is still in progress.",
      });
      chatView?.refresh();
      schedulePersistRuntime(session.id);
      return true;
    }
    rt.sending = true;
    rt.compactInFlight = true;
    rt.pendingInterrupt = false;
    const pendingId = newLocalId("compacting");
    rt.pendingCompactBlockId = pendingId;
    upsertBlock(session.id, {
      id: pendingId,
      type: "divider",
      status: "inProgress",
      text: `${makeDividerLine("Context")}\n• Compacting…`,
    });
    chatView?.refresh();
    schedulePersistRuntime(session.id);

    try {
      await backendManager.threadCompact(session);
    } catch (err) {
      const errText =
        err instanceof Error
          ? err.message
          : typeof err === "string"
            ? err
            : JSON.stringify(err);
      outputChannel?.appendLine(
        `[compact] Failed: sessionId=${session.id} threadId=${session.threadId} err=${errText}`,
      );
      rt.sending = false;
      rt.compactInFlight = false;
      if (rt.pendingCompactBlockId) {
        upsertBlock(session.id, {
          id: rt.pendingCompactBlockId,
          type: "divider",
          status: "failed",
          text: `${makeDividerLine("Context")}\n• Compact failed`,
        });
      }
      rt.pendingCompactBlockId = null;
      rt.pendingInterrupt = false;
      upsertBlock(session.id, {
        id: newLocalId("error"),
        type: "error",
        title: "Compact failed",
        text: errText,
      });
      chatView?.refresh();
      schedulePersistRuntime(session.id);
    }
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
    const mineSelected = isMineSelectedForBackendKey(session.backendKey);
    upsertBlock(session.id, {
      id: newLocalId("help"),
      type: "system",
      title: "Help",
      text: [
        "Slash commands:",
        mineSelected
          ? "- /compact: Compact context"
          : "- /compact: (codex-mine 選択時のみ対応)",
        "- /new: New session",
        "- /init: Create AGENTS.md",
        "- /resume: Resume from history",
        "- /status: Show status",
        "- /diff: Open Latest Diff",
        "- /rename <title>: Rename session",
        "- /skills: Browse skills",
        mineSelected
          ? "- /agents: Browse agents"
          : "- /agents: Browse agents (codex-mine 選択時のみ対応)",
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
        error: "Cannot expand @selection because there is no active editor.",
      };
    }

    const folderFsPath = folder.uri.fsPath;
    const docFsPath = docUri.fsPath;
    let relPath = path.relative(folderFsPath, docFsPath);
    relPath = relPath.split(path.sep).join("/");
    if (!relPath || relPath.startsWith("../") || path.isAbsolute(relPath)) {
      return { ok: false, error: " file is outside the workspace." };
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

  // NOTE: Treat unresolved "@" tokens in copied text as plain text.
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

function shouldHideCommandText(
  command: string,
  actions: CommandAction[],
): boolean {
  const hasKnownAction = actions.some((a) => a.type !== "unknown");
  if (hasKnownAction) return false;
  return looksOpaqueCommandToken(command);
}

function looksOpaqueCommandToken(command: string): boolean {
  const t = command.trim();
  if (t.length < 40) return false;
  if (/\s/.test(t)) return false;
  // Likely base64 or similar opaque token (do not decode).
  if (!/^[A-Za-z0-9+/=]+$/.test(t)) return false;
  return true;
}

async function showCodexMineViewContainer(): Promise<void> {
  await vscode.commands.executeCommand("workbench.view.extension.codexMine");
}

function hasConversationBlocks(rt: SessionRuntime): boolean {
  return rt.blocks.some((b) => {
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
}

function setActiveSession(
  sessionId: string,
  opts?: { markRead?: boolean },
): void {
  const markRead = opts?.markRead ?? true;
  activeSessionId = sessionId;
  ensureRuntime(sessionId);
  if (markRead) unreadSessionIds.delete(sessionId);
  if (extensionContext) {
    void extensionContext.workspaceState.update(
      LAST_ACTIVE_SESSION_KEY,
      sessionId,
    );
  }
  // If a hidden tab session is selected (e.g. via Sessions tree), show it again.
  if (hiddenTabSessionIds.delete(sessionId)) {
    if (extensionContext) saveHiddenTabSessions(extensionContext);
  }
  const s = sessions ? sessions.getById(sessionId) : null;
  if (s) void ensureModelsFetched(s);
  chatView?.refresh();
  chatView?.syncBlocksForActiveSession();
}

function markUnreadSession(sessionId: string): void {
  if (activeSessionId === sessionId) return;
  if (unreadSessionIds.has(sessionId)) return;
  unreadSessionIds.add(sessionId);
}

function loadHiddenTabSessions(context: vscode.ExtensionContext): void {
  const raw = context.workspaceState.get<unknown>(HIDDEN_TAB_SESSIONS_KEY);
  if (!Array.isArray(raw)) return;
  for (const v of raw) {
    if (typeof v === "string" && v) hiddenTabSessionIds.add(v);
  }
}

function loadWorkspaceColorOverrides(
  context: vscode.ExtensionContext,
): Record<string, number> {
  const raw = context.globalState.get<unknown>(WORKSPACE_COLOR_OVERRIDES_KEY);
  if (!raw || typeof raw !== "object") return {};

  const out: Record<string, number> = {};
  for (const [k, v] of Object.entries(raw as Record<string, unknown>)) {
    if (typeof k !== "string" || !k) continue;
    if (typeof v !== "number" || !Number.isFinite(v)) continue;
    const idx = Math.trunc(v);
    if (idx < 0 || idx >= WORKSPACE_COLOR_PALETTE.length) continue;
    out[k] = idx;
  }
  return out;
}

function colorIndexForWorkspaceFolderUri(workspaceFolderUri: string): number {
  const override = workspaceColorOverrides[workspaceFolderUri];
  if (typeof override === "number") {
    const idx = Math.trunc(override);
    if (idx < 0 || idx >= WORKSPACE_COLOR_PALETTE.length) {
      throw new Error(
        `Invalid workspace color override: ${workspaceFolderUri}=${idx}`,
      );
    }
    return idx;
  }
  return fnv1a32(workspaceFolderUri) % WORKSPACE_COLOR_PALETTE.length;
}

function fnv1a32(input: string): number {
  let hash = 0x811c9dc5;
  for (let i = 0; i < input.length; i++) {
    hash ^= input.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
    hash >>>= 0;
  }
  return hash >>> 0;
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

function formatRateLimitLine(
  labelFallback: string,
  w: RateLimitWindow,
): string {
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

function rateLimitShortLabelFromMinutes(mins: number): string {
  if (mins === 300) return "5h";
  if (mins === 10080) return "wk";
  if (mins === 1440) return "day";
  if (mins % 60 === 0) return `${mins / 60}h`;
  return `${mins}m`;
}

function formatPercent2(n: number): string {
  return String(Math.round(n * 100) / 100);
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

function formatDurationJa(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const totalMinutes = Math.floor(totalSeconds / 60);
  const totalHours = Math.floor(totalMinutes / 60);
  const days = Math.floor(totalHours / 24);
  const hours = totalHours % 24;
  const minutes = totalMinutes % 60;
  const parts: string[] = [];
  if (days > 0) parts.push(`${days}日`);
  if (hours > 0 || days > 0) parts.push(`${hours}時間`);
  parts.push(`${minutes}分`);
  return parts.join("");
}

function formatResetsAtTooltip(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000);
  const abs = d.toLocaleString(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
  const deltaMs = d.getTime() - Date.now();
  if (!Number.isFinite(deltaMs)) return abs;
  if (deltaMs >= 0) return `${abs}（あと${formatDurationJa(deltaMs)}）`;
  return `${abs}（${formatDurationJa(-deltaMs)}前）`;
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

function cliVariantForBackendKey(
  backendKey: string,
): "unknown" | "codex" | "codex-mine" {
  const detected = cliVariantByBackendKey.get(backendKey) ?? "unknown";
  if (detected !== "unknown") return detected;

  // No detected runtime variant yet (e.g. backend not started). Use config as a hint.
  const folderUri = vscode.Uri.parse(backendKey);
  const cfg = vscode.workspace.getConfiguration("codexMine", folderUri);
  const raw = cfg.get<string>("cli.variant") ?? "auto";
  const normalized =
    raw === "mine" ? "codex-mine" : raw === "upstream" ? "codex" : raw;
  if (normalized === "codex-mine") return "codex-mine";
  if (normalized === "codex") return "codex";
  return "unknown";
}

function selectedCliVariantForBackendKey(
  backendKey: string,
): "auto" | "codex" | "codex-mine" {
  try {
    const folderUri = vscode.Uri.parse(backendKey);
    const cfg = vscode.workspace.getConfiguration("codexMine", folderUri);
    return normalizeCliVariant(cfg.get<string>("cli.variant") ?? "auto");
  } catch {
    return "auto";
  }
}

function isMineSelectedForBackendKey(backendKey: string): boolean {
  return selectedCliVariantForBackendKey(backendKey) === "codex-mine";
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

function normalizeCliVariant(
  raw: string | null,
): "auto" | "codex" | "codex-mine" {
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
  reason: "newSession" | "agents" | "mineFeature",
  notifyUser = true,
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
  if (notifyUser) {
    void vscode.window.showInformationMessage(
      `Backend restarted to use ${desired.variant}.`,
    );
  }
}

async function probeCliVersion(
  command: string,
): Promise<{ ok: true; version: string } | { ok: false; error: string }> {
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
      if ((first === '"' && last === '"') || (first === "'" && last === "'")) {
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
    uiHydrationBlockedText: null,
    tokenUsage: null,
    sending: false,
    reloading: false,
    compactInFlight: false,
    pendingCompactBlockId: null,
    pendingAssistantDeltas: new Map(),
    pendingAssistantDeltaFlushTimer: null,
    streamingAssistantItemIds: new Set(),
    activeTurnId: null,
    pendingInterrupt: false,
    lastTurnStartedAtMs: null,
    lastTurnCompletedAtMs: null,
    v2NotificationsSeen: false,
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
  const capsForBackendKey = (backendKey: string | null) => {
    if (!backendKey)
      return {
        agents: false,
        selectedCliVariant: "auto" as const,
        detectedCliVariant: "unknown" as const,
      };
    const detectedCliVariant = cliVariantForBackendKey(backendKey);
    return {
      agents: isMineSelectedForBackendKey(backendKey),
      selectedCliVariant: selectedCliVariantForBackendKey(backendKey),
      detectedCliVariant,
    };
  };
  if (!sessions)
    return {
      globalBlocks: globalRuntime.blocks,
      capabilities: capsForBackendKey(null),
      workspaceColorOverrides,
      sessions: [],
      activeSession: null,
      unreadSessionIds: [],
      runningSessionIds: [],
      blocks: [],
      latestDiff: null,
      sending: false,
      reloading: false,
      statusText: [globalStatusText, globalRateLimitStatusText]
        .filter(Boolean)
        .join(" • "),
      statusTooltip: globalRateLimitStatusTooltip,
      modelState: getSessionModelState(),
      models: null,
      approvals: [],
      customPrompts: promptSummaries,
    };

  const tabSessionsRaw = sessions
    .listAll()
    .filter((s) => !hiddenTabSessionIds.has(s.id));
  const runningSessionIds = tabSessionsRaw
    .map((s) => (ensureRuntime(s.id).sending ? s.id : null))
    .filter((v): v is string => typeof v === "string");
  const activeRaw = activeSessionId ? sessions.getById(activeSessionId) : null;
  if (!activeRaw)
    return {
      globalBlocks: globalRuntime.blocks,
      capabilities: capsForBackendKey(null),
      workspaceColorOverrides,
      sessions: tabSessionsRaw,
      activeSession: null,
      unreadSessionIds: [...unreadSessionIds],
      runningSessionIds,
      blocks: [],
      latestDiff: null,
      sending: false,
      reloading: false,
      statusText: [globalStatusText, globalRateLimitStatusText]
        .filter(Boolean)
        .join(" • "),
      statusTooltip: globalRateLimitStatusTooltip,
      modelState: getSessionModelState(),
      approvals: [],
      customPrompts: promptSummaries,
    };

  const rt = ensureRuntime(activeRaw.id);
  const baseStatusText = rt.statusText ?? null;
  const core: string[] = [];
  const hydrationBlockedText = rt.uiHydrationBlockedText ?? null;
  if (hydrationBlockedText) core.push("history not loaded");
  if (baseStatusText) core.push(baseStatusText);
  if (globalRateLimitStatusText) core.push(globalRateLimitStatusText);
  const suffix: string[] = [];
  if (rt.sending) suffix.push("sending…");
  if (rt.reloading) suffix.push("reloading…");
  const worked = computeWorkedSeconds(rt);
  if (worked !== null) suffix.push(`worked=${worked}s`);
  if (rt.pendingApprovals.size > 0)
    suffix.push(`approvals=${rt.pendingApprovals.size}`);
  const coreText = core.length > 0 ? core.join(" • ") : null;
  const statusText =
    coreText && suffix.length > 0
      ? `${coreText} • ${suffix.join(" • ")}`
      : coreText || (suffix.length > 0 ? suffix.join(" • ") : null);
  const statusTooltipParts = [
    hydrationBlockedText,
    globalRateLimitStatusTooltip,
  ]
    .filter(Boolean)
    .join("\n\n");
  return {
    globalBlocks: globalRuntime.blocks,
    capabilities: capsForBackendKey(activeRaw.backendKey),
    workspaceColorOverrides,
    sessions: tabSessionsRaw,
    activeSession: activeRaw,
    unreadSessionIds: [...unreadSessionIds],
    runningSessionIds,
    blocks: rt.blocks,
    latestDiff: rt.latestDiff,
    sending: rt.sending,
    reloading: rt.reloading,
    statusText:
      statusText ??
      [globalStatusText, globalRateLimitStatusText].filter(Boolean).join(" • "),
    statusTooltip: statusTooltipParts || null,
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
  backendKey: string,
  sessionId: string,
  n: AnyServerNotification,
): void {
  const rt = ensureRuntime(sessionId);
  if (!n.method.startsWith("codex/event/")) {
    if (!rt.v2NotificationsSeen) purgeLegacyToolBlocks(rt);
    rt.v2NotificationsSeen = true;
  }
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
      const blockId = rt.pendingCompactBlockId
        ? rt.pendingCompactBlockId
        : `compacted:${turnId || Date.now()}`;
      upsertBlock(sessionId, {
        id: blockId,
        type: "divider",
        status: "completed",
        text: `${line}\n• Context compacted`,
      });
      // Auto-compaction can happen mid-turn (the backend continues working).
      // In that case, do not unlock the input.
      if (rt.activeTurnId === null) rt.sending = false;
      rt.compactInFlight = false;
      rt.pendingCompactBlockId = null;
      chatView?.refresh();
      return;
    }
    case "turn/started":
      rt.sending = true;
      rt.lastTurnStartedAtMs = Date.now();
      rt.lastTurnCompletedAtMs = null;
      rt.activeTurnId = String((n as any).params?.turn?.id ?? "") || null;
      if (
        rt.pendingInterrupt &&
        rt.activeTurnId &&
        backendManager &&
        sessions
      ) {
        rt.pendingInterrupt = false;
        const session = sessions.getById(sessionId);
        if (session) {
          const turnId = rt.activeTurnId;
          outputChannel?.appendLine(
            `[turn] Sending queued interrupt: turnId=${turnId}`,
          );
          void backendManager.interruptTurn(session, turnId).catch((err) => {
            outputChannel?.appendLine(
              `[turn] Failed to interrupt (queued): ${String(err)}`,
            );
            upsertBlock(sessionId, {
              id: newLocalId("error"),
              type: "error",
              title: "Interrupt failed",
              text: String(err),
            });
            chatView?.refresh();
          });
        } else {
          outputChannel?.appendLine(
            `[turn] Queued interrupt dropped: session not found (sessionId=${sessionId})`,
          );
        }
      }
      chatView?.refresh();
      return;
    case "turn/completed":
      rt.sending = false;
      rt.lastTurnCompletedAtMs = Date.now();
      rt.activeTurnId = null;
      rt.pendingInterrupt = false;
      flushPendingAssistantDeltas(sessionId, rt);
      for (const id of rt.streamingAssistantItemIds) {
        const idx = rt.blockIndexById.get(id);
        if (idx === undefined) continue;
        const b = rt.blocks[idx];
        if (b && b.type === "assistant") {
          (b as any).streaming = false;
          chatView?.postBlockUpsert(sessionId, b);
        }
      }
      rt.streamingAssistantItemIds.clear();
      markUnreadSession(sessionId);
      sessionPanels?.markTurnCompleted(sessionId);
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
        streaming: true,
      }));
      const delta = (n as any).params.delta as string;
      if (block.type === "assistant") {
        (block as any).streaming = true;
      }
      rt.streamingAssistantItemIds.add(id);
      markUnreadSession(sessionId);
      const prev = rt.pendingAssistantDeltas.get(id);
      rt.pendingAssistantDeltas.set(id, prev ? prev + delta : delta);
      scheduleAssistantDeltaFlush(sessionId, rt);
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
      chatView?.postBlockUpsert(sessionId, block);
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
      chatView?.postBlockUpsert(sessionId, block);
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
      chatView?.postBlockUpsert(sessionId, block);
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
        hideCommandText: false,
        cwd: null,
        exitCode: null,
        durationMs: null,
        terminalStdin: [],
        output: "",
      }));
      const delta = (n as any).params.delta as string;
      if (block.type === "command") block.output += delta;
      chatView?.postBlockAppend(sessionId, id, "commandOutput", delta);
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
        hideCommandText: false,
        cwd: null,
        exitCode: null,
        durationMs: null,
        terminalStdin: [],
        output: "",
      }));
      if (block.type === "command")
        block.terminalStdin.push((n as any).params.stdin as string);
      chatView?.postBlockUpsert(sessionId, block);
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
      const delta = (n as any).params.delta as string;
      if (block.type === "fileChange") block.detail += delta;
      if (block.type === "fileChange")
        block.diffs = diffsForFiles(block.files, rt.latestDiff);
      chatView?.postBlockAppend(sessionId, id, "fileChangeDetail", delta);
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
      chatView?.postBlockUpsert(sessionId, block);
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
          chatView?.postBlockUpsert(sessionId, b);
        }
      }
      sessionPanels?.setLatestDiff(sessionId, rt.latestDiff);
      chatView?.refresh();
      return;
    }
    case "error": {
      const p = (n as any).params as {
        error?: {
          message?: unknown;
          codexErrorInfo?: unknown;
          additionalDetails?: unknown;
        };
        willRetry?: unknown;
      };
      const err = p?.error ?? {};
      const rawMessage =
        typeof err?.message === "string" ? err.message : String(err?.message ?? "");
      const message = rawMessage.trim();

      const additionalDetails =
        typeof err?.additionalDetails === "string"
          ? err.additionalDetails.trim()
          : "";

      const rawInfo = err?.codexErrorInfo ?? null;
      const infoKey =
        typeof rawInfo === "string"
          ? rawInfo
          : rawInfo && typeof rawInfo === "object"
            ? Object.keys(rawInfo as Record<string, unknown>)[0] ?? null
            : null;
      const infoValue =
        infoKey && rawInfo && typeof rawInfo === "object"
          ? (rawInfo as Record<string, unknown>)[infoKey]
          : null;
      const httpStatusCode =
        infoValue && typeof infoValue === "object"
          ? (infoValue as any).httpStatusCode ?? (infoValue as any).http_status_code
          : null;

      const willRetry = !!p?.willRetry;

      let title = "Error";
      if (infoKey === "rateLimited" || infoKey === "rate_limited") {
        title =
          typeof httpStatusCode === "number" ? `Rate limited (HTTP ${httpStatusCode})` : "Rate limited";
      } else if (infoKey === "usageLimitExceeded" || infoKey === "usage_limit_exceeded") {
        title = "Usage limit exceeded";
      } else if (infoKey === "contextWindowExceeded" || infoKey === "context_window_exceeded") {
        title = "Context window exceeded";
      }

      const lines: string[] = [];
      if (message) lines.push(message);
      if (additionalDetails) {
        if (lines.length > 0) lines.push("");
        lines.push(additionalDetails);
      }
      if (willRetry) {
        if (lines.length > 0) lines.push("");
        lines.push("Will retry automatically.");
      }
      upsertBlock(sessionId, {
        id: newLocalId("error"),
        type: "error",
        title,
        text: lines.join("\n").trim(),
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
        applyCodexEvent(rt, sessionId, backendKey, n.method, (n as any).params);
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
      chatView?.postBlockUpsert(sessionId, block);
      break;
    }
    case "commandExecution": {
      const block = getOrCreateBlock(rt, item.id, () => ({
        id: item.id,
        type: "command",
        title: "Command",
        status: item.status,
        command: item.command,
        hideCommandText: shouldHideCommandText(
          item.command,
          item.commandActions,
        ),
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
        block.hideCommandText = shouldHideCommandText(
          item.command,
          item.commandActions,
        );
        block.actionsText = formatCommandActions(item.commandActions);
        block.cwd = item.cwd ?? null;
        block.exitCode = item.exitCode;
        block.durationMs = item.durationMs;
        if (completed && item.aggregatedOutput)
          block.output = item.aggregatedOutput;
      }
      chatView?.postBlockUpsert(sessionId, block);
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
      chatView?.postBlockUpsert(sessionId, block);
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
      chatView?.postBlockUpsert(sessionId, block);
      if (completed && item.result?.content) {
        void appendMcpImageBlocks(
          rt,
          sessionId,
          item.id,
          item.server,
          item.tool,
          item.result.content,
        );
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

      upsertBlock(sessionId, {
        id: item.id,
        type: "webSearch",
        query: item.query,
        status: completed ? "completed" : "inProgress",
      });
      break;
    }
    case "imageView": {
      void upsertImageViewBlock(rt, sessionId, item.id, item.path, statusText);
      break;
    }
    case "enteredReviewMode": {
      upsertBlock(sessionId, {
        id: item.id,
        type: "system",
        title: `Entered review mode (${statusText})`,
        text: item.review,
      });
      break;
    }
    case "exitedReviewMode": {
      upsertBlock(sessionId, {
        id: item.id,
        type: "system",
        title: `Exited review mode (${statusText})`,
        text: item.review,
      });
      break;
    }
    case "agentMessage": {
      const id = item.id;
      const block = getOrCreateBlock(rt, id, () => ({
        id,
        type: "assistant",
        text: "",
        streaming: !completed,
      }));
      if (block.type === "assistant") {
        if (completed && typeof (item as any).text === "string") {
          block.text = String((item as any).text);
        }
        (block as any).streaming = !completed;
      }
      if (completed) rt.streamingAssistantItemIds.delete(id);
      else rt.streamingAssistantItemIds.add(id);
      chatView?.postBlockUpsert(sessionId, block);
      break;
    }
    default:
      // Hide userMessage/agentMessage lifecycle; handled elsewhere.
      break;
  }
}

function scheduleAssistantDeltaFlush(
  sessionId: string,
  rt: SessionRuntime,
): void {
  if (rt.pendingAssistantDeltaFlushTimer) return;
  rt.pendingAssistantDeltaFlushTimer = setTimeout(() => {
    rt.pendingAssistantDeltaFlushTimer = null;
    flushPendingAssistantDeltas(sessionId, rt);
  }, 16);
}

function flushPendingAssistantDeltas(
  sessionId: string,
  rt: SessionRuntime,
): void {
  if (rt.pendingAssistantDeltaFlushTimer) {
    clearTimeout(rt.pendingAssistantDeltaFlushTimer);
    rt.pendingAssistantDeltaFlushTimer = null;
  }
  if (rt.pendingAssistantDeltas.size === 0) return;
  const pending = [...rt.pendingAssistantDeltas.entries()];
  rt.pendingAssistantDeltas.clear();

  for (const [id, delta] of pending) {
    const idx = rt.blockIndexById.get(id);
    if (idx === undefined) {
      outputChannel?.appendLine(
        `[delta] Dropped pending assistant delta (missing block): sessionId=${sessionId} itemId=${id} bytes=${delta.length}`,
      );
      continue;
    }
    const b = rt.blocks[idx];
    if (!b || b.type !== "assistant") continue;
    b.text += delta;
    // IMPORTANT:
    // Do not force `streaming=true` here. This function can run after a turn is
    // completed (timer flush), and re-enabling streaming would keep the webview
    // in the <pre> fast-path and skip Markdown rendering for the final message.
    const isStreaming =
      rt.streamingAssistantItemIds.has(id) || rt.activeTurnId !== null;
    (b as any).streaming = isStreaming;
    sessionPanels?.appendAssistantDelta(sessionId, delta);
    chatView?.postBlockAppend(sessionId, id, "assistantText", delta, {
      streaming: isStreaming,
    });
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
    if (typeof sessionIdOrRt === "string") {
      chatView?.postBlockUpsert(sessionIdOrRt, block);
    }
    return;
  }
  rt.blocks[idx] = block;
  if (typeof sessionIdOrRt === "string") {
    chatView?.postBlockUpsert(sessionIdOrRt, block);
  }
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

function rebuildBlockIndex(rt: SessionRuntime): void {
  rt.blockIndexById.clear();
  for (let i = 0; i < rt.blocks.length; i++) {
    const b = rt.blocks[i];
    if (!b) continue;
    rt.blockIndexById.set(b.id, i);
  }
}

function purgeLegacyToolBlocks(rt: SessionRuntime): void {
  const before = rt.blocks.length;
  rt.blocks = rt.blocks.filter((b) => {
    const id = String(b?.id ?? "");
    if (!id) return true;
    if (id.startsWith("legacyCmd:")) return false;
    if (id.startsWith("legacyPatch:")) return false;
    if (id.startsWith("legacyWebSearch:")) return false;
    return true;
  });
  if (rt.blocks.length === before) return;
  rebuildBlockIndex(rt);
  rt.legacyPatchTargetByCallId.clear();
  rt.legacyWebSearchTargetByCallId.clear();
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
    // Mirror the TUI logic: compute remaining percentage from the last usage snapshot,
    // which reflects the latest context size, rather than the cumulative thread total.
    const BASELINE_TOKENS = 12000;
    const usedInContext = tokenUsage.last.totalTokens;
    const remainingTokens = Math.max(0, modelContextWindow - usedInContext);

    const remainingPct = (() => {
      if (modelContextWindow <= BASELINE_TOKENS) return 0;
      const effectiveWindow = modelContextWindow - BASELINE_TOKENS;
      const used = Math.max(0, usedInContext - BASELINE_TOKENS);
      const remaining = Math.max(0, effectiveWindow - used);
      return Math.max(
        0,
        Math.min(100, Math.round((remaining / effectiveWindow) * 100)),
      );
    })();

    return `ctx remaining=${remainingPct}% (${formatK(remainingTokens)}/${formatK(modelContextWindow)})`;
  }
  return `tokens used=${formatK(total.totalTokens)}`;
}

function isImageContent(block: ContentBlock): block is ImageContent {
  return (
    typeof (block as ImageContent).data === "string" &&
    typeof (block as ImageContent).mimeType === "string"
  );
}

function imageMimeFromPath(filePath: string): string | null {
  const ext = path.extname(filePath).toLowerCase();
  switch (ext) {
    case ".png":
      return "image/png";
    case ".jpg":
    case ".jpeg":
      return "image/jpeg";
    case ".gif":
      return "image/gif";
    case ".webp":
      return "image/webp";
    case ".bmp":
      return "image/bmp";
    case ".svg":
      return "image/svg+xml";
    case ".tif":
    case ".tiff":
      return "image/tiff";
    default:
      return null;
  }
}

async function loadLocalImageDataUrl(
  filePath: string,
): Promise<{ url: string; mimeType: string } | { error: string }> {
  const mimeType = imageMimeFromPath(filePath);
  if (!mimeType) {
    return { error: `Unsupported image extension: ${filePath}` };
  }
  try {
    const data = await fs.readFile(filePath);
    const base64 = data.toString("base64");
    return { url: `data:${mimeType};base64,${base64}`, mimeType };
  } catch (err) {
    return { error: `Failed to read image ${filePath}: ${String(err)}` };
  }
}

function enforceSessionImageAutoloadLimit(rt: SessionRuntime): void {
  const keep = SESSION_IMAGE_AUTOLOAD_RECENT;
  if (keep <= 0) return;
  let kept = 0;
  for (let i = rt.blocks.length - 1; i >= 0; i--) {
    const b = rt.blocks[i];
    if (!b) continue;

    const refs: any[] =
      b.type === "image"
        ? [b as any]
        : b.type === "imageGallery"
          ? Array.isArray((b as any).images)
            ? ((b as any).images as any[])
            : []
          : [];

    for (let j = refs.length - 1; j >= 0; j--) {
      const ref = refs[j];
      const hasKey = typeof ref?.imageKey === "string" && ref.imageKey;
      if (!hasKey) continue;
      if (kept < keep) {
        ref.autoLoad = true;
        kept += 1;
      } else {
        ref.autoLoad = false;
        // Ensure we don't keep a large inline src around for offloaded images.
        if (typeof ref.src === "string") ref.src = "";
      }
    }
  }
}

async function appendMcpImageBlocks(
  rt: SessionRuntime,
  sessionId: string,
  itemId: string,
  server: string,
  tool: string,
  content: ContentBlock[],
): Promise<void> {
  const images = content.filter(isImageContent);
  if (images.length === 0) return;
  const cached: Array<{
    imageKey: string;
    mimeType: string;
    byteLength: number;
  }> = [];
  for (let index = 0; index < images.length; index++) {
    const img = images[index]!;
    const bytes = Buffer.from(img.data, "base64");
    const saved = await cacheImageBytes({
      imageKey: `mcp-${sessionId}-${itemId}-${index}`,
      prefix: `mcp-${server}-${tool}`,
      mimeType: img.mimeType,
      bytes,
    });
    cached.push(saved);
    upsertBlock(sessionId, {
      id: `mcp-image:${itemId}:${index}`,
      type: "image",
      title: `MCP image (${server}.${tool})`,
      src: "",
      imageKey: saved.imageKey,
      mimeType: saved.mimeType,
      byteLength: saved.byteLength,
      autoLoad: true,
      alt: `mcp-image-${index + 1}`,
      caption: img.mimeType || null,
      role: "tool",
    } as any);
  }
  void cached;
  enforceSessionImageAutoloadLimit(rt);
  schedulePersistRuntime(sessionId);
}

async function upsertImageViewBlock(
  rt: SessionRuntime,
  sessionId: string,
  itemId: string,
  imagePath: string,
  statusText: string,
): Promise<void> {
  const mimeType = imageMimeFromPath(imagePath);
  if (!mimeType) {
    upsertBlock(sessionId, {
      id: `imageView:${itemId}`,
      type: "error",
      title: `Image view (${statusText})`,
      text: `Unsupported image extension: ${imagePath}`,
    });
    schedulePersistRuntime(sessionId);
    return;
  }

  try {
    const data = await fs.readFile(imagePath);
    const saved = await cacheImageBytes({
      imageKey: `imageView-${sessionId}-${itemId}`,
      prefix: `imageView-${itemId}`,
      mimeType,
      bytes: Buffer.from(data),
    });
    upsertBlock(sessionId, {
      id: `imageView:${itemId}`,
      type: "image",
      title: `Image view (${statusText})`,
      src: "",
      imageKey: saved.imageKey,
      mimeType: saved.mimeType,
      byteLength: saved.byteLength,
      autoLoad: true,
      alt: path.basename(imagePath) || "image",
      caption: imagePath,
      role: "system",
    } as any);
    enforceSessionImageAutoloadLimit(rt);
  } catch (err) {
    upsertBlock(sessionId, {
      id: `imageView:${itemId}`,
      type: "error",
      title: `Image view (${statusText})`,
      text: `Failed to read image ${imagePath}: ${String(err)}`,
    });
  }
  schedulePersistRuntime(sessionId);
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

function formatAskUserQuestionSummary(
  request: AskUserQuestionRequest,
  response: AskUserQuestionResponse,
): string {
  const title =
    typeof (request as any)?.title === "string" && (request as any).title.trim()
      ? String((request as any).title).trim()
      : "Codex question";

  const answers =
    typeof (response as any)?.answers === "object" &&
    (response as any).answers !== null
      ? ((response as any).answers as Record<string, unknown>)
      : {};

  const questions = Array.isArray((request as any)?.questions)
    ? ((request as any).questions as Array<any>)
    : [];

  const lines: string[] = [];
  lines.push(`**${title}**`);
  if ((response as any)?.cancelled) lines.push("_Cancelled_");
  lines.push("");

  for (const q of questions) {
    const id = typeof q?.id === "string" ? q.id : null;
    const prompt = typeof q?.prompt === "string" ? q.prompt : null;
    if (!id || !prompt) continue;

    const rawAnswer = answers[id];
    const optLabelByValue = new Map<string, string>();
    const rawOptions = Array.isArray(q?.options) ? (q.options as any[]) : [];
    for (const opt of rawOptions) {
      const value = typeof opt?.value === "string" ? opt.value : null;
      const label = typeof opt?.label === "string" ? opt.label : null;
      if (value && label) optLabelByValue.set(value, label);
    }

    const allowOther = Boolean(q?.allow_other);
    const questionType = typeof q?.type === "string" ? q.type : null;

    lines.push(`- **${prompt}**`);

    if (questionType === "single_select" || questionType === "multi_select") {
      const optionValues = new Set<string>(optLabelByValue.keys());

      const selectedValues: string[] = (() => {
        if (rawAnswer === null || rawAnswer === undefined) return [];
        if (Array.isArray(rawAnswer)) return rawAnswer.map((v) => String(v));
        return [String(rawAnswer)];
      })();

      const selectedOptionValues = new Set<string>(
        selectedValues.filter((v) => optionValues.has(v)),
      );
      const selectedOtherValues = selectedValues
        .filter((v) => !optionValues.has(v))
        .map((v) => v.trim())
        .filter(Boolean);

      for (const opt of rawOptions) {
        const value = typeof opt?.value === "string" ? opt.value : null;
        const label = typeof opt?.label === "string" ? opt.label : null;
        if (!value || !label) continue;
        const checked = selectedOptionValues.has(value) ? "x" : " ";
        lines.push(`  - [${checked}] ${label}`);
      }

      if (allowOther) {
        const checked = selectedOtherValues.length > 0 ? "x" : " ";
        const other =
          selectedOtherValues.length > 0
            ? `: ${selectedOtherValues.join(", ")}`
            : "";
        lines.push(`  - [${checked}] Other…${other}`);
      }

      if (
        rawOptions.length === 0 &&
        selectedValues.length > 0 &&
        selectedOtherValues.length > 0
      ) {
        lines.push(`  - Answer: ${selectedOtherValues.join(", ")}`);
      }

      if (selectedValues.length === 0) lines.push("  - _No selection_");
      continue;
    }

    const rendered = (() => {
      if (rawAnswer === null || rawAnswer === undefined) return "—";
      if (Array.isArray(rawAnswer)) {
        const parts = rawAnswer
          .map((v) => String(v))
          .map((s) => s.trim())
          .filter(Boolean);
        return parts.length > 0 ? parts.join(", ") : "—";
      }
      const s = String(rawAnswer).trim();
      return s ? s : "—";
    })();

    lines.push(`  - Answer: ${rendered}`);
  }

  return lines.join("\n").trim();
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

function applyGlobalNotification(
  backendKey: string,
  n: AnyServerNotification,
): void {
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

      const effectiveBackendKey = backendKeyForCwd(cwd) ?? backendKey;
      if (effectiveBackendKey) {
        if (isMineSelectedForBackendKey(effectiveBackendKey)) {
          const mcpLine = formatMcpStatusSummary(effectiveBackendKey);
          if (mcpLine) lines.push(mcpLine);
        }
        const next = inferCliVariantFromCliVersion(cliVersion);
        cliVariantByBackendKey.set(effectiveBackendKey, next);
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

      if (
        cwd &&
        effectiveBackendKey &&
        isMineSelectedForBackendKey(effectiveBackendKey)
      ) {
        void refreshMcpConfiguredServersForBackend(effectiveBackendKey, cwd);
      }

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
      const p = (n as any).params as { authMode?: unknown; activeAccount?: unknown };
      const authMode = String(p?.authMode ?? "null");
      const activeAccount =
        typeof p?.activeAccount === "string" ? p.activeAccount : null;
      globalStatusText = activeAccount
        ? `authMode=${authMode} active=${activeAccount}`
        : `authMode=${authMode}`;
      chatView?.refresh();
      return;
    }
    case "account/rateLimits/updated": {
      const rateLimits: RateLimitSnapshot = (n as any).params
        .rateLimits as RateLimitSnapshot;
      const p = rateLimits.primary;
      const s = rateLimits.secondary;
      const parts: string[] = [];
      const tooltipLines: string[] = [];
      if (p) {
        const mins = p.windowDurationMins ?? null;
        const label = mins ? rateLimitShortLabelFromMinutes(mins) : "primary";
        parts.push(`${label}:${formatPercent2(p.usedPercent)}%`);
        const reset = p.resetsAt ? formatResetsAtTooltip(p.resetsAt) : "不明";
        tooltipLines.push(`${label} リセット: ${reset}`);
      }
      if (s) {
        const mins = s.windowDurationMins ?? null;
        const label = mins ? rateLimitShortLabelFromMinutes(mins) : "secondary";
        parts.push(`${label}:${formatPercent2(s.usedPercent)}%`);
        const reset = s.resetsAt ? formatResetsAtTooltip(s.resetsAt) : "不明";
        tooltipLines.push(`${label} リセット: ${reset}`);
      }
      globalRateLimitStatusText = parts.length > 0 ? parts.join(" ") : null;
      globalRateLimitStatusTooltip =
        tooltipLines.length > 0 ? tooltipLines.join("\n") : null;
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
        applyGlobalCodexEvent(backendKey, n.method, (n as any).params);
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

async function refreshMcpConfiguredServersForBackend(
  backendKey: string,
  cwd: string,
): Promise<void> {
  if (!backendManager) return;
  if (!isMineSelectedForBackendKey(backendKey)) return;

  try {
    const response = await backendManager.listMcpServerStatus(backendKey, cwd);
    const nextNames = response.data.map((s) => s.name).filter(Boolean);

    const previous = mcpStatusByBackendKey.get(backendKey) ?? new Map();
    const next = new Map<string, string>();
    for (const name of nextNames) {
      next.set(name, previous.get(name) ?? "configured");
    }

    mcpStatusByBackendKey.set(backendKey, next);
    updateThreadStartedBlocks();
  } catch (e) {
    const msg =
      e instanceof Error ? e.stack || e.message : `Unknown error: ${String(e)}`;
    outputChannel?.appendLine(
      `[mcp] Failed to list configured MCP servers (backend=${backendKey}, cwd=${cwd}): ${msg}`,
    );
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

function getMcpStatusMap(backendKey: string): Map<string, string> {
  const existing = mcpStatusByBackendKey.get(backendKey);
  if (existing) return existing;
  const next = new Map<string, string>();
  mcpStatusByBackendKey.set(backendKey, next);
  return next;
}

function formatMcpStatusSummary(backendKey: string): string | null {
  const status = mcpStatusByBackendKey.get(backendKey);
  if (!status || status.size === 0) return null;
  const icon = (state: string): string =>
    state === "ready" ? "✓" : state === "starting" ? "…" : "•";
  const lines = [...status.entries()].map(
    ([server, state]) => `${icon(state)} ${server}`,
  );
  return ["MCP servers:", ...lines].join("\n");
}

function formatSessionConfigForDisplay(
  params: Record<string, unknown>,
): string {
  const model = typeof params.model === "string" ? params.model : "default";
  const provider =
    typeof params.modelProvider === "string" ? params.modelProvider : "default";
  const sandbox =
    typeof params.sandbox === "string" ? params.sandbox : "default";
  const plan =
    typeof params.planType === "string" ? params.planType : "default";
  return `model=${model}\nprovider=${provider}\nsandbox=${sandbox}\nplan=${plan}`;
}

function updateThreadStartedBlocks(): void {
  let changed = false;
  for (let i = 0; i < globalRuntime.blocks.length; i++) {
    const b = globalRuntime.blocks[i];
    if (!b) continue;
    if (b.type !== "info" || b.title !== "Thread started") continue;
    const cwdPrefix = "global:threadStarted:cwd:";
    const cwd = b.id.startsWith(cwdPrefix)
      ? b.id.slice(cwdPrefix.length)
      : null;
    const backendKey = cwd ? backendKeyForCwd(cwd) : null;
    const summary = backendKey ? formatMcpStatusSummary(backendKey) : null;
    const lines = b.text
      .split("\n")
      .filter(
        (l) => !l.startsWith("MCP servers:") && !/^\s*-?\s*[✓…•]/.test(l),
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

function applyGlobalCodexEvent(
  backendKey: string,
  method: string,
  params: unknown,
): void {
  const p = params as any;
  const msg = p?.msg as any;
  const type = typeof msg?.type === "string" ? msg.type : null;

  // A-policy: show only a minimal allowlist of legacy (codex/event/*) events.
  // Everything else is handled by v2 notifications and would otherwise duplicate UI.
  if (
    type !== "token_count" &&
    type !== "mcp_startup_complete" &&
    type !== "mcp_startup_update"
  ) {
    return;
  }

  if (type === "token_count") {
    const totalUsage = msg.info?.total_token_usage ?? null;
    const lastUsage = msg.info?.last_token_usage ?? null;
    const info = lastUsage ?? totalUsage ?? null;
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
        globalStatusText = `ctx remaining=${remainingPct}% (${remaining}/${ctx})`;
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
    const status =
      typeof msg.status === "object" && msg.status !== null ? msg.status : {};
    const state =
      typeof (status as any).state === "string"
        ? (status as any).state
        : "unknown";
    if (server !== "(unknown)") getMcpStatusMap(backendKey).set(server, state);
    updateThreadStartedBlocks();
    return;
  }
}

function applyCodexEvent(
  rt: SessionRuntime,
  sessionId: string,
  backendKey: string,
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

  // A-policy: show only a minimal allowlist of legacy (codex/event/*) events.
  // Everything else is handled by v2 notifications and would otherwise duplicate UI.
  if (
    type !== "token_count" &&
    type !== "turn_aborted" &&
    type !== "mcp_startup_complete" &&
    type !== "mcp_startup_update" &&
    type !== "list_custom_prompts_response"
  ) {
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
        argumentHint:
          typeof p?.argument_hint === "string" ? p.argument_hint : null,
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
    const state =
      typeof (status as any).state === "string"
        ? (status as any).state
        : "unknown";
    if (server !== "(unknown)") {
      getMcpStatusMap(backendKey).set(server, state);
      updateThreadStartedBlocks();
    }
    return;
  }

  if (type === "token_count") {
    const totalUsage = msg.info?.total_token_usage ?? null;
    const lastUsage = msg.info?.last_token_usage ?? null;
    const info = lastUsage ?? totalUsage ?? null;
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
        rt.statusText = `ctx remaining=${remainingPct}% (${remaining}/${ctx})`;
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
    upsertBlock(sessionId, {
      id: newLocalId("mcpStartup"),
      type: "system",
      title: "MCP startup issues",
      text: formatParamsForDisplay(msg),
    });
    return;
  }
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

function hydrateRuntimeFromThread(
  sessionId: string,
  thread: Thread,
  opts?: { force?: boolean },
): void {
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
  if (hasConversationBlocks) rt.uiHydrationBlockedText = null;
  if (!opts?.force && hasConversationBlocks) return;

  // Preserve non-conversation blocks that may have arrived before hydration (e.g. legacy warnings).
  const preserved = rt.blocks.filter(
    (b) =>
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
        if (text) upsertBlock(sessionId, { id: item.id, type: "user", text });
      }
      if (item.type === "agentMessage") {
        if (item.text)
          upsertBlock(sessionId, {
            id: item.id,
            type: "assistant",
            text: item.text,
            streaming: false,
          });
      }
    }
  }

  for (const b of preserved) upsertBlock(sessionId, b);

  if (activeSessionId === sessionId) {
    chatView?.syncBlocksForActiveSession();
  }
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
  | "id"
  | "backendKey"
  | "workspaceFolderUri"
  | "title"
  | "threadId"
  | "customTitle"
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

function schedulePersistRuntime(sessionId: string): void {
  // Intentionally no-op: only UI-specific state is persisted (sessions list, hidden tabs, etc).
  // Conversation history is re-hydrated from `thread/resume`, backed by ~/.codex/sessions.
  void sessionId;
}

async function cleanupLegacyRuntimeCache(
  context: vscode.ExtensionContext,
): Promise<void> {
  // Older versions cached full conversation blocks in workspaceState or storageUri, which
  // can make the Extension Host sluggish. We no longer use this cache.
  try {
    await context.workspaceState.update(LEGACY_RUNTIMES_KEY, undefined);
  } catch (err) {
    outputChannel?.appendLine(
      `[runtime] Failed to clear legacy workspaceState: ${String(err)}`,
    );
  }

  const base = context.storageUri?.fsPath ?? null;
  if (!base) return;
  const dir = path.join(base, "sessionRuntime.v1");
  await fs.rm(dir, { recursive: true, force: true }).catch(() => null);
}
