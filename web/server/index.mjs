import crypto from "node:crypto";
import fs from "node:fs/promises";
import fssync from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import http from "node:http";

import express from "express";
import mime from "mime";
import { Codex } from "@openai/codex-sdk";
import { WebSocketServer } from "ws";
import { AppServerProcess } from "./app_server.mjs";
import { renderVscodeChatHtml } from "./vscode_chat_html.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const webRoot = path.resolve(__dirname, "..");
const dataDir = path.join(webRoot, ".data");
const storePath = path.join(dataDir, "workspace.json");
const chatDir = path.join(dataDir, "chat");
const webviewStateDir = path.join(dataDir, "webview_state");
const distDir = path.join(webRoot, "dist");

const repoRoot = path.resolve(webRoot, "..");
const vscodeExtensionRoot = path.join(repoRoot, "vscode-extension");
const vscodeExtensionUiDir = path.join(vscodeExtensionRoot, "dist", "ui");
const vscodeExtensionVendorDir = path.join(vscodeExtensionRoot, "resources", "vendor");

const home = os.homedir();
const homeReal = await fs.realpath(home);

const MAX_TEXT_BYTES = 2 * 1024 * 1024;

function jsonError(res, status, message) {
  res.status(status).type("text/plain; charset=utf-8").send(message);
}

function isObject(value) {
  return typeof value === "object" && value !== null;
}

function isWithin(baseAbsReal, targetAbsReal) {
  const rel = path.relative(baseAbsReal, targetAbsReal);
  if (rel === "") return true;
  if (rel === "..") return false;
  if (rel.startsWith(`..${path.sep}`)) return false;
  return !path.isAbsolute(rel);
}

async function ensureDataDir() {
  await fs.mkdir(dataDir, { recursive: true });
}

async function readStore() {
  await ensureDataDir();
  try {
    const raw = await fs.readFile(storePath, "utf8");
    const parsed = JSON.parse(raw);
    if (parsed?.version !== 1 || !Array.isArray(parsed?.roots)) {
      throw new Error("workspace.json の形式が不正です");
    }
    const normDefault = normalizeCliCommand(defaultCliCommand) ?? "codez";
    // migrate settings.cliCommand (global)
    if (!parsed.settings || typeof parsed.settings !== "object") {
      parsed.settings = { cliCommand: normDefault };
      await writeStore(parsed);
      console.log(`[web] migrated workspace store: added settings.cliCommand=${normDefault}`);
    } else if (parsed.settings.cliCommand === undefined) {
      parsed.settings.cliCommand = normDefault;
      await writeStore(parsed);
      console.log(`[web] migrated workspace store: added settings.cliCommand=${normDefault}`);
    } else if (!normalizeCliCommand(parsed.settings.cliCommand)) {
      throw new Error("workspace.json の settings.cliCommand が不正です");
    }

    let migrated = false;
    for (const r of parsed.roots) {
      if (r && typeof r === "object" && r.cliCommand === undefined) {
        r.cliCommand = normDefault;
        migrated = true;
      }
    }
    if (migrated) {
      await writeStore(parsed);
      console.log(`[web] migrated workspace store: added cliCommand (default=${normDefault})`);
    }
    return parsed;
  } catch (e) {
    if (e && typeof e === "object" && "code" in e && e.code === "ENOENT") {
      const normDefault = normalizeCliCommand(defaultCliCommand) ?? "codez";
      const init = { version: 1, roots: [], settings: { cliCommand: normDefault } };
      await writeStore(init);
      console.log(`[web] init workspace store: ${storePath}`);
      return init;
    }
    throw e;
  }
}

async function writeStore(next) {
  await ensureDataDir();
  const tmp = path.join(dataDir, `workspace.json.tmp.${crypto.randomUUID()}`);
  await fs.writeFile(tmp, `${JSON.stringify(next, null, 2)}\n`, "utf8");
  await fs.rename(tmp, storePath);
}

function sortRoots(roots) {
  return [...roots].sort((a, b) => (a.order ?? 0) - (b.order ?? 0));
}

async function validateDirUnderHome(absPath) {
  if (typeof absPath !== "string" || absPath.length === 0) {
    throw new Error("absPath が不正です");
  }
  if (!path.isAbsolute(absPath)) {
    throw new Error("absPath は絶対パスである必要があります");
  }
  const st = await fs.stat(absPath);
  if (!st.isDirectory()) {
    throw new Error("ディレクトリではありません");
  }
  const real = await fs.realpath(absPath);
  if (!isWithin(homeReal, real)) {
    throw new Error("HOME 配下ではありません");
  }
  return real;
}

function parsePosixPath(p) {
  if (typeof p !== "string") throw new Error("path が不正です");
  if (!p.startsWith("/")) throw new Error("path は '/' で始まる必要があります");
  if (p.includes("\u0000")) throw new Error("path が不正です");
  const parts = p.split("/").filter(Boolean);
  for (const part of parts) {
    if (part === "." || part === "..") throw new Error("path が不正です");
  }
  return parts;
}

async function resolveInRoot(rootAbsReal, relPosix) {
  const parts = parsePosixPath(relPosix);
  const abs = path.join(rootAbsReal, ...parts);
  const st = await fs.lstat(abs);
  if (st.isSymbolicLink()) {
    const real = await fs.realpath(abs);
    if (!isWithin(rootAbsReal, real) || !isWithin(homeReal, real)) {
      throw new Error("symlink が root/HOME 外を指しています");
    }
    return { absPath: real, stat: await fs.stat(real), isSymlink: true };
  }
  const real = await fs.realpath(abs);
  if (!isWithin(rootAbsReal, real) || !isWithin(homeReal, real)) {
    throw new Error("root/HOME 外のパスです");
  }
  return { absPath: real, stat: await fs.stat(real), isSymlink: false };
}

function getMime(filePath) {
  return mime.getType(filePath) ?? "application/octet-stream";
}

async function listDir(absDir) {
  const entries = await fs.readdir(absDir, { withFileTypes: true });
  const out = [];
  for (const ent of entries) {
    const abs = path.join(absDir, ent.name);
    let kind = "other";
    if (ent.isDirectory()) kind = "dir";
    else if (ent.isFile()) kind = "file";
    else if (ent.isSymbolicLink()) kind = "symlink";
    const st = await fs.lstat(abs);
    const mtimeMs = st.mtimeMs;
    const size = kind === "file" ? st.size : undefined;
    out.push({ name: ent.name, kind, absPath: abs, size, mtimeMs });
  }
  out.sort((a, b) => {
    if (a.kind !== b.kind) {
      const rank = (k) => (k === "dir" ? 0 : k === "file" ? 1 : 2);
      return rank(a.kind) - rank(b.kind);
    }
    return a.name.localeCompare(b.name);
  });
  return out;
}

const app = express();
app.use(express.json({ limit: "5mb" }));

