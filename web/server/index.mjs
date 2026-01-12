import crypto from "node:crypto";
import fs from "node:fs/promises";
import fssync from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import express from "express";
import mime from "mime";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const webRoot = path.resolve(__dirname, "..");
const dataDir = path.join(webRoot, ".data");
const storePath = path.join(dataDir, "workspace.json");
const distDir = path.join(webRoot, "dist");

const home = os.homedir();
const homeReal = await fs.realpath(home);

const MAX_TEXT_BYTES = 2 * 1024 * 1024;

function jsonError(res, status, message) {
  res.status(status).type("text/plain; charset=utf-8").send(message);
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
    return parsed;
  } catch (e) {
    if (e && typeof e === "object" && "code" in e && e.code === "ENOENT") {
      const init = { version: 1, roots: [] };
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
app.use(express.json({ limit: "1mb" }));

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
    res.json({ version: 1, roots: sortRoots(st.roots) });
  } catch (e) {
    return jsonError(res, 500, e instanceof Error ? e.message : String(e));
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
    const next = {
      ...st,
      roots: [
        ...st.roots,
        { id, label: rootLabel, absPath: real, createdAt: new Date().toISOString(), order: nextOrder },
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
    const { label } = req.body ?? {};
    if (typeof label !== "string" || label.trim().length === 0) {
      return jsonError(res, 400, "label が不正です");
    }
    const st = await readStore();
    const idx = st.roots.findIndex((r) => r.id === id);
    if (idx < 0) return jsonError(res, 404, "root が見つかりません");
    const roots = [...st.roots];
    roots[idx] = { ...roots[idx], label: label.trim() };
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

if (fssync.existsSync(distDir)) {
  app.use(express.static(distDir));
  app.get("*", (req, res, next) => {
    if (req.path.startsWith("/api/")) return next();
    res.sendFile(path.join(distDir, "index.html"));
  });
}

const port = Number(process.env.PORT ?? 3000);
const host = String(process.env.HOST ?? "0.0.0.0");
app.listen(port, host, () => {
  console.log(`[web] server listening on http://${host}:${port}`);
  console.log(`[web] HOME allowlist: ${homeReal}`);
});
