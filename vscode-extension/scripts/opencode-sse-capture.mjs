#!/usr/bin/env node
import { spawn } from "node:child_process";
import { mkdir, open } from "node:fs/promises";
import { dirname, resolve } from "node:path";

function parseArgs(argv) {
  const out = {
    directory: process.cwd(),
    out: null,
    baseUrl: null,
    spawn: true,
    spawnPort: 0,
    durationMs: 12_000,
    exercise: true,
    summaryOut: null,
  };
  const rest = [...argv];
  while (rest.length > 0) {
    const tok = rest.shift();
    if (tok === "--directory") out.directory = String(rest.shift() ?? "");
    else if (tok === "--out") out.out = String(rest.shift() ?? "");
    else if (tok === "--summary-out")
      out.summaryOut = String(rest.shift() ?? "");
    else if (tok === "--base-url") out.baseUrl = String(rest.shift() ?? "");
    else if (tok === "--spawn") out.spawn = true;
    else if (tok === "--no-spawn") out.spawn = false;
    else if (tok === "--spawn-port") out.spawnPort = Number(rest.shift() ?? "");
    else if (tok === "--exercise") out.exercise = true;
    else if (tok === "--no-exercise") out.exercise = false;
    else if (tok === "--duration-ms")
      out.durationMs = Number(rest.shift() ?? "");
    else if (tok === "--help" || tok === "-h") out.help = true;
    else throw new Error(`Unknown arg: ${tok}`);
  }
  if (!out.directory) throw new Error("--directory is required");
  if (out.out === null || out.out === "") {
    const ts = new Date().toISOString().replace(/[:.]/g, "-");
    out.out = resolve(process.cwd(), `dev/tmp/opencode-sse-${ts}.jsonl`);
  }
  if (out.summaryOut === null || out.summaryOut === "") {
    const ts = new Date().toISOString().replace(/[:.]/g, "-");
    out.summaryOut = resolve(process.cwd(), `.memo/opencode-sse-${ts}.md`);
  }
  if (!Number.isFinite(out.durationMs) || out.durationMs < 1) {
    throw new Error(`Invalid --duration-ms: ${String(out.durationMs)}`);
  }
  if (
    !Number.isFinite(out.spawnPort) ||
    out.spawnPort < 0 ||
    !Number.isInteger(out.spawnPort)
  ) {
    throw new Error(`Invalid --spawn-port: ${String(out.spawnPort)}`);
  }
  if (out.baseUrl && out.spawn) {
    throw new Error("Use either --base-url or --spawn/--no-spawn (not both).");
  }
  return out;
}

function usage() {
  return [
    "opencode SSE capture",
    "",
    "Usage:",
    "  node vscode-extension/scripts/opencode-sse-capture.mjs [options]",
    "",
    "Options:",
    "  --directory <path>       directory query param (default: cwd)",
    "  --out <path>             JSONL output path (default: dev/tmp/opencode-sse-<ts>.jsonl)",
    "  --summary-out <path>     Markdown summary path (default: .memo/opencode-sse-<ts>.md)",
    "  --duration-ms <n>        capture duration in ms (default: 12000)",
    "  --base-url <url>         connect to an existing opencode server (disables spawning)",
    "  --spawn / --no-spawn     spawn `opencode serve` (default: spawn)",
    "  --spawn-port <n>         when spawning, use a fixed port (0=auto; default: 0)",
    "  --exercise / --no-exercise  run a small set of API calls to generate events (default: exercise)",
  ].join("\n");
}

async function waitFor(predicate, timeoutMs, intervalMs = 50) {
  const startedAt = Date.now();
  for (;;) {
    const v = predicate();
    if (v) return v;
    if (Date.now() - startedAt > timeoutMs) {
      throw new Error("Timed out waiting for condition");
    }
    await new Promise((r) => setTimeout(r, intervalMs));
  }
}