const defaultCliCommand = String(process.env.CODEX_WEB_DEFAULT_COMMAND ?? "codez");
function normalizeCliCommand(v) {
  if (v === "codex") return "codex";
  if (v === "codez") return "codez";
  return null;
}

// VSCode拡張の webview UI 資産（chat_view_client 等）を web から利用する。
// NOTE: これらはサーバー側で配信し、Vite dev では proxy でこのパスを通す。
if (fssync.existsSync(vscodeExtensionUiDir)) {
  app.use("/_ext/ui", express.static(vscodeExtensionUiDir));
}
if (fssync.existsSync(vscodeExtensionVendorDir)) {
  app.use("/_ext/vendor", express.static(vscodeExtensionVendorDir));
}

app.get("/api/browser/home", (req, res) => {
  res.json({ home: homeReal });
});

app.get("/api/browser/list", async (req, res) => {
  try {
    const p = req.query.path;
    if (typeof p !== "string") return jsonError(res, 400, "path が必要です");
    const abs = await validateDirUnderHome(p);
    const entries = await listDir(abs);
    const dirs = entries
      .filter((e) => e.kind === "dir")
      .map((e) => ({ name: e.name, absPath: e.absPath }));
    res.json({ path: abs, entries: dirs });
  } catch (e) {
    return jsonError(res, 400, e instanceof Error ? e.message : String(e));
  }
});

app.get("/api/workspace", async (req, res) => {
  try {
    const st = await readStore();
    res.json({ version: 1, roots: sortRoots(st.roots), settings: st.settings ?? null });
  } catch (e) {
    return jsonError(res, 500, e instanceof Error ? e.message : String(e));
  }
});

app.patch("/api/workspace/settings", async (req, res) => {
  try {
    const { cliCommand } = req.body ?? {};
    const norm = normalizeCliCommand(cliCommand);
    if (!norm) return jsonError(res, 400, "cliCommand が不正です");
    const st = await readStore();
    const next = { ...st, settings: { ...(st.settings ?? {}), cliCommand: norm } };
    await writeStore(next);

    // CLI切替は backend を作り直す必要があるので、起動中の app-server を落とす。
    for (const { proc } of appServers.values()) {
      try {
        proc.dispose();
      } catch {
        // ignore
      }
    }
    appServers.clear();

    res.json({ ok: true, settings: next.settings });
  } catch (e) {
    return jsonError(res, 500, e instanceof Error ? e.message : String(e));
  }
});

function sseWrite(res, payload) {
  res.write(`data: ${JSON.stringify(payload)}\n\n`);
}

function sseHeaders(res) {
  res.status(200);
  res.setHeader("Content-Type", "text/event-stream; charset=utf-8");
  res.setHeader("Cache-Control", "no-cache, no-transform");
  res.setHeader("Connection", "keep-alive");
  res.setHeader("X-Accel-Buffering", "no");
}

function isIdSafe(id) {
  return typeof id === "string" && /^[a-zA-Z0-9_-]{1,80}$/.test(id);
}

async function ensureChatDir() {
  await fs.mkdir(chatDir, { recursive: true });
}

async function ensureWebviewStateDir() {
  await fs.mkdir(webviewStateDir, { recursive: true });
}

function webviewStatePath(rootId) {
  if (!isIdSafe(rootId)) throw new Error("rootId が不正です");
  return path.join(webviewStateDir, `${rootId}.json`);
}

async function readWebviewState(rootId) {
  await ensureWebviewStateDir();
  const fp = webviewStatePath(rootId);
  try {
    const raw = await fs.readFile(fp, "utf8");
    const parsed = JSON.parse(raw);
    if (parsed?.version !== 1) throw new Error("webview state の形式が不正です");
    return { version: 1, state: parsed.state ?? null };
  } catch (e) {
    if (e && typeof e === "object" && "code" in e && e.code === "ENOENT") {
      const init = { version: 1, state: null };
      await writeWebviewState(rootId, init);
      return init;
    }
    throw e;
  }
}

async function writeWebviewState(rootId, next) {
  await ensureWebviewStateDir();
  const fp = webviewStatePath(rootId);
  const tmp = path.join(webviewStateDir, `webview_state.tmp.${crypto.randomUUID()}`);
  await fs.writeFile(tmp, `${JSON.stringify(next, null, 2)}\n`, "utf8");
  await fs.rename(tmp, fp);
}

function chatFilePath(rootId) {
  if (!isIdSafe(rootId)) throw new Error("rootId が不正です");
  return path.join(chatDir, `${rootId}.json`);
}

async function readChatStore(rootId) {
  await ensureChatDir();
  const fp = chatFilePath(rootId);
  try {
    const raw = await fs.readFile(fp, "utf8");
    const parsed = JSON.parse(raw);
    if (parsed?.version !== 1 || !Array.isArray(parsed?.sessions)) {
      throw new Error("chat store の形式が不正です");
    }
    const sessions = parsed.sessions
      .filter((s) => s && typeof s === "object" && typeof s.id === "string")
      .map((s) => {
        const o = s;
        const id = String(o.id);
        const title =
          typeof o.title === "string" && o.title.trim().length > 0 ? o.title.trim() : "chat";
        const threadId = o.threadId === null ? null : typeof o.threadId === "string" ? o.threadId : null;
        return { id, title, threadId };
      });
    const activeSessionId =
      typeof parsed.activeSessionId === "string" ? parsed.activeSessionId : (sessions[0]?.id ?? null);
    const messagesBySession =
      parsed.messagesBySession && typeof parsed.messagesBySession === "object"
        ? parsed.messagesBySession
        : {};
    return { version: 1, sessions, activeSessionId, messagesBySession };
  } catch (e) {
    if (e && typeof e === "object" && "code" in e && e.code === "ENOENT") {
      const init = {
        version: 1,
        sessions: [{ id: crypto.randomUUID(), title: "chat", threadId: null }],
        activeSessionId: null,
        messagesBySession: {},
      };
      init.activeSessionId = init.sessions[0].id;
      await writeChatStore(rootId, init);
      return init;
    }
    throw e;
  }
}

async function writeChatStore(rootId, next) {
  await ensureChatDir();
  const fp = chatFilePath(rootId);
  const tmp = path.join(chatDir, `chat.tmp.${crypto.randomUUID()}`);
  await fs.writeFile(tmp, `${JSON.stringify(next, null, 2)}\n`, "utf8");
  await fs.rename(tmp, fp);
}

async function requireWorkspaceRoot(rootId) {
  if (!isIdSafe(rootId)) throw new Error("rootId が不正です");
  const ws = await readStore();
  const roots = sortRoots(ws.roots);
  const root = roots.find((r) => r.id === rootId) ?? null;
  if (!root) throw new Error("root が見つかりません");
  return root;
}

app.get("/api/codex/status", async (req, res) => {
  // SDK は vendor バイナリを使うので "インストール済みか" は概ね問題にならない。
  // ただし、実行時に必要な認証/設定が足りない場合は run 時にエラーになる。
  res.json({
    available: true,
    notes:
      "Codex 実行はサーバー側プロセスで行います。API キー等の未設定は実行時にエラーとして露出します。",
  });
});

