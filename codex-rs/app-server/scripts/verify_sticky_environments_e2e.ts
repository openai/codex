#!/usr/bin/env -S ts-node --transpile-only

import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import * as fs from "node:fs";
import * as http from "node:http";
import * as os from "node:os";
import * as path from "node:path";
import * as readline from "node:readline";

type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };

type JsonRpcResponse = {
  id: number;
  result?: JsonValue;
  error?: { code: number; message: string; data?: JsonValue };
};

type JsonRpcNotification = {
  method: string;
  params?: JsonValue;
};

type Thread = {
  id: string;
};

type ThreadResponse = {
  thread: Thread;
};

type TurnEnvironment = {
  environmentId: string;
  cwd: string;
};

type ScriptOptions = {
  codexBin: string | null;
  keepTemp: boolean;
  requestTimeoutMs: number;
  verbose: boolean;
};

const scriptDir = __dirname;
const repoRoot = path.resolve(scriptDir, "../../..");
const defaultCodexBin = path.join(repoRoot, "codex-rs", "target", "debug", "codex");

function parseArgs(argv: string[]): ScriptOptions {
  const options: ScriptOptions = {
    codexBin: process.env.CODEX_BIN ?? null,
    keepTemp: false,
    requestTimeoutMs: Number(process.env.STICKY_ENV_E2E_TIMEOUT_MS ?? 120_000),
    verbose: false,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    assert(arg !== undefined, "missing argument");
    if (arg === "--codex-bin") {
      const value = argv[i + 1];
      assert(value, "--codex-bin requires a path");
      options.codexBin = value;
      i += 1;
    } else if (arg === "--keep-temp") {
      options.keepTemp = true;
    } else if (arg === "--timeout-ms") {
      const value = argv[i + 1];
      assert(value, "--timeout-ms requires a number");
      options.requestTimeoutMs = Number(value);
      assert(Number.isFinite(options.requestTimeoutMs), "--timeout-ms must be a finite number");
      i += 1;
    } else if (arg === "--verbose") {
      options.verbose = true;
    } else if (arg === "--help" || arg === "-h") {
      printUsageAndExit();
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  return options;
}

function printUsageAndExit(): void {
  console.log(`Usage:
  pnpm --dir sdk/typescript exec ts-node --transpile-only --compiler-options '{"module":"CommonJS","moduleResolution":"node"}' ../../codex-rs/app-server/scripts/verify_sticky_environments_e2e.ts [--codex-bin PATH] [--keep-temp] [--timeout-ms N] [--verbose]

If --codex-bin/CODEX_BIN is omitted, the script uses codex-rs/target/debug/codex
when present, otherwise it falls back to cargo run -p codex-cli --bin codex.`);
  process.exit(0);
}

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

function writeConfigToml(codexHome: string, serverUrl: string): void {
  fs.writeFileSync(
    path.join(codexHome, "config.toml"),
    `
model = "mock-model"
approval_policy = "never"
sandbox_mode = "danger-full-access"
model_provider = "mock_provider"

[features]
plugins = false
unified_exec = true

[model_providers.mock_provider]
name = "Mock provider for sticky environment e2e"
base_url = "${serverUrl}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
supports_websockets = false
`,
  );
}

function sse(events: JsonValue[]): string {
  return events
    .map((event) => {
      assert(
        event && typeof event === "object" && !Array.isArray(event),
        "SSE event must be an object",
      );
      const type = event.type;
      assert(typeof type === "string", "SSE event is missing string type");
      return `event: ${type}\ndata: ${JSON.stringify(event)}\n\n`;
    })
    .join("");
}

function assistantResponseSse(index: number): string {
  const responseId = `resp-${index}`;
  return sse([
    {
      type: "response.created",
      response: { id: responseId },
    },
    {
      type: "response.output_item.done",
      item: {
        type: "message",
        role: "assistant",
        id: `msg-${index}`,
        content: [{ type: "output_text", text: `done ${index}` }],
      },
    },
    {
      type: "response.completed",
      response: {
        id: responseId,
        usage: {
          input_tokens: 0,
          input_tokens_details: null,
          output_tokens: 0,
          output_tokens_details: null,
          total_tokens: 0,
        },
      },
    },
  ]);
}

class MockResponsesServer {
  private server: http.Server;
  private requestCount = 0;
  readonly requests: JsonValue[] = [];

  private constructor(server: http.Server) {
    this.server = server;
  }

  static async start(): Promise<MockResponsesServer> {
    const mock = new MockResponsesServer(
      http.createServer((req, res) => {
        void mock.handle(req, res);
      }),
    );
    await new Promise<void>((resolve) => {
      mock.server.listen(0, "127.0.0.1", resolve);
    });
    return mock;
  }

  url(): string {
    const address = this.server.address();
    assert(address && typeof address === "object", "mock server is not listening");
    return `http://127.0.0.1:${address.port}`;
  }

  close(): Promise<void> {
    return new Promise((resolve, reject) => {
      this.server.close((error) => (error ? reject(error) : resolve()));
    });
  }

  private async handle(req: http.IncomingMessage, res: http.ServerResponse): Promise<void> {
    if (req.method !== "POST") {
      res.writeHead(404).end();
      return;
    }

    const body = await readRequestBody(req);
    const parsed = JSON.parse(body) as JsonValue;
    this.requests.push(parsed);
    this.requestCount += 1;

    res.writeHead(200, {
      "Content-Type": "text/event-stream",
      "Cache-Control": "no-cache",
      Connection: "keep-alive",
    });
    res.end(assistantResponseSse(this.requestCount));
  }
}

function readRequestBody(req: http.IncomingMessage): Promise<string> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on("data", (chunk: Buffer) => chunks.push(chunk));
    req.on("error", reject);
    req.on("end", () => resolve(Buffer.concat(chunks).toString("utf8")));
  });
}

