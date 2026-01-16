import { createServer } from "node:http";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { WebSocketServer } from "ws";

const port = Number(process.env.APP_SERVER_UI_PORT ?? 8787);
const codexBin = process.env.CODEX_BIN ?? "codex";
const explicitAppServerBin =
  process.env.APP_SERVER_BIN ?? process.env.CODEX_APP_SERVER_BIN ?? null;
const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..", "..");
const localAppServerBin = resolve(repoRoot, "codex-rs/target/debug/codex-app-server");
const appServerBin =
  explicitAppServerBin ?? (existsSync(localAppServerBin) ? localAppServerBin : null);

const sockets = new Set();
let appServer = null;

const broadcast = (payload) => {
  const message = JSON.stringify(payload);
  for (const ws of sockets) {
    if (ws.readyState === ws.OPEN) {
      ws.send(message);
    }
  }
};

const startAppServer = () => {
  if (appServer?.child?.exitCode === null) {
    return appServer;
  }

  const command = appServerBin ?? codexBin;
  const args = appServerBin ? [] : ["app-server"];
  const child = spawn(command, args, {
    stdio: ["pipe", "pipe", "pipe"],
    env: process.env,
  });

  const stdout = child.stdout;
  const stderr = child.stderr;
  const stdoutRl = stdout ? createInterface({ input: stdout }) : null;
  const stderrRl = stderr ? createInterface({ input: stderr }) : null;

  stdoutRl?.on("line", (line) => {
    const trimmed = line.trim();
    if (!trimmed) {
      return;
    }

    let payload = trimmed;
    try {
      const parsed = JSON.parse(trimmed);
      payload = JSON.stringify(parsed);
    } catch {
      payload = JSON.stringify({
        method: "ui/raw",
        params: { line: trimmed },
      });
    }

    for (const ws of sockets) {
      if (ws.readyState === ws.OPEN) {
        ws.send(payload);
      }
    }
  });

  stderrRl?.on("line", (line) => {
    const trimmed = line.trim();
    if (!trimmed) {
      return;
    }
    console.error(trimmed);
    broadcast({ method: "ui/stderr", params: { line: trimmed } });
  });

  child.on("error", (err) => {
    console.error("codex app-server spawn error:", err);
    broadcast({ method: "ui/error", params: { message: "Failed to spawn app-server.", details: String(err) } });
    appServer = null;
  });

  child.on("exit", (code, signal) => {
    console.log(`codex app-server exited (code=${code ?? "null"}, signal=${signal ?? "null"})`);
    broadcast({ method: "ui/exit", params: { code, signal } });
    appServer = null;
  });

  appServer = { child, stdoutRl, stderrRl };
  return appServer;
};

const server = createServer((req, res) => {
  if (req.url === "/health") {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ status: "ok" }));
    return;
  }

  res.writeHead(404, { "content-type": "text/plain" });
  res.end("Not found");
});

const wss = new WebSocketServer({ server });

wss.on("connection", (ws) => {
  sockets.add(ws);
  const running = Boolean(appServer?.child?.exitCode === null);
  ws.send(
    JSON.stringify({
      method: "ui/connected",
      params: { pid: appServer?.child?.pid ?? null, running },
    }),
  );

  ws.on("close", () => {
    sockets.delete(ws);
  });

  ws.on("message", (data) => {
    const text = typeof data === "string" ? data : data.toString("utf8");
    if (!text.trim()) {
      return;
    }

    let parsed;
    try {
      parsed = JSON.parse(text);
    } catch (err) {
      ws.send(
        JSON.stringify({
          method: "ui/error",
          params: {
            message: "Failed to parse JSON from client.",
            details: String(err),
          },
        }),
      );
      return;
    }

    if (!appServer || appServer.child.exitCode !== null || !appServer.child.stdin?.writable) {
      startAppServer();
    }

    if (!appServer || !appServer.child.stdin?.writable) {
      ws.send(
        JSON.stringify({
          method: "ui/error",
          params: {
            message: "app-server stdin is closed.",
          },
        }),
      );
      return;
    }

    appServer.child.stdin.write(`${JSON.stringify(parsed)}\n`);
  });
});

server.listen(port, () => {
  console.log(`App server bridge listening on ws://localhost:${port}`);
});

startAppServer();

const shutdown = () => {
  appServer?.stdoutRl?.close();
  appServer?.stderrRl?.close();
  wss.close();
  server.close();
  appServer?.child?.kill("SIGTERM");
};

process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);