app.get("/api/chat/state", async (req, res) => {
  try {
    const rootId = req.query.root;
    if (typeof rootId !== "string") return jsonError(res, 400, "root が必要です");
    await requireWorkspaceRoot(rootId);
    const st = await readChatStore(rootId);
    res.json({ sessions: st.sessions, activeSessionId: st.activeSessionId });
  } catch (e) {
    return jsonError(res, 400, e instanceof Error ? e.message : String(e));
  }
});

app.get("/api/chat/messages", async (req, res) => {
  try {
    const rootId = req.query.root;
    const sessionId = req.query.session;
    if (typeof rootId !== "string") return jsonError(res, 400, "root が必要です");
    if (typeof sessionId !== "string") return jsonError(res, 400, "session が必要です");
    if (!isIdSafe(sessionId)) return jsonError(res, 400, "session が不正です");
    await requireWorkspaceRoot(rootId);
    const st = await readChatStore(rootId);
    const msgs = st.messagesBySession?.[sessionId];
    res.json({ messages: Array.isArray(msgs) ? msgs : [] });
  } catch (e) {
    return jsonError(res, 400, e instanceof Error ? e.message : String(e));
  }
});

app.post("/api/chat/session", async (req, res) => {
  try {
    const { rootId, title } = req.body ?? {};
    if (typeof rootId !== "string") return jsonError(res, 400, "rootId が必要です");
    const root = await requireWorkspaceRoot(rootId);
    const st = await readChatStore(rootId);
    const id = crypto.randomUUID();
    const t = typeof title === "string" && title.trim() ? title.trim() : `chat ${st.sessions.length + 1}`;

    // VSCode拡張と同様に、session 作成時点で backend thread を作る。
    const backend = await getOrStartAppServer(root);
    const started = await backend.proc.threadStart({
      model: null,
      modelProvider: null,
      cwd: root.absPath,
      approvalPolicy: "on-request",
      sandbox: "workspace-write",
      config: null,
      baseInstructions: null,
      developerInstructions: null,
      experimentalRawEvents: false,
    });
    const threadId = started?.thread?.id ?? null;
    if (!threadId) throw new Error("thread/start failed: missing threadId");

    const session = { id, title: t, threadId };
    const next = {
      ...st,
      sessions: [...st.sessions, session],
      activeSessionId: id,
    };
    await writeChatStore(rootId, next);
    res.status(201).json({ session, activeSessionId: id });
  } catch (e) {
    return jsonError(res, 400, e instanceof Error ? e.message : String(e));
  }
});

app.patch("/api/chat/state", async (req, res) => {
  try {
    const { rootId, activeSessionId } = req.body ?? {};
    if (typeof rootId !== "string") return jsonError(res, 400, "rootId が必要です");
    await requireWorkspaceRoot(rootId);
    const st = await readChatStore(rootId);
    if (activeSessionId !== null && activeSessionId !== undefined) {
      if (typeof activeSessionId !== "string" || !isIdSafe(activeSessionId)) {
        return jsonError(res, 400, "activeSessionId が不正です");
      }
      if (!st.sessions.some((s) => s.id === activeSessionId)) {
        return jsonError(res, 404, "session が見つかりません");
      }
    }
    const next = { ...st, activeSessionId: activeSessionId ?? null };
    await writeChatStore(rootId, next);
    res.json({ ok: true });
  } catch (e) {
    return jsonError(res, 400, e instanceof Error ? e.message : String(e));
  }
});

app.patch("/api/chat/session", async (req, res) => {
  try {
    const { rootId, sessionId, title, threadId } = req.body ?? {};
    if (typeof rootId !== "string") return jsonError(res, 400, "rootId が必要です");
    if (typeof sessionId !== "string" || !isIdSafe(sessionId)) {
      return jsonError(res, 400, "sessionId が不正です");
    }
    await requireWorkspaceRoot(rootId);
    const st = await readChatStore(rootId);
    const idx = st.sessions.findIndex((s) => s.id === sessionId);
    if (idx < 0) return jsonError(res, 404, "session が見つかりません");
    const cur = st.sessions[idx];
    const nextSession = { ...cur };
    if (title !== undefined) {
      if (typeof title !== "string" || !title.trim()) return jsonError(res, 400, "title が不正です");
      nextSession.title = title.trim();
    }
    if (threadId !== undefined) {
      if (threadId !== null && typeof threadId !== "string") {
        return jsonError(res, 400, "threadId が不正です");
      }
      nextSession.threadId = threadId === null ? null : threadId;
    }
    const sessions = [...st.sessions];
    sessions[idx] = nextSession;
    const next = { ...st, sessions };
    await writeChatStore(rootId, next);
    res.json({ ok: true, session: nextSession });
  } catch (e) {
    return jsonError(res, 400, e instanceof Error ? e.message : String(e));
  }
});

app.put("/api/chat/messages", async (req, res) => {
  try {
    const { rootId, sessionId, messages } = req.body ?? {};
    if (typeof rootId !== "string") return jsonError(res, 400, "rootId が必要です");
    if (typeof sessionId !== "string" || !isIdSafe(sessionId)) {
      return jsonError(res, 400, "sessionId が不正です");
    }
    if (!Array.isArray(messages)) return jsonError(res, 400, "messages が不正です");
    await requireWorkspaceRoot(rootId);
    const st = await readChatStore(rootId);
    if (!st.sessions.some((s) => s.id === sessionId)) {
      return jsonError(res, 404, "session が見つかりません");
    }
    const next = {
      ...st,
      messagesBySession: { ...st.messagesBySession, [sessionId]: messages },
    };
    await writeChatStore(rootId, next);
    res.json({ ok: true });
  } catch (e) {
    return jsonError(res, 400, e instanceof Error ? e.message : String(e));
  }
});