class AppServerClient {
  private child: ChildProcessWithoutNullStreams;
  private nextId = 1;
  private pending = new Map<number, (response: JsonRpcResponse) => void>();
  private notifications: JsonRpcNotification[] = [];
  private notificationWaiters: Array<{
    method: string;
    resolve: (notification: JsonRpcNotification) => void;
    reject: (error: Error) => void;
    timer: ReturnType<typeof setTimeout>;
  }> = [];
  private stderrLines: string[] = [];
  private requestTimeoutMs: number;
  private verbose: boolean;

  private constructor(child: ChildProcessWithoutNullStreams, options: ScriptOptions) {
    this.child = child;
    this.requestTimeoutMs = options.requestTimeoutMs;
    this.verbose = options.verbose;

    const stdout = readline.createInterface({ input: child.stdout });
    stdout.on("line", (line) => this.handleLine(line));
    child.stderr.on("data", (data: Buffer) => {
      const text = data.toString("utf8");
      if (this.verbose) {
        process.stderr.write(text);
      }
      for (const line of text.split(/\r?\n/)) {
        if (line.length > 0) {
          this.stderrLines.push(line);
        }
      }
      this.stderrLines = this.stderrLines.slice(-200);
    });
    child.on("exit", (code, signal) => {
      const error = new Error(`app-server exited with code=${code} signal=${signal}`);
      for (const resolve of this.pending.values()) {
        resolve({ id: -1, error: { code: -1, message: error.message } });
      }
      this.pending.clear();
      for (const waiter of this.notificationWaiters.splice(0)) {
        clearTimeout(waiter.timer);
        waiter.reject(error);
      }
    });
  }

  static start(codexHome: string, options: ScriptOptions): AppServerClient {
    const command = appServerCommand(options.codexBin);
    const child = spawn(command.program, command.args, {
      cwd: repoRoot,
      env: {
        ...process.env,
        CODEX_HOME: codexHome,
        RUST_LOG: process.env.RUST_LOG ?? "warn",
      },
      stdio: ["pipe", "pipe", "pipe"],
    });

    return new AppServerClient(child, options);
  }

  async initialize(): Promise<void> {
    await this.request("initialize", {
      clientInfo: {
        name: "sticky_environment_e2e",
        title: "Sticky Environment E2E",
        version: "0.1.0",
      },
      capabilities: {
        experimentalApi: true,
      },
    });
    this.notify("initialized", undefined);
  }

  async request(method: string, params: JsonValue | undefined): Promise<JsonValue> {
    const id = this.nextId;
    this.nextId += 1;
    const responsePromise = new Promise<JsonRpcResponse>((resolve) => {
      this.pending.set(id, resolve);
    });
    this.write({ method, id, params });
    const response = await withTimeout(
      responsePromise,
      this.requestTimeoutMs,
      `response for ${method}`,
    );
    if (response.error) {
      throw new Error(`${method} failed: ${response.error.message}`);
    }
    return response.result ?? {};
  }

  notify(method: string, params: JsonValue | undefined): void {
    this.write({ method, params });
  }

