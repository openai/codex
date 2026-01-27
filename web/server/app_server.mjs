import { spawn } from "node:child_process";
import { EventEmitter } from "node:events";
import readline from "node:readline";

function isObject(value) {
  return typeof value === "object" && value !== null;
}

export class RpcClient extends EventEmitter {
  #nextId = 0;
  #pending = new Map(); // id -> {resolve,reject}
  #stdoutRl;
  #stderrRl;
  #stdinError = null;
  #log;
  #logRpc;

  constructor(child, args) {
    super();
    this.child = child;
    this.#log = args.log ?? (() => {});
    this.#logRpc = Boolean(args.logRpcPayloads);

    this.#stdoutRl = readline.createInterface({ input: this.child.stdout });
    this.#stderrRl = readline.createInterface({ input: this.child.stderr });

    this.#stdoutRl.on("line", (line) => this.#onStdoutLine(line));
    this.#stderrRl.on("line", (line) => this.#log(`[app-server stderr] ${line}`));

    this.child.stdin.on("error", (err) => {
      const e = err instanceof Error ? err : new Error(String(err));
      this.#stdinError = e;
      this.#log(`app-server stdin error: ${e.message}`);
      for (const { reject } of this.#pending.values()) reject(e);
      this.#pending.clear();
    });
    this.child.stdin.on("close", () => {
      if (this.#stdinError) return;
      const e = new Error("app-server stdin closed");
      this.#stdinError = e;
      this.#log(e.message);
      for (const { reject } of this.#pending.values()) reject(e);
      this.#pending.clear();
    });

    this.child.on("exit", (code, signal) => {
      this.#log(`app-server exited: code=${code ?? "null"} signal=${signal ?? "null"}`);
      for (const { reject } of this.#pending.values()) reject(new Error("app-server exited"));
      this.#pending.clear();
      this.emit("exit", { code: code ?? null, signal: signal ?? null });
    });
    this.child.on("error", (err) => {
      this.#log(`app-server process error: ${String(err)}`);
      this.emit("error", err);
    });
  }

  dispose() {
    this.#stdoutRl.close();
    this.#stderrRl.close();
    for (const { reject } of this.#pending.values()) reject(new Error("RpcClient disposed"));
    this.#pending.clear();
    this.removeAllListeners();
  }

  request(request) {
    const id = this.#nextId++;
    const payload = { ...request, id };
    this.#writeJson(payload);
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve, reject });
    });
  }

  notify(notification) {
    this.#writeJson(notification);
  }

  respond(id, result) {
    this.#writeJson({ id, result });
  }

  #writeJson(obj) {
    const line = JSON.stringify(obj);
    if (this.#logRpc) this.#log(`[rpc ->] ${line}`);
    if (this.#stdinError) throw this.#stdinError;
    this.child.stdin.write(`${line}\n`);
  }

  #onStdoutLine(line) {
    if (!line.trim()) return;
    if (this.#logRpc) this.#log(`[rpc <-] ${line}`);

    let msg;
    try {
      msg = JSON.parse(line);
    } catch (err) {
      this.#log(`Failed to parse app-server JSONL message: ${String(err)}; line=${line}`);
      return;
    }
    if (!isObject(msg)) {
      this.#log(`Unexpected JSON-RPC message: ${line}`);
      return;
    }

    const id = msg.id;
    const method = msg.method;

    if (id !== undefined && method === undefined) {
      const pending = this.#pending.get(id);
      if (!pending) {
        this.#log(`No pending request for id=${String(id)}`);
        return;
      }
      this.#pending.delete(id);
      if (msg.error !== undefined) pending.reject(msg.error);
      else pending.resolve(msg.result);
      return;
    }

    if (typeof method === "string") {
      if (id !== undefined) this.emit("serverRequest", msg);
      else this.emit("serverNotification", msg);
      return;
    }

    this.#log(`Unrecognized JSON-RPC message shape: ${line}`);
  }
}

export class AppServerProcess {
  constructor(args) {
    this.command = args.command;
    this.cwd = args.cwd;
    this.log = args.log ?? (() => {});
    this.logRpcPayloads = Boolean(args.logRpcPayloads);
    this.child = null;
    this.rpc = null;
  }

  async start() {
    if (this.child) return;
    const child = spawn(this.command, ["app-server"], {
      cwd: this.cwd,
      stdio: ["pipe", "pipe", "pipe"],
      env: process.env,
    });
    this.child = child;
    this.rpc = new RpcClient(child, { log: this.log, logRpcPayloads: this.logRpcPayloads });

    const params = {
      clientInfo: { name: "codez-web", title: "Codez Web", version: "0.0.1" },
    };
    const result = await this.rpc.request({ method: "initialize", params });
    this.log(`Initialized app-server (userAgent=${String(result?.userAgent ?? "unknown")})`);
    this.rpc.notify({ method: "initialized" });
  }

  dispose() {
    try {
      this.rpc?.dispose();
    } finally {
      this.rpc = null;
      try {
        this.child?.kill();
      } catch {
        // ignore
      }
      this.child = null;
    }
  }

  // Thin wrappers
  threadStart(params) {
    return this.rpc.request({ method: "thread/start", params });
  }
  threadResume(params) {
    return this.rpc.request({ method: "thread/resume", params });
  }
  threadReload(params) {
    return this.rpc.request({ method: "thread/reload", params });
  }
  threadList(params) {
    return this.rpc.request({
      method: "thread/list",
      params: {
        cursor: params?.cursor ?? null,
        limit: params?.limit ?? null,
        modelProviders: params?.modelProviders ?? null,
      },
    });
  }
  turnStart(params) {
    return this.rpc.request({ method: "turn/start", params });
  }
  turnInterrupt(params) {
    return this.rpc.request({ method: "turn/interrupt", params });
  }
  skillsList(params) {
    return this.rpc.request({ method: "skills/list", params });
  }
  fuzzyFileSearch(params) {
    return this.rpc.request({ method: "fuzzyFileSearch", params });
  }
  modelList(params) {
    return this.rpc.request({
      method: "model/list",
      params: { cursor: params?.cursor ?? null, limit: params?.limit ?? null },
    });
  }
  accountRead(params) {
    return this.rpc.request({ method: "account/read", params });
  }
  accountRateLimitsRead() {
    return this.rpc.request({ method: "account/rateLimits/read", params: undefined });
  }
}