app.post("/api/codex/run", async (req, res) => {
  const { input, threadId, rootId } = req.body ?? {};
  if (typeof input !== "string" || input.trim().length === 0) {
    return jsonError(res, 400, "input が必要です");
  }
  if (threadId !== undefined && (typeof threadId !== "string" || threadId.length === 0)) {
    return jsonError(res, 400, "threadId が不正です");
  }
  if (rootId !== undefined && (typeof rootId !== "string" || rootId.length === 0)) {
    return jsonError(res, 400, "rootId が不正です");
  }

  sseHeaders(res);

  const heartbeat = setInterval(() => {
    res.write(": ping\n\n");
  }, 15000);

  const controller = new AbortController();
  req.on("close", () => controller.abort());

  try {
    const ws = await readStore();
    const roots = sortRoots(ws.roots);
    const activeRoot =
      typeof rootId === "string"
        ? roots.find((r) => r.id === rootId) ?? null
        : roots[0] ?? null;
    if (!activeRoot) {
      throw new Error("root がありません（先にワークスペースへフォルダを追加してください）");
    }
    const additionalDirectories = [activeRoot.absPath];
    const workingDirectory = activeRoot.absPath;

    const cliCommand = normalizeCliCommand(activeRoot.cliCommand) ?? "codez";
    const codex = new Codex({ codexPathOverride: cliCommand });

    const threadOptions = {
      sandboxMode: "read-only",
      approvalPolicy: "never",
      skipGitRepoCheck: true,
      workingDirectory,
      additionalDirectories,
      networkAccessEnabled: false,
      webSearchEnabled: false,
    };

    const thread = threadId
      ? codex.resumeThread(threadId, threadOptions)
      : codex.startThread(threadOptions);
    const { events } = await thread.runStreamed(input, { signal: controller.signal });
    for await (const ev of events) {
      // note: thread.started 等をそのままクライアントへ透過
      sseWrite(res, ev);
    }

    // 正常終了の印
    sseWrite(res, { type: "web.done" });
  } catch (e) {
    sseWrite(res, {
      type: "web.error",
      message: e instanceof Error ? e.message : String(e),
    });
  } finally {
    clearInterval(heartbeat);
    res.end();
  }
});

app.post("/api/workspace/roots", async (req, res) => {
  try {
    const { absPath, label } = req.body ?? {};
    const real = await validateDirUnderHome(absPath);
    const st = await readStore();
    if (st.roots.some((r) => r.absPath === real)) {
      return jsonError(res, 409, "すでに登録済みです");
    }
    const id = crypto.randomUUID();
    const nextOrder =
      st.roots.length === 0 ? 0 : Math.max(...st.roots.map((r) => r.order ?? 0)) + 1;
    const rootLabel =
      typeof label === "string" && label.trim().length > 0 ? label.trim() : path.basename(real);
    const cliCommand = normalizeCliCommand(defaultCliCommand) ?? "codez";
    const next = {
      ...st,
      roots: [
        ...st.roots,
        {
          id,
          label: rootLabel,
          absPath: real,
          cliCommand,
          createdAt: new Date().toISOString(),
          order: nextOrder,
        },
      ],
    };
    await writeStore(next);
    res.status(201).json({ ok: true, id });
  } catch (e) {
    return jsonError(res, 400, e instanceof Error ? e.message : String(e));
  }
});

app.patch("/api/workspace/roots/:id", async (req, res) => {
  try {
    const id = req.params.id;
    const { label, cliCommand } = req.body ?? {};
    const st = await readStore();
    const idx = st.roots.findIndex((r) => r.id === id);
    if (idx < 0) return jsonError(res, 404, "root が見つかりません");
    const roots = [...st.roots];
    const cur = roots[idx];
    const next = { ...cur };
    if (label !== undefined) {
      if (typeof label !== "string" || label.trim().length === 0) {
        return jsonError(res, 400, "label が不正です");
      }
      next.label = label.trim();
    }
    if (cliCommand !== undefined) {
      const norm = normalizeCliCommand(cliCommand);
      if (!norm) return jsonError(res, 400, "cliCommand が不正です");
      next.cliCommand = norm;
    }
    roots[idx] = next;
    await writeStore({ ...st, roots });
    res.json({ ok: true });
  } catch (e) {
    return jsonError(res, 500, e instanceof Error ? e.message : String(e));
  }
});

app.delete("/api/workspace/roots/:id", async (req, res) => {
  try {
    const id = req.params.id;
    const st = await readStore();
    const roots = st.roots.filter((r) => r.id !== id);
    if (roots.length === st.roots.length) return jsonError(res, 404, "root が見つかりません");
    await writeStore({ ...st, roots });
    res.status(204).end();
  } catch (e) {
    return jsonError(res, 500, e instanceof Error ? e.message : String(e));
  }
});

app.get("/api/tree", async (req, res) => {
  try {
    const rootId = req.query.root;
    const rel = req.query.path ?? "/";
    if (typeof rootId !== "string" || rootId.length === 0) {
      return jsonError(res, 400, "root が必要です");
    }
    if (typeof rel !== "string") return jsonError(res, 400, "path が不正です");
    const st = await readStore();
    const root = st.roots.find((r) => r.id === rootId);
    if (!root) return jsonError(res, 404, "root が見つかりません");
    const { absPath, stat } = await resolveInRoot(root.absPath, rel);
    if (!stat.isDirectory()) return jsonError(res, 400, "ディレクトリではありません");
    const entries = await listDir(absPath);
    res.json(
      entries.map((e) => ({
        name: e.name,
        kind: e.kind,
        path: joinPosix(rel, e.name),
        size: e.size,
        mtimeMs: e.mtimeMs,
      })),
    );
  } catch (e) {
    return jsonError(res, 400, e instanceof Error ? e.message : String(e));
  }
});

function joinPosix(dir, name) {
  const base = dir.endsWith("/") ? dir.slice(0, -1) : dir;
  return `${base}/${name}`;
}

app.get("/api/file", async (req, res) => {
  try {
    const rootId = req.query.root;
    const rel = req.query.path ?? "/";
    if (typeof rootId !== "string" || rootId.length === 0) {
      return jsonError(res, 400, "root が必要です");
    }
    if (typeof rel !== "string") return jsonError(res, 400, "path が不正です");
    const st = await readStore();
    const root = st.roots.find((r) => r.id === rootId);
    if (!root) return jsonError(res, 404, "root が見つかりません");
    const { absPath, stat } = await resolveInRoot(root.absPath, rel);
    if (!stat.isFile()) return jsonError(res, 400, "ファイルではありません");
    if (stat.size > MAX_TEXT_BYTES) {
      return jsonError(res, 413, `ファイルが大きすぎます (${stat.size} bytes)`);
    }
    const buf = await fs.readFile(absPath);
    const dec = new TextDecoder("utf-8", { fatal: true });
    let text;
    try {
      text = dec.decode(buf);
    } catch {
      return jsonError(res, 415, "UTF-8 として解釈できません（バイナリの可能性）");
    }
    res.json({ text });
  } catch (e) {
    return jsonError(res, 400, e instanceof Error ? e.message : String(e));
  }
});

app.get("/api/raw", async (req, res) => {
  try {
    const rootId = req.query.root;
    const rel = req.query.path ?? "/";
    if (typeof rootId !== "string" || rootId.length === 0) {
      return jsonError(res, 400, "root が必要です");
    }
    if (typeof rel !== "string") return jsonError(res, 400, "path が不正です");
    const st = await readStore();
    const root = st.roots.find((r) => r.id === rootId);
    if (!root) return jsonError(res, 404, "root が見つかりません");
    const { absPath, stat } = await resolveInRoot(root.absPath, rel);
    if (!stat.isFile()) return jsonError(res, 400, "ファイルではありません");
    res.setHeader("Content-Type", getMime(absPath));
    fssync.createReadStream(absPath).pipe(res);
  } catch (e) {
    return jsonError(res, 400, e instanceof Error ? e.message : String(e));
  }
});