  waitForNotification(method: string, timeoutMs = 20_000): Promise<JsonRpcNotification> {
    const index = this.notifications.findIndex((notification) => notification.method === method);
    if (index >= 0) {
      const [notification] = this.notifications.splice(index, 1);
      assert(notification, `missing queued notification for ${method}`);
      return Promise.resolve(notification);
    }

    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.notificationWaiters = this.notificationWaiters.filter(
          (waiter) => waiter.timer !== timer,
        );
        reject(new Error(`timed out waiting for ${method}`));
      }, timeoutMs);
      this.notificationWaiters.push({ method, resolve, reject, timer });
    });
  }

  async stop(): Promise<void> {
    if (this.child.exitCode !== null) {
      return;
    }
    this.child.kill("SIGTERM");
    await new Promise<void>((resolve) => {
      const timer = setTimeout(() => {
        if (this.child.exitCode === null) {
          this.child.kill("SIGKILL");
        }
        resolve();
      }, 2_000);
      this.child.once("exit", () => {
        clearTimeout(timer);
        resolve();
      });
    });
  }

  stderrTail(): string {
    return this.stderrLines.join("\n");
  }

  private write(message: { method: string; id?: number; params?: JsonValue }): void {
    this.child.stdin.write(`${JSON.stringify(message)}\n`);
  }

  private handleLine(line: string): void {
    if (line.trim().length === 0) {
      return;
    }
    const message = JSON.parse(line) as JsonRpcResponse | JsonRpcNotification;
    if ("id" in message) {
      const resolve = this.pending.get(message.id);
      if (resolve) {
        this.pending.delete(message.id);
        resolve(message);
      }
      return;
    }

    const waiterIndex = this.notificationWaiters.findIndex(
      (waiter) => waiter.method === message.method,
    );
    if (waiterIndex >= 0) {
      const [waiter] = this.notificationWaiters.splice(waiterIndex, 1);
      assert(waiter, `missing waiter for ${message.method}`);
      clearTimeout(waiter.timer);
      waiter.resolve(message);
      return;
    }
    this.notifications.push(message);
  }
}

function appServerCommand(codexBin: string | null): { program: string; args: string[] } {
  const bin = codexBin ?? (fs.existsSync(defaultCodexBin) ? defaultCodexBin : null);
  if (bin) {
    return { program: bin, args: ["app-server", "--listen", "stdio://"] };
  }

  return {
    program: "cargo",
    args: [
      "run",
      "--manifest-path",
      path.join(repoRoot, "codex-rs", "Cargo.toml"),
      "-p",
      "codex-cli",
      "--bin",
      "codex",
      "--",
      "app-server",
      "--listen",
      "stdio://",
    ],
  };
}

async function withTimeout<T>(promise: Promise<T>, timeoutMs: number, label: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new Error(`timed out waiting for ${label}`)), timeoutMs);
  });
  try {
    return await Promise.race([promise, timeout]);
  } finally {
    if (timer) {
      clearTimeout(timer);
    }
  }
}

function asThreadResponse(result: JsonValue): ThreadResponse {
  assert(
    result && typeof result === "object" && !Array.isArray(result),
    "thread response must be an object",
  );
  const thread = result.thread;
  assert(
    thread && typeof thread === "object" && !Array.isArray(thread),
    "thread response is missing thread",
  );
  assert(typeof thread.id === "string", "thread response is missing thread.id");
  return { thread: { id: thread.id } };
}

function textInput(text: string): JsonValue[] {
  return [{ type: "text", text, textElements: [] }];
}

async function startThread(
  client: AppServerClient,
  params: Record<string, JsonValue>,
): Promise<Thread> {
  const result = await client.request("thread/start", {
    model: "mock-model",
    experimentalRawEvents: false,
    persistExtendedHistory: true,
    ...params,
  });
  return asThreadResponse(result).thread;
}

async function runTurn(
  client: AppServerClient,
  threadId: string,
  text: string,
  params: Record<string, JsonValue> = {},
): Promise<void> {
  await client.request("turn/start", {
    threadId,
    input: textInput(text),
    ...params,
  });
  await client.waitForNotification("turn/completed");
}

function latestRequest(mock: MockResponsesServer): JsonValue {
  const request = mock.requests.at(-1);
  assert(request, "mock Responses API did not receive a request");
  return request;
}

function toolNames(request: JsonValue): string[] {
  if (!request || typeof request !== "object" || Array.isArray(request)) {
    return [];
  }
  const tools = request.tools;
  if (!Array.isArray(tools)) {
    return [];
  }
  return tools
    .map((tool) => {
      if (!tool || typeof tool !== "object" || Array.isArray(tool)) {
        return null;
      }
      return typeof tool.name === "string"
        ? tool.name
        : typeof tool.type === "string"
          ? tool.type
          : null;
    })
    .filter((name): name is string => name !== null);
}

function assertEnvironmentToolsPresent(request: JsonValue, label: string): void {
  const tools = toolNames(request);
  assert(
    tools.includes("exec_command") || tools.includes("shell_command"),
    `${label}: expected an environment-backed shell tool, got tools=${JSON.stringify(tools)}`,
  );
}

