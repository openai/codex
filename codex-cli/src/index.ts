import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawn, type SpawnOptions, type ChildProcess } from "node:child_process";
import { Readable } from "node:stream";
import readline from "node:readline";

// __dirname equivalent in ESM context
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

export type CodexSpawnOptions = Omit<SpawnOptions, "env"> & {
  env?: NodeJS.ProcessEnv;
  /**
   * Additional directories to prepend to PATH when launching codex.
   * By default, the appropriate vendor path is added automatically.
   */
  extraPathDirs?: string[];
  /** Override the path to the codex binary (for testing/custom installs). */
  binaryPath?: string;
  /** Optional override for the model provider base URL (sets OPENAI_BASE_URL). */
  baseUrl?: string;

};

export type CodexResult = {
  type: "code" | "signal";
  exitCode?: number;
  signal?: NodeJS.Signals;
};

// ------------------------
// Exec JSON event types
// ------------------------

export type CommandExecutionStatus = "in_progress" | "completed" | "failed";
export type PatchApplyStatus = "completed" | "failed";
export type PatchChangeKind = "add" | "delete" | "update";

export type AssistantMessageItem = {
  item_type: "assistant_message";
  id: string;
  text: string;
};

export type ReasoningItem = {
  item_type: "reasoning";
  id: string;
  text: string;
};

export type CommandExecutionItem = {
  item_type: "command_execution";
  id: string;
  command: string;
  aggregated_output: string;
  exit_code?: number;
  status: CommandExecutionStatus;
};

export type FileUpdateChange = {
  path: string;
  kind: PatchChangeKind;
};

export type FileChangeItem = {
  item_type: "file_change";
  id: string;
  changes: FileUpdateChange[];
  status: PatchApplyStatus;
};

export type McpToolCallItem = {
  item_type: "mcp_tool_call";
  id: string;
  server: string;
  tool: string;
  status: CommandExecutionStatus;
};

export type WebSearchItem = {
  item_type: "web_search";
  id: string;
  query: string;
};

export type ErrorItem = {
  item_type: "error";
  id: string;
  message: string;
};

export type ConversationItem =
  | AssistantMessageItem
  | ReasoningItem
  | CommandExecutionItem
  | FileChangeItem
  | McpToolCallItem
  | WebSearchItem
  | ErrorItem;

export type ConversationEvent =
  | { type: "session.created"; session_id: string } & Record<string, never>
  | { type: "item.started"; item: ConversationItem }
  | { type: "item.completed"; item: ConversationItem }
  | { type: "error"; message: string };

/** Resolve the target triple for the current platform/arch. */
function resolveTargetTriple(
  platform: NodeJS.Platform = process.platform,
  arch: string = process.arch,
): string | null {
  switch (platform) {
    case "linux":
    case "android":
      switch (arch) {
        case "x64":
          return "x86_64-unknown-linux-musl";
        case "arm64":
          return "aarch64-unknown-linux-musl";
        default:
          return null;
      }
    case "darwin":
      switch (arch) {
        case "x64":
          return "x86_64-apple-darwin";
        case "arm64":
          return "aarch64-apple-darwin";
        default:
          return null;
      }
    case "win32":
      switch (arch) {
        case "x64":
          return "x86_64-pc-windows-msvc";
        case "arm64":
          return "aarch64-pc-windows-msvc";
        default:
          return null;
      }
    default:
      return null;
  }
}

/**
 * Get the absolute path to the packaged native codex binary for the current platform.
 */
function getCodexBinaryPath(): string {
  const targetTriple = resolveTargetTriple();
  if (!targetTriple) {
    throw new Error(`Unsupported platform: ${process.platform} (${process.arch})`);
  }

  const vendorRoot = path.join(__dirname, "..", "vendor");
  const archRoot = path.join(vendorRoot, targetTriple);
  const codexBinaryName = process.platform === "win32" ? "codex.exe" : "codex";
  const binaryPath = path.join(archRoot, "codex", codexBinaryName);
  return binaryPath;
}