app.get("/webview/chat", async (req, res) => {
  try {
    const rootId = req.query.rootId;
    if (typeof rootId !== "string" || rootId.length === 0) {
      return jsonError(res, 400, "rootId が必要です");
    }
    const root = await requireWorkspaceRoot(rootId);
    const uiState = await readWebviewState(rootId);

    const shimScript = `
(() => {
  const initial = ${JSON.stringify(uiState.state ?? null)};
  let state = initial;
  const rootId = ${JSON.stringify(rootId)};
  const wsUrl = (() => {
    const proto = location.protocol === "https:" ? "wss:" : "ws:";
    return proto + "//" + location.host + "/api/webview/ws?rootId=" + encodeURIComponent(rootId);
  })();

  const ws = new WebSocket(wsUrl);
  const queue = [];
  const send = (obj) => {
    const line = JSON.stringify(obj);
    if (ws.readyState === WebSocket.OPEN) ws.send(line);
    else queue.push(line);
  };
  ws.addEventListener("open", () => {
    while (queue.length) ws.send(queue.shift());
  });
  ws.addEventListener("message", (ev) => {
    try {
      const msg = JSON.parse(String(ev.data));
      window.dispatchEvent(new MessageEvent("message", { data: msg }));
    } catch (err) {
      window.dispatchEvent(new MessageEvent("message", { data: { type: "toast", kind: "error", message: "webview: invalid message from server: " + String(err), timeoutMs: 4000 } }));
    }
  });
  ws.addEventListener("close", () => {
    window.dispatchEvent(new MessageEvent("message", { data: { type: "toast", kind: "error", message: "webview: 接続が切れました（再読み込みしてください）", timeoutMs: 0 } }));
  });

  window.addEventListener("message", (ev) => {
    try {
      if (ev.origin !== location.origin) return;
      const d = ev.data;
      if (!d || typeof d !== "object") return;
      if (d.type === "codez.refreshState") {
        send({ type: "webview.refresh" });
      }
    } catch {
      // ignore
    }
  });

  globalThis.acquireVsCodeApi = () => ({
    postMessage: (msg) => {
      try {
        const t = msg && typeof msg === "object" ? msg.type : null;
        if (t === "newSessionPickFolder") {
          window.parent?.postMessage({ type: "codez.newSessionPickFolder" }, location.origin);
          return;
        }
        if (t === "selectCliVariant") {
          window.parent?.postMessage({ type: "codez.openSettings" }, location.origin);
          return;
        }
      } catch {
        // ignore
      }
      send({ type: "webview.postMessage", msg });
    },
    getState: () => state,
    setState: (next) => {
      state = next;
      send({ type: "webview.setState", state: next });
    },
  });
})();`.trim();

    const html = renderVscodeChatHtml({
      title: `Codex: ${root.label}`,
      shimScript,
      markdownItSrc: "/_ext/vendor/markdown-it.min.js",
      clientScriptSrc: "/_ext/ui/chat_view_client.js",
    });

    res.status(200).type("text/html; charset=utf-8").send(html);
  } catch (e) {
    return jsonError(res, 500, e instanceof Error ? e.message : String(e));
  }
});

if (fssync.existsSync(distDir)) {
  app.use(express.static(distDir));
  app.get("*", (req, res, next) => {
    if (req.path.startsWith("/api/")) return next();
    res.sendFile(path.join(distDir, "index.html"));
  });
}

const port = Number(process.env.PORT ?? 3000);
const host = String(process.env.HOST ?? "0.0.0.0");

const server = http.createServer(app);

// app-server per rootId (single-user前提)
const appServers = new Map(); // rootId -> { proc, subscribers:Set<WebSocket>, pending:Map<id, {type, resolveAtMs}> }

async function getOrStartAppServer(root) {
  const key = root.id;
  const existing = appServers.get(key);
  if (existing) return existing;

  const ws = await readStore();
  const globalCli = normalizeCliCommand(ws.settings?.cliCommand) ?? "codez";

  const proc = new AppServerProcess({
    command: globalCli,
    cwd: root.absPath,
    log: (line) => console.log(`[app-server ${root.label}] ${line}`),
    logRpcPayloads: false,
  });
  await proc.start();

  const state = { proc, subscribers: new Set(), pending: new Map() };
  proc.rpc.on("serverNotification", (n) => {
    const payload = JSON.stringify({ type: "backend.notification", rootId: key, notification: n });
    for (const ws of state.subscribers) {
      if (ws.readyState === ws.OPEN) ws.send(payload);
    }
  });

  proc.rpc.on("serverRequest", (req) => {
    const id = req.id;
    // approvals / requestUserInput は UI へ委譲
    state.pending.set(id, { req, createdAtMs: Date.now() });
    const payload = JSON.stringify({ type: "backend.request", rootId: key, request: req });
    for (const ws of state.subscribers) {
      if (ws.readyState === ws.OPEN) ws.send(payload);
    }
  });

  proc.rpc.on("exit", () => {
    appServers.delete(key);
    const payload = JSON.stringify({ type: "backend.exit", rootId: key });
    for (const ws of state.subscribers) {
      if (ws.readyState === ws.OPEN) ws.send(payload);
    }
  });

  appServers.set(key, state);
  return state;
}

function wsSend(ws, msg) {
  if (ws.readyState === ws.OPEN) ws.send(JSON.stringify(msg));
}

function normalizeApprovalDecision(v) {
  if (v === "accept") return "accept";
  if (v === "acceptForSession") return "acceptForSession";
  if (v === "decline") return "decline";
  if (v === "cancel") return "cancel";
  // acceptWithExecpolicyAmendment などは将来対応
  return null;
}

function toCliVariant(v) {
  const norm = normalizeCliCommand(v);
  if (norm === "codex") return "codex";
  if (norm === "codez") return "codez";
  return "unknown";
}

function rootUri(root) {
  return pathToFileURL(root.absPath).toString();
}

function chatMessagesToBlocks(messages) {
  const blocks = [];
  for (const m of Array.isArray(messages) ? messages : []) {
    if (!m || typeof m !== "object") continue;
    const id = typeof m.id === "string" ? m.id : null;
    const role = typeof m.role === "string" ? m.role : null;
    const text = typeof m.text === "string" ? m.text : "";
    if (!id || !role) continue;
    if (role === "user") blocks.push({ id, type: "user", text });
    else if (role === "assistant") blocks.push({ id, type: "assistant", text, streaming: false });
    else blocks.push({ id, type: "note", text });
  }
  return blocks;
}

function blockAssistantPlaceholder(id) {
  return { id, type: "assistant", text: "", streaming: true };
}