async function spawnOpencodeServer({ cwd, command, args }) {
  const child = spawn(command, args, {
    cwd,
    stdio: ["ignore", "pipe", "pipe"],
    env: process.env,
  });

  const stripAnsi = (s) => String(s).replace(/\u001b\[[0-9;]*[A-Za-z]/g, "");
  let resolved = null;
  let spawnError = null;
  let exited = null;

  child.on("error", (err) => {
    spawnError = err instanceof Error ? err : new Error(String(err));
  });
  child.on("exit", (code, signal) => {
    if (!resolved) exited = { code, signal };
  });

  const onLine = (line) => {
    const s = stripAnsi(String(line ?? "")).trimEnd();
    const m = s.match(/opencode server listening on (https?:\/\/\S+)/i);
    if (!resolved && m && m[1]) {
      try {
        resolved = new URL(m[1]);
      } catch {
        // ignore malformed URL; keep waiting
      }
    }
  };

  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stdout.on("data", (d) => String(d).split(/\r?\n/).forEach(onLine));
  child.stderr.on("data", (d) => String(d).split(/\r?\n/).forEach(onLine));

  try {
    const baseUrl = await waitFor(() => {
      if (spawnError) throw spawnError;
      if (exited) {
        throw new Error(
          `opencode server exited before printing baseUrl (code=${String(exited.code)} signal=${String(exited.signal)})`,
        );
      }
      return resolved;
    }, 60_000);
    return {
      baseUrl,
      dispose: () => {
        try {
          child.kill();
        } catch {
          // ignore
        }
      },
    };
  } catch (err) {
    try {
      child.kill();
    } catch {
      // ignore
    }
    throw err;
  }
}

async function fetchJson(
  baseUrl,
  pathname,
  { method = "GET", query, body } = {},
) {
  const url = new URL(pathname, baseUrl);
  if (query) {
    for (const [k, v] of Object.entries(query)) {
      url.searchParams.set(k, String(v));
    }
  }
  const res = await fetch(url, {
    method,
    headers: body ? { "content-type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await res.text();
  let parsed;
  try {
    parsed = text ? JSON.parse(text) : null;
  } catch {
    parsed = text;
  }
  if (!res.ok) {
    const detail =
      typeof parsed === "string" ? parsed : JSON.stringify(parsed ?? null);
    throw new Error(
      `${method} ${pathname} failed: ${res.status} ${res.statusText} body=${detail}`,
    );
  }
  return parsed;
}

function sseConnect(url, { onRawEvent, onError, signal }) {
  return (async () => {
    const res = await fetch(url, {
      method: "GET",
      headers: { accept: "text/event-stream" },
      signal,
    });
    if (!res.ok)
      throw new Error(`SSE connect failed: ${res.status} ${res.statusText}`);
    if (!res.body) throw new Error("SSE response has no body");

    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let buf = "";

    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      buf += decoder.decode(value, { stream: true });
      for (;;) {
        const idx = buf.indexOf("\n\n");
        if (idx === -1) break;
        const raw = buf.slice(0, idx);
        buf = buf.slice(idx + 2);
        const dataLines = raw
          .split("\n")
          .map((l) => l.trimEnd())
          .filter((l) => l.startsWith("data:"))
          .map((l) => l.slice("data:".length).trimStart());
        if (dataLines.length === 0) continue;
        onRawEvent(dataLines.join("\n"));
      }
    }
  })().catch((err) => {
    if ((err && err.name) === "AbortError") return;
    onError(err);
  });
}

function summarizeJsonlRecords(records) {
  const typeCounts = new Map();
  const firstByType = new Map();

  for (const r of records) {
    if (r.kind !== "event") continue;
    if (!r.parsedOk) continue;
    const evt = r.event;
    const payload = evt && evt.payload ? evt.payload : evt;
    const type = typeof payload?.type === "string" ? payload.type : "(unknown)";
    typeCounts.set(type, (typeCounts.get(type) ?? 0) + 1);
    if (!firstByType.has(type)) firstByType.set(type, payload);
  }

  const ordered = [...typeCounts.entries()].sort((a, b) => b[1] - a[1]);
  return { ordered, firstByType };
}

function renderSummaryMarkdown({
  baseUrl,
  directory,
  outPath,
  ordered,
  firstByType,
}) {
  const lines = [];
  lines.push("# opencode SSE 観測ログ（実測）");
  lines.push("");
  lines.push(`- baseUrl: \`${String(baseUrl)}\``);
  lines.push(`- directory: \`${directory}\``);
  lines.push(`- raw log: \`${outPath}\``);
  lines.push("");
  lines.push("## event type counts");
  lines.push("");
  for (const [t, c] of ordered) lines.push(`- ${String(c)}: \`${t}\``);
  lines.push("");
  lines.push("## sample payloads (first seen)");
  lines.push("");
  for (const [t] of ordered) {
    const sample = firstByType.get(t);
    lines.push(`### ${t}`);
    lines.push("");
    lines.push("```json");
    const raw = JSON.stringify(sample, null, 2);
    // Keep the markdown readable. Full raw events are in JSONL.
    lines.push(
      raw.length > 4000 ? raw.slice(0, 4000) + "\n...<truncated>" : raw,
    );
    lines.push("```");
    lines.push("");
  }
  return lines.join("\n");
}

async function spawnOpencodeChild({ cwd, command, args }) {
  const child = spawn(command, args, {
    cwd,
    stdio: ["ignore", "pipe", "pipe"],
    env: process.env,
  });
  let spawnError = null;
  child.on("error", (err) => {
    spawnError = err instanceof Error ? err : new Error(String(err));
  });
  // Give the process a brief moment to emit immediate spawn errors (e.g. ENOENT).
  await new Promise((r) => setTimeout(r, 200));
  if (spawnError) {
    try {
      child.kill();
    } catch {
      // ignore
    }
    throw spawnError;
  }
  return {
    dispose: () => {
      try {
        child.kill();
      } catch {
        // ignore
      }
    },
  };
}

async function waitForHealth(baseUrl, timeoutMs) {
  const startedAt = Date.now();
  for (;;) {
    try {
      await fetchJson(baseUrl, "/global/health");
      return;
    } catch (err) {
      if (Date.now() - startedAt > timeoutMs) {
        throw new Error(
          `Timed out waiting for /global/health: ${String(err?.message ?? err)}`,
        );
      }
      await new Promise((r) => setTimeout(r, 100));
    }
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    process.stdout.write(usage() + "\n");
    return;
  }

  await mkdir(dirname(args.out), { recursive: true });
  await mkdir(dirname(args.summaryOut), { recursive: true });

  const outHandle = await open(args.out, "w");
  const records = [];
  const writeRecord = async (rec) => {
    records.push(rec);
    await outHandle.write(`${JSON.stringify(rec)}\n`);
  };

  let server = null;
  let baseUrl = null;
  if (args.spawn) {
    const command = "opencode";
    const hostname = "127.0.0.1";
    const spawnArgs = [
      "serve",
      "--hostname",
      hostname,
      "--port",
      String(args.spawnPort),
      "--print-logs",
    ];
    if (args.spawnPort > 0) {
      server = await spawnOpencodeChild({
        cwd: args.directory,
        command,
        args: spawnArgs,
      });
      baseUrl = new URL(`http://${hostname}:${args.spawnPort}`);
      await waitForHealth(baseUrl, 60_000);
    } else {
      server = await spawnOpencodeServer({
        cwd: args.directory,
        command,
        args: spawnArgs,
      });
      baseUrl = server.baseUrl;
    }
  } else if (args.baseUrl) {
    baseUrl = new URL(args.baseUrl);
  } else {
    throw new Error(
      "Either --base-url must be set, or --spawn must be enabled.",
    );
  }

  await writeRecord({
    kind: "meta",
    capturedAtMs: Date.now(),
    baseUrl: String(baseUrl),
    directory: args.directory,
    spawn: args.spawn,
    durationMs: args.durationMs,
    exercise: args.exercise,
  });

  const controller = new AbortController();
  const sseUrl = new URL("/event", baseUrl);
  sseUrl.searchParams.set("directory", args.directory);

  const onRawEvent = (raw) => {
    let event = null;
    let parsedOk = false;
    try {
      event = JSON.parse(raw);
      parsedOk = true;
    } catch {
      event = null;
    }
    void writeRecord({
      kind: "event",
      receivedAtMs: Date.now(),
      raw,
      parsedOk,
      event,
    });
  };

  const onError = (err) => {
    void writeRecord({
      kind: "error",
      receivedAtMs: Date.now(),
      error: String(err?.message ?? err),
    });
  };

  const sseTask = sseConnect(sseUrl, {
    onRawEvent,
    onError,
    signal: controller.signal,
  });

  if (args.exercise) {
    const writeClientError = async (op, err) => {
      await writeRecord({
        kind: "clientError",
        atMs: Date.now(),
        op,
        error: String(err?.message ?? err),
      });
    };

    try {
      await fetchJson(baseUrl, "/global/health");
    } catch (err) {
      await writeClientError("health", err);
    }

    let session = null;
    try {
      session = await fetchJson(baseUrl, "/session", {
        method: "POST",
        query: { directory: args.directory },
        body: {},
      });
      await writeRecord({
        kind: "client",
        atMs: Date.now(),
        op: "createSession",
        sessionID: String(session?.id ?? ""),
      });
    } catch (err) {
      await writeClientError("createSession", err);
    }

    if (session && typeof session?.id === "string" && session.id) {
      const sessionID = session.id;

      try {
        await fetchJson(
          baseUrl,
          `/session/${encodeURIComponent(sessionID)}/message`,
          {
            method: "POST",
            query: { directory: args.directory },
            body: {
              parts: [
                {
                  type: "text",
                  text: "このリポジトリのルートで `ls` を実行し、最初の5行を箇条書きで教えて。",
                },
              ],
            },
          },
        );
        await writeRecord({
          kind: "client",
          atMs: Date.now(),
          op: "prompt",
          sessionID,
        });
      } catch (err) {
        await writeClientError("prompt", err);
      }

      // Try summarize (can fail depending on providers/config).
      try {
        await fetchJson(
          baseUrl,
          `/session/${encodeURIComponent(sessionID)}/summarize`,
          {
            method: "POST",
            query: { directory: args.directory },
            body: { providerID: "openai", modelID: "gpt-5.2", auto: true },
          },
        );
        await writeRecord({
          kind: "client",
          atMs: Date.now(),
          op: "summarize",
          sessionID,
        });
      } catch (err) {
        await writeClientError("summarize", err);
      }
    }
  }

  await new Promise((r) => setTimeout(r, args.durationMs));
  controller.abort();
  await sseTask;
  if (server) server.dispose();
  await outHandle.close();

  const { ordered, firstByType } = summarizeJsonlRecords(records);
  const md = renderSummaryMarkdown({
    baseUrl,
    directory: args.directory,
    outPath: args.out,
    ordered,
    firstByType,
  });
  await mkdir(dirname(args.summaryOut), { recursive: true });
  const summaryHandle = await open(args.summaryOut, "w");
  await summaryHandle.write(md);
  await summaryHandle.close();

  process.stdout.write(`wrote: ${args.out}\n`);
  process.stdout.write(`wrote: ${args.summaryOut}\n`);
}

main().catch((err) => {
  process.stderr.write(String(err?.stack ?? err) + "\n");
  process.exitCode = 1;
});