function assertEnvironmentToolsAbsent(request: JsonValue, label: string): void {
  const tools = toolNames(request);
  for (const tool of [
    "exec_command",
    "shell_command",
    "write_stdin",
    "apply_patch",
    "view_image",
    "js_repl",
  ]) {
    assert(
      !tools.includes(tool),
      `${label}: expected ${tool} to be omitted, got tools=${JSON.stringify(tools)}`,
    );
  }
}

function assertRequestMentionsCwd(request: JsonValue, cwd: string, label: string): void {
  const body = JSON.stringify(request);
  assert(body.includes(cwd), `${label}: expected request to mention cwd ${cwd}`);
}

async function main(): Promise<void> {
  const options = parseArgs(process.argv.slice(2));
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "codex-sticky-env-e2e-"));
  const codexHome = path.join(tmp, "codex-home");
  const workspace = path.join(tmp, "workspace");
  const stickyCwd = path.join(workspace, "sticky");
  const turnOverrideCwd = path.join(workspace, "turn-override");
  const threadDefaultCwd = path.join(workspace, "thread-default");
  fs.mkdirSync(codexHome, { recursive: true });
  fs.mkdirSync(stickyCwd, { recursive: true });
  fs.mkdirSync(turnOverrideCwd, { recursive: true });
  fs.mkdirSync(threadDefaultCwd, { recursive: true });

  const mock = await MockResponsesServer.start();
  let appServer: AppServerClient | null = null;
  try {
    writeConfigToml(codexHome, mock.url());

    const stickyEnvironment: TurnEnvironment = {
      environmentId: "local",
      cwd: stickyCwd,
    };
    const turnOverrideEnvironment: TurnEnvironment = {
      environmentId: "local",
      cwd: turnOverrideCwd,
    };

    appServer = AppServerClient.start(codexHome, options);
    await appServer.initialize();

    const stickyThread = await startThread(appServer, {
      cwd: threadDefaultCwd,
      environments: [stickyEnvironment],
    });

    await runTurn(appServer, stickyThread.id, "sticky thread default turn");
    assertEnvironmentToolsPresent(latestRequest(mock), "thread sticky environment");
    assertRequestMentionsCwd(latestRequest(mock), stickyCwd, "thread sticky environment");
    console.log("ok thread/start non-empty environments are sticky");

    const noEnvironmentThread = await startThread(appServer, {
      cwd: threadDefaultCwd,
      environments: [],
    });
    await runTurn(appServer, noEnvironmentThread.id, "thread empty environment turn");
    assertEnvironmentToolsAbsent(latestRequest(mock), "thread/start empty environments");
    console.log("ok thread/start empty environments disable environment-backed tools");

    await runTurn(appServer, stickyThread.id, "turn override environment", {
      environments: [turnOverrideEnvironment],
    });
    assertEnvironmentToolsPresent(latestRequest(mock), "turn/start non-empty environments");
    assertRequestMentionsCwd(
      latestRequest(mock),
      turnOverrideCwd,
      "turn/start non-empty environments",
    );
    console.log("ok turn/start non-empty environments override the sticky cwd for one turn");

    await runTurn(appServer, stickyThread.id, "sticky restored after turn override");
    assertEnvironmentToolsPresent(latestRequest(mock), "sticky restored after turn override");
    assertRequestMentionsCwd(latestRequest(mock), stickyCwd, "sticky restored after turn override");
    console.log("ok turn/start non-empty override does not replace sticky environments");

    await runTurn(appServer, stickyThread.id, "turn empty environment", {
      environments: [],
    });
    assertEnvironmentToolsAbsent(latestRequest(mock), "turn/start empty environments");
    console.log("ok turn/start empty environments disable environment-backed tools for one turn");

    await runTurn(appServer, stickyThread.id, "sticky restored after empty turn environment");
    assertEnvironmentToolsPresent(latestRequest(mock), "sticky restored after empty turn environment");
    assertRequestMentionsCwd(
      latestRequest(mock),
      stickyCwd,
      "sticky restored after empty turn environment",
    );
    console.log("ok turn/start empty override does not replace sticky environments");

    console.log("sticky environment app-server e2e verification passed");
  } catch (error) {
    if (appServer) {
      const stderr = appServer.stderrTail();
      if (stderr.length > 0) {
        console.error("\napp-server stderr tail:\n" + stderr);
      }
    }
    throw error;
  } finally {
    if (appServer) {
      await appServer.stop();
    }
    await mock.close();
    if (options.keepTemp) {
      console.log(`kept temp directory: ${tmp}`);
    } else {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  }
}

main().catch((error: unknown) => {
  console.error(error instanceof Error ? error.stack : error);
  process.exitCode = 1;
});