const wssWebview = new WebSocketServer({ server, path: "/api/webview/ws" });
wssWebview.on("connection", async (ws, req) => {
  const url = new URL(req.url ?? "/", "http://localhost");
  const rootId = url.searchParams.get("rootId");
  if (!rootId || !isIdSafe(rootId)) {
    wsSend(ws, { type: "toast", kind: "error", message: "webview: rootId が不正です", timeoutMs: 6000 });
    ws.close();
    return;
  }

  let root;
  let backend;
  try {
    root = await requireWorkspaceRoot(rootId);
    backend = await getOrStartAppServer(root);
  } catch (e) {
    wsSend(ws, { type: "toast", kind: "error", message: e instanceof Error ? e.message : String(e), timeoutMs: 8000 });
    ws.close();
    return;
  }

  let seq = 1;
  const approvalsByKey = new Map(); // requestKey -> { requestId:number, method:string, params:any, canAcceptForSession:boolean }
  const requestUserInputByKey = new Map(); // requestKey -> { requestId:number, params:any }
  const activeTurnByThreadId = new Map(); // threadId -> turnId
  const assistantByThreadId = new Map(); // threadId -> { sessionId, blockId, text }

  async function loadState() {
    const wsStore = await readStore();
    const cliVariant = toCliVariant(wsStore.settings?.cliCommand);

    const chat = await readChatStore(root.id);
    const sessions = chat.sessions.map((s) => ({
      id: s.id,
      title: s.title,
      workspaceFolderUri: rootUri(root),
    }));
    const activeSessionId = chat.activeSessionId;
    const activeSession = activeSessionId ? sessions.find((s) => s.id === activeSessionId) ?? null : null;

    const messages =
      activeSessionId && chat.messagesBySession && chat.messagesBySession[activeSessionId]
        ? chat.messagesBySession[activeSessionId]
        : [];
    const blocks = chatMessagesToBlocks(messages);

    const approvals = [...approvalsByKey.entries()].map(([requestKey, ap]) => ({
      requestKey,
      title: ap.method,
      detail: JSON.stringify(ap.params ?? null, null, 2),
      canAcceptForSession: Boolean(ap.canAcceptForSession),
    }));

    const runningSessionIds = [];
    for (const s of chat.sessions) {
      if (!s.threadId) continue;
      const activeTurnId = activeTurnByThreadId.get(s.threadId);
      if (activeTurnId) runningSessionIds.push(s.id);
    }

    return {
      full: {
        capabilities: { agents: false, cliVariant },
        sessions,
        activeSession,
        unreadSessionIds: [],
        runningSessionIds,
        blocks,
        latestDiff: null,
        sending: runningSessionIds.includes(activeSessionId ?? ""),
        reloading: false,
        approvals,
        customPrompts: [],
      },
      activeSessionId,
      blocks,
    };
  }

  async function sendFullState() {
    const st = await loadState();
    wsSend(ws, { type: "state", seq: seq++, state: st.full });
  }

  async function sendBlocksReset(sessionId) {
    const chat = await readChatStore(root.id);
    const messages = chat.messagesBySession?.[sessionId] ?? [];
    const blocks = chatMessagesToBlocks(messages);
    wsSend(ws, { type: "blocksReset", sessionId, blocks });
  }

  async function sendControlState() {
    const wsStore = await readStore();
    const cliVariant = toCliVariant(wsStore.settings?.cliCommand);
    const chat = await readChatStore(root.id);
    const sessions = chat.sessions.map((s) => ({
      id: s.id,
      title: s.title,
      workspaceFolderUri: rootUri(root),
    }));
    const activeSessionId = chat.activeSessionId;
    const activeSession = activeSessionId ? sessions.find((s) => s.id === activeSessionId) ?? null : null;

    const approvals = [...approvalsByKey.entries()].map(([requestKey, ap]) => ({
      requestKey,
      title: ap.method,
      detail: JSON.stringify(ap.params ?? null, null, 2),
      canAcceptForSession: Boolean(ap.canAcceptForSession),
    }));

    const runningSessionIds = [];
    for (const s of chat.sessions) {
      if (!s.threadId) continue;
      const activeTurnId = activeTurnByThreadId.get(s.threadId);
      if (activeTurnId) runningSessionIds.push(s.id);
    }

    wsSend(ws, {
      type: "controlState",
      seq: seq++,
      state: {
        capabilities: { agents: false, cliVariant },
        sessions,
        activeSession,
        unreadSessionIds: [],
        runningSessionIds,
        latestDiff: null,
        sending: runningSessionIds.includes(activeSessionId ?? ""),
        reloading: false,
        approvals,
        customPrompts: [],
      },
    });
  }

  function onServerNotification(n) {
    const method = String(n?.method ?? "");
    const p = n?.params ?? null;

    if (method === "turn/started") {
      const threadId = typeof p?.threadId === "string" ? p.threadId : null;
      const turnId = typeof p?.turn?.id === "string" ? p.turn.id : null;
      if (threadId && turnId) {
        activeTurnByThreadId.set(threadId, turnId);
        void sendControlState().catch(() => {});
      }
      return;
    }

    if (method === "turn/completed") {
      const threadId = typeof p?.threadId === "string" ? p.threadId : null;
      const turnId = typeof p?.turn?.id === "string" ? p.turn.id : null;
      if (threadId && turnId) {
        const active = activeTurnByThreadId.get(threadId);
        if (active === turnId) activeTurnByThreadId.set(threadId, null);
        const a = assistantByThreadId.get(threadId);
        if (a) {
          wsSend(ws, {
            type: "blockUpsert",
            sessionId: a.sessionId,
            block: { id: a.blockId, type: "assistant", text: a.text, streaming: false },
          });
          assistantByThreadId.delete(threadId);
          // persist assistant message
          void (async () => {
            const chat = await readChatStore(root.id);
            const msgs = chat.messagesBySession?.[a.sessionId] ?? [];
            const idx = msgs.findIndex((m) => m && m.id === a.blockId);
            if (idx >= 0) msgs[idx] = { ...msgs[idx], role: "assistant", text: a.text };
            else msgs.push({ id: a.blockId, role: "assistant", text: a.text });
            chat.messagesBySession = { ...(chat.messagesBySession ?? {}), [a.sessionId]: msgs };
            await writeChatStore(root.id, chat);
          })().catch(() => {});
        }
        void sendControlState().catch(() => {});
      }
      return;
    }

    if (method === "item/agentMessage/delta") {
      const threadId = typeof p?.threadId === "string" ? p.threadId : null;
      const turnId = typeof p?.turnId === "string" ? p.turnId : null;
      const delta = typeof p?.delta === "string" ? p.delta : null;
      if (!threadId || !turnId || !delta) return;
      const activeTurnId = activeTurnByThreadId.get(threadId);
      if (!activeTurnId || activeTurnId !== turnId) return;
      const a = assistantByThreadId.get(threadId);
      if (!a) return;
      a.text += delta;
      wsSend(ws, {
        type: "blockAppend",
        sessionId: a.sessionId,
        blockId: a.blockId,
        field: "assistantText",
        delta,
        streaming: true,
      });
      return;
    }
  }

  function onServerRequest(r) {
    const method = String(r?.method ?? "");
    const params = r?.params ?? null;
    const requestId = typeof r?.id === "number" ? r.id : null;
    if (requestId === null) return;

    if (method === "item/commandExecution/requestApproval" || method === "item/fileChange/requestApproval") {
      const requestKey = `${root.id}:${String(requestId)}`;
      approvalsByKey.set(requestKey, { requestId, method, params, canAcceptForSession: true });
      void sendControlState().catch(() => {});
      return;
    }

    if (method === "item/tool/requestUserInput") {
      const requestKey = `${root.id}:${String(requestId)}`;
      requestUserInputByKey.set(requestKey, { requestId, params });
      wsSend(ws, { type: "requestUserInputStart", requestKey, params });
      return;
    }
  }

  backend.proc.rpc.on("serverNotification", onServerNotification);
  backend.proc.rpc.on("serverRequest", onServerRequest);

  ws.on("close", () => {
    try {
      backend.proc.rpc.off("serverNotification", onServerNotification);
      backend.proc.rpc.off("serverRequest", onServerRequest);
    } catch {
      // ignore
    }
  });

  ws.on("message", async (data) => {
    let msg;
    try {
      msg = JSON.parse(String(data));
    } catch {
      wsSend(ws, { type: "toast", kind: "error", message: "webview: invalid json", timeoutMs: 4000 });
      return;
    }
    if (!msg || typeof msg !== "object") return;

    try {
      if (msg.type === "webview.postMessage") {
        const inner = msg.msg;
        if (!inner || typeof inner !== "object") return;

        if (inner.type === "ready") {
          await sendFullState();
          return;
        }

        if (inner.type === "stateAck") {
          // best-effort; used for backpressure in the extension
          return;
        }

        if (inner.type === "selectSession") {
          const sessionId = typeof inner.sessionId === "string" ? inner.sessionId : null;
          if (!sessionId || !isIdSafe(sessionId)) throw new Error("sessionId が不正です");
          const chat = await readChatStore(root.id);
          if (!chat.sessions.some((s) => s.id === sessionId)) throw new Error("session が見つかりません");
          chat.activeSessionId = sessionId;
          await writeChatStore(root.id, chat);
          await sendControlState();
          await sendBlocksReset(sessionId);
          return;
        }

        if (inner.type === "newSessionPickFolder") {
          // Web では root 選択 UI を親側に寄せる予定。
          // まずは現在の root に新規 session を作成する。
          const chat = await readChatStore(root.id);
          const id = crypto.randomUUID();
          const title = `chat ${chat.sessions.length + 1}`;
          const res = await backend.proc.threadStart({
            model: null,
            modelProvider: null,
            cwd: root.absPath,
            approvalPolicy: "on-request",
            sandbox: "workspace-write",
            config: null,
            baseInstructions: null,
            developerInstructions: null,
            experimentalRawEvents: false,
          });
          const threadId = res?.thread?.id ?? null;
          if (!threadId) throw new Error("thread/start failed: missing threadId");
          chat.sessions.push({ id, title, threadId });
          chat.activeSessionId = id;
          await writeChatStore(root.id, chat);
          await sendFullState();
          return;
        }

        if (inner.type === "send") {
          const text = typeof inner.text === "string" ? inner.text : "";
          if (!text.trim()) return;
          const chat = await readChatStore(root.id);
          const activeSessionId = chat.activeSessionId;
          if (!activeSessionId) {
            wsSend(ws, { type: "toast", kind: "error", message: "session がありません（New を押してください）", timeoutMs: 3500 });
            return;
          }
          const sess = chat.sessions.find((s) => s.id === activeSessionId) ?? null;
          if (!sess) throw new Error("active session が見つかりません");

          const userBlock = { id: crypto.randomUUID(), type: "user", text };
          wsSend(ws, { type: "blockUpsert", sessionId: activeSessionId, block: userBlock });

          const msgs = chat.messagesBySession?.[activeSessionId] ?? [];
          msgs.push({ id: userBlock.id, role: "user", text });
          chat.messagesBySession = { ...(chat.messagesBySession ?? {}), [activeSessionId]: msgs };
          await writeChatStore(root.id, chat);

          let threadId = sess.threadId;
          if (!threadId) {
            const res = await backend.proc.threadStart({
              model: null,
              modelProvider: null,
              cwd: root.absPath,
              approvalPolicy: "on-request",
              sandbox: "workspace-write",
              config: null,
              baseInstructions: null,
              developerInstructions: null,
              experimentalRawEvents: false,
            });
            threadId = res?.thread?.id ?? null;
            if (!threadId) throw new Error("thread/start failed: missing threadId");
            sess.threadId = threadId;
            await writeChatStore(root.id, chat);
          }

          const assistantBlockId = crypto.randomUUID();
          assistantByThreadId.set(threadId, { sessionId: activeSessionId, blockId: assistantBlockId, text: "" });
          wsSend(ws, { type: "blockUpsert", sessionId: activeSessionId, block: blockAssistantPlaceholder(assistantBlockId) });

          const input = [{ type: "text", text }];
          await backend.proc.turnStart({
            threadId,
            input,
            cwd: null,
            approvalPolicy: null,
            sandboxPolicy: null,
            model: null,
            effort: null,
            summary: null,
          });
          await sendControlState();
          return;
        }

        if (inner.type === "stop") {
          const chat = await readChatStore(root.id);
          const activeSessionId = chat.activeSessionId;
          const sess = activeSessionId ? chat.sessions.find((s) => s.id === activeSessionId) ?? null : null;
          if (!sess?.threadId) return;
          const turnId = activeTurnByThreadId.get(sess.threadId);
          if (!turnId) return;
          await backend.proc.turnInterrupt({ threadId: sess.threadId, turnId });
          return;
        }

        if (inner.type === "approve") {
          const requestKey = typeof inner.requestKey === "string" ? inner.requestKey : null;
          const decision = normalizeApprovalDecision(inner.decision);
          if (!requestKey || !decision) throw new Error("approve が不正です");
          const ap = approvalsByKey.get(requestKey);
          if (!ap) throw new Error("unknown approval request");
          approvalsByKey.delete(requestKey);
          backend.proc.rpc.respond(ap.requestId, { decision });
          await sendControlState();
          return;
        }

        if (inner.type === "requestUserInputResponse") {
          const requestKey = typeof inner.requestKey === "string" ? inner.requestKey : null;
          const response = inner.response ?? null;
          if (!requestKey) throw new Error("requestKey が不正です");
          const pending = requestUserInputByKey.get(requestKey);
          if (!pending) throw new Error("unknown requestUserInput request");
          requestUserInputByKey.delete(requestKey);
          backend.proc.rpc.respond(pending.requestId, response);
          return;
        }

        if (inner.type === "requestFileSearch") {
          const sessionId = typeof inner.sessionId === "string" ? inner.sessionId : null;
          const query = typeof inner.query === "string" ? inner.query : null;
          if (!sessionId || !query) return;
          const cancellationToken = crypto.randomUUID();
          const res = await backend.proc.fuzzyFileSearch({ query, roots: [root.absPath], cancellationToken });
          const paths = Array.isArray(res?.paths) ? res.paths : [];
          wsSend(ws, { type: "fileSearchResult", sessionId, query, paths });
          return;
        }

        if (inner.type === "requestSkillIndex") {
          const sessionId = typeof inner.sessionId === "string" ? inner.sessionId : null;
          if (!sessionId) return;
          const res = await backend.proc.skillsList({ cwds: [root.absPath], forceReload: false });
          wsSend(ws, { type: "skillIndex", sessionId, skills: res?.data ?? [] });
          return;
        }

        if (inner.type === "selectCliVariant") {
          wsSend(ws, { type: "toast", kind: "info", message: "CLI 切替は右上⚙（Webアプリ側）から行ってください", timeoutMs: 3500 });
          return;
        }

        if (inner.type === "openExternal") {
          const url = typeof inner.url === "string" ? inner.url : null;
          if (url) {
            wsSend(ws, { type: "toast", kind: "info", message: `openExternal は未実装です: ${url}`, timeoutMs: 3500 });
          }
          return;
        }

        if (inner.type === "openFile") {
          const p = typeof inner.path === "string" ? inner.path : null;
          if (p) {
            wsSend(ws, { type: "toast", kind: "info", message: `openFile は未実装です: ${p}`, timeoutMs: 3500 });
          }
          return;
        }

        if (inner.type === "uiError") {
          const message = typeof inner.message === "string" ? inner.message : "uiError";
          wsSend(ws, { type: "toast", kind: "error", message, timeoutMs: 5000 });
          return;
        }

        // 未実装: resumeFromHistory, reloadSession, openDiff, showStatus, sessionMenu, rename 等
        wsSend(ws, { type: "toast", kind: "info", message: `未実装のUIメッセージ: ${String(inner.type)}`, timeoutMs: 2500 });
        return;
      }

      if (msg.type === "webview.refresh") {
        await sendFullState();
        return;
      }

      if (msg.type === "webview.setState") {
        await writeWebviewState(root.id, { version: 1, state: msg.state ?? null });
        return;
      }

      wsSend(ws, { type: "toast", kind: "error", message: `webview: unknown message type: ${String(msg.type)}`, timeoutMs: 4000 });
    } catch (e) {
      wsSend(ws, { type: "toast", kind: "error", message: e instanceof Error ? e.message : String(e), timeoutMs: 6000 });
    }
  });
});