/** Build an updated PATH including any vendor-provided path helpers. */
function buildCodexPath(extraDirs: string[] = []): string {
  const targetTriple = resolveTargetTriple();
  if (!targetTriple) {
    throw new Error(`Unsupported platform: ${process.platform} (${process.arch})`);
  }

  const vendorRoot = path.join(__dirname, "..", "vendor");
  const archRoot = path.join(vendorRoot, targetTriple);
  const toPrepend = [...extraDirs];

  const pathDir = path.join(archRoot, "path");
  if (existsSync(pathDir)) {
    toPrepend.push(pathDir);
  }

  const sep = process.platform === "win32" ? ";" : ":";
  const existing = process.env.PATH || "";
  return [...toPrepend, ...existing.split(sep).filter(Boolean)].join(sep);
}

/**
 * Spawn the packaged codex binary.
 *
 * The default behavior mirrors the CLI wrapper: `stdio: \"inherit\"`, and PATH
 * is augmented with any vendor-provided shims.
 */
/**
 * Execute the codex binary with provided arguments/options and await completion.
 * Defaults to the packaged binary unless `binaryPath` is provided in options.
 */
function resolveCodexCommand(binaryPath?: string): string {
  if (binaryPath) return binaryPath;
  const candidate = getCodexBinaryPath();
  if (existsSync(candidate)) return candidate;
  return process.platform === "win32" ? "codex.exe" : "codex";
}

export async function execCodex(
  args: string[] = [],
  options: CodexSpawnOptions = {},
): Promise<CodexResult> {
  const binaryPath = resolveCodexCommand(options.binaryPath);
  const { extraPathDirs = [], env, stdio = "inherit", ...rest } = options;

  const childEnv: NodeJS.ProcessEnv = {
    ...process.env,
    ...env,
    PATH: buildCodexPath(extraPathDirs),
    CODEX_MANAGED_BY_NPM: "1",
  };

  const child = spawn(binaryPath, args, { stdio, env: childEnv, ...rest });
  return await new Promise<CodexResult>((resolve, reject) => {
    child.on("error", (err) => reject(err));
    child.on("exit", (code, signal) => {
      if (signal) {
        resolve({ type: "signal", signal });
      } else {
        resolve({ type: "code", exitCode: code ?? 1 });
      }
    });
  });
}

/** Parse newline-delimited JSON ConversationEvent objects from a readable stream. */
export async function* parseExecEvents(stream: Readable): AsyncIterable<ConversationEvent> {
  const rl = readline.createInterface({ input: stream, crlfDelay: Infinity });
  for await (const line of rl) {
    const trimmed = String(line).trim();
    if (!trimmed) continue;
    try {
      const obj = JSON.parse(trimmed) as ConversationEvent;
      // Basic shape check: must have a type
      if (obj && typeof obj === "object" && "type" in obj) {
        yield obj;
      }
    } catch {
      // Ignore malformed lines to keep the stream resilient.
    }
  }
}

export type RunExecReturn = {
  child: ChildProcess;
  events: AsyncIterable<ConversationEvent>;
  done: Promise<CodexResult>;
};

/**
 * Run `codex exec` with JSON output enabled and return a live stream of events.
 * Always injects `exec` and `--json-experimental` ahead of provided args.
 */
export function runExec(execArgs: string[] = [], options: CodexSpawnOptions = {}): RunExecReturn {
  const binaryPath = resolveCodexCommand(options.binaryPath);
  const { extraPathDirs = [], env, baseUrl } = options;

  const childEnv: NodeJS.ProcessEnv = {
    ...process.env,
    ...env,
    PATH: buildCodexPath(extraPathDirs),
    ...(baseUrl ? { OPENAI_BASE_URL: baseUrl } : {}),
  };

  const args = ["exec", "--experimental-json", ...execArgs]

  // Force stdout to be piped so we can parse events; let stderr inherit by default.
  const child = spawn(binaryPath, args, {
    stdio: ["inherit", "pipe", "inherit"],
    env: childEnv
  });

  const done = new Promise<CodexResult>((resolve, reject) => {
    child.on("error", (err) => reject(err));
    child.on("exit", (code, signal) => {
      if (signal) {
        resolve({ type: "signal", signal });
      } else {
        resolve({ type: "code", exitCode: code ?? 1 });
      }
    });
  });

  const events = child.stdout ? parseExecEvents(child.stdout) : (async function* () {})();

  return { child, events, done };
}