const wss = new WebSocketServer({ server, path: "/api/ws" });
wss.on("connection", (ws) => {
  let subscribedRootId = null;

  ws.on("message", async (data) => {
    let msg;
    try {
      msg = JSON.parse(String(data));
    } catch {
      wsSend(ws, { type: "error", message: "invalid json" });
      return;
    }
    if (!msg || typeof msg !== "object") {
      wsSend(ws, { type: "error", message: "invalid message" });
      return;
    }

    try {
      if (msg.type === "subscribe") {
        const rootId = msg.rootId;
        if (typeof rootId !== "string") throw new Error("rootId が必要です");
        const root = await requireWorkspaceRoot(rootId);
        const s = await getOrStartAppServer(root);
        subscribedRootId = rootId;
        s.subscribers.add(ws);
        wsSend(ws, { type: "subscribed", rootId });
        return;
      }

      // subscribe 必須
      if (!subscribedRootId) {
        throw new Error("not subscribed");
      }

      const root = await requireWorkspaceRoot(subscribedRootId);
      const state = await getOrStartAppServer(root);

      if (msg.type === "approval.respond") {
        const requestId = msg.requestId;
        const decision = normalizeApprovalDecision(msg.decision);
        if (typeof requestId !== "number") throw new Error("requestId が不正です");
        if (!decision) throw new Error("decision が不正です");
        if (!state.pending.has(requestId)) throw new Error("unknown pending request");
        state.pending.delete(requestId);
        state.proc.rpc.respond(requestId, { decision });
        wsSend(ws, { type: "ok" });
        return;
      }

      if (msg.type === "ask.respond") {
        const requestId = msg.requestId;
        const result = msg.result;
        if (typeof requestId !== "number") throw new Error("requestId が不正です");
        if (!isObject(result)) throw new Error("result が不正です");
        if (!state.pending.has(requestId)) throw new Error("unknown pending request");
        state.pending.delete(requestId);
        state.proc.rpc.respond(requestId, result);
        wsSend(ws, { type: "ok" });
        return;
      }

      if (msg.type === "chat.send") {
        const sessionId = msg.sessionId;
        const text = msg.text;
        if (!isIdSafe(sessionId)) throw new Error("sessionId が不正です");
        if (typeof text !== "string" || !text.trim()) throw new Error("text が不正です");

        const chat = await readChatStore(root.id);
        const idx = chat.sessions.findIndex((s) => s.id === sessionId);
        if (idx < 0) throw new Error("session が見つかりません");
        const sess = chat.sessions[idx];

        let threadId = sess.threadId;
        if (!threadId) {
          const res = await state.proc.threadStart({
            model: null,
            modelProvider: null,
            cwd: root.absPath,
            approvalPolicy: "on-request",
            sandbox: "workspace-write",
            config: null,
            baseInstructions: null,
            developerInstructions: null,
            experimentalRawEvents: false,
          });
          threadId = res?.thread?.id ?? null;
          if (!threadId) throw new Error("thread/start failed: missing threadId");
          chat.sessions[idx] = { ...sess, threadId };
          await writeChatStore(root.id, chat);
        }

        const input = [{ type: "text", text }];
        await state.proc.turnStart({
          threadId,
          input,
          cwd: null,
          approvalPolicy: null,
          sandboxPolicy: null,
          model: null,
          effort: null,
          summary: null,
        });
        wsSend(ws, { type: "chat.sent", sessionId, threadId });
        return;
      }

      if (msg.type === "thread.list") {
        const cursor = msg.cursor ?? null;
        const limit = msg.limit ?? 50;
        const res = await state.proc.threadList({ cursor, limit, modelProviders: null });
        wsSend(ws, { type: "thread.list.result", data: res?.data ?? [], nextCursor: res?.nextCursor ?? null });
        return;
      }

      wsSend(ws, { type: "error", message: `unknown type: ${String(msg.type)}` });
    } catch (e) {
      wsSend(ws, { type: "error", message: e instanceof Error ? e.message : String(e) });
    }
  });

  ws.on("close", () => {
    if (!subscribedRootId) return;
    const s = appServers.get(subscribedRootId);
    s?.subscribers.delete(ws);
  });
});

server.listen(port, host, () => {
  console.log(`[web] server listening on http://${host}:${port}`);
  console.log(`[web] HOME allowlist: ${homeReal}`);
  console.log(`[web] ws: /api/ws`);
});
