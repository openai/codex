import { spawn } from "node:child_process";
import path from "node:path";
import readline from "node:readline";
import { fileURLToPath } from "node:url";

import {
  SandboxMode,
  ModelReasoningEffort,
  ApprovalMode,
  WebSearchMode,
} from "./threadOptions";

export type CodexExecArgs = {
  input: string;

  baseUrl?: string;
  apiKey?: string;
  threadId?: string | null;
  images?: string[];
  // --model
  model?: string;
  // --sandbox
  sandboxMode?: SandboxMode;
  // --cd
  workingDirectory?: string;
  // --add-dir
  additionalDirectories?: string[];
  // --skip-git-repo-check
  skipGitRepoCheck?: boolean;
  // --output-schema
  outputSchemaFile?: string;
  // --config model_reasoning_effort
  modelReasoningEffort?: ModelReasoningEffort;
  // AbortSignal to cancel the execution
  signal?: AbortSignal;
  // --config sandbox_workspace_write.network_access
  networkAccessEnabled?: boolean;
  // --config web_search
  webSearchMode?: WebSearchMode;
  // legacy --config features.web_search_request
  webSearchEnabled?: boolean;
  // --config approval_policy
  approvalPolicy?: ApprovalMode;
};

const INTERNAL_ORIGINATOR_ENV = "CODEX_INTERNAL_ORIGINATOR_OVERRIDE";
const TYPESCRIPT_SDK_ORIGINATOR = "codex_sdk_ts";

export class CodexExec {
  private executablePath: string;
  private envOverride?: Record<string, string>;

  constructor(executablePath: string | null = null, env?: Record<string, string>) {
    this.executablePath = executablePath || findCodexPath();
    this.envOverride = env;
  }

  async *run(args: CodexExecArgs): AsyncGenerator<string> {
    const commandArgs: string[] = ["exec", "--experimental-json"];

    if (args.model) {
      commandArgs.push("--model", args.model);
    }

    if (args.sandboxMode) {
      commandArgs.push("--sandbox", args.sandboxMode);
    }

    if (args.workingDirectory) {
      commandArgs.push("--cd", args.workingDirectory);
    }

    if (args.additionalDirectories?.length) {
      for (const dir of args.additionalDirectories) {
        commandArgs.push("--add-dir", dir);
      }
    }

    if (args.skipGitRepoCheck) {
      commandArgs.push("--skip-git-repo-check");
    }

    if (args.outputSchemaFile) {
      commandArgs.push("--output-schema", args.outputSchemaFile);
    }

    if (args.modelReasoningEffort) {
      commandArgs.push("--config", `model_reasoning_effort="${args.modelReasoningEffort}"`);
    }

    if (args.networkAccessEnabled !== undefined) {
      commandArgs.push(
        "--config",
        `sandbox_workspace_write.network_access=${args.networkAccessEnabled}`,
      );
    }

    if (args.webSearchMode) {
      commandArgs.push("--config", `web_search="${args.webSearchMode}"`);
    } else if (args.webSearchEnabled === true) {
      commandArgs.push("--config", `web_search="live"`);
    } else if (args.webSearchEnabled === false) {
      commandArgs.push("--config", `web_search="disabled"`);
    }

    if (args.approvalPolicy) {
      commandArgs.push("--config", `approval_policy="${args.approvalPolicy}"`);
    }

    if (args.images?.length) {
      for (const image of args.images) {
        commandArgs.push("--image", image);
      }
    }

    if (args.threadId) {
      commandArgs.push("resume", args.threadId);
    }

    const env: Record<string, string> = {};
    if (this.envOverride) {
      Object.assign(env, this.envOverride);
    } else {
      for (const [key, value] of Object.entries(process.env)) {
        if (value !== undefined) {
          env[key] = value;
        }
      }
    }
    if (!env[INTERNAL_ORIGINATOR_ENV]) {
      env[INTERNAL_ORIGINATOR_ENV] = TYPESCRIPT_SDK_ORIGINATOR;
    }
    if (args.baseUrl) {
      env.OPENAI_BASE_URL = args.baseUrl;
    }
    if (args.apiKey) {
      env.CODEX_API_KEY = args.apiKey;
    }

    const child = spawn(this.executablePath, commandArgs, {
      env,
      signal: args.signal,
    });

    let spawnError: unknown | null = null;
    child.once("error", (err) => (spawnError = err));

    if (!child.stdin) {
      child.kill();
      throw new Error("Child process has no stdin");
    }
    child.stdin.write(args.input);
    child.stdin.end();

    if (!child.stdout) {
      child.kill();
      throw new Error("Child process has no stdout");
    }
    const stderrChunks: Buffer[] = [];

    if (child.stderr) {
      child.stderr.on("data", (data) => {
        stderrChunks.push(data);
      });
    }

    const exitPromise = new Promise<{ code: number | null; signal: NodeJS.Signals | null }>(
      (resolve) => {
        child.once("exit", (code, signal) => {
          resolve({ code, signal });
        });
      },
    );

    const rl = readline.createInterface({
      input: child.stdout,
      crlfDelay: Infinity,
    });

    try {
      for await (const line of rl) {
        // `line` is a string (Node sets default encoding to utf8 for readline)
        yield line as string;
      }

      if (spawnError) throw spawnError;
      const { code, signal } = await exitPromise;
      if (code !== 0 || signal) {
        const stderrBuffer = Buffer.concat(stderrChunks);
        const detail = signal ? `signal ${signal}` : `code ${code ?? 1}`;
        throw new Error(`Codex Exec exited with ${detail}: ${stderrBuffer.toString("utf8")}`);
      }
    } finally {
      rl.close();
      child.removeAllListeners();
      try {
        if (!child.killed) child.kill();
      } catch {
        // ignore
      }
    }
  }
}

const scriptFileName = fileURLToPath(import.meta.url);
const scriptDirName = path.dirname(scriptFileName);

function findCodexPath() {
  const { platform, arch } = process;

  let targetTriple = null;
  switch (platform) {
    case "linux":
    case "android":
      switch (arch) {
        case "x64":
          targetTriple = "x86_64-unknown-linux-musl";
          break;
        case "arm64":
          targetTriple = "aarch64-unknown-linux-musl";
          break;
        default:
          break;
      }
      break;
    case "darwin":
      switch (arch) {
        case "x64":
          targetTriple = "x86_64-apple-darwin";
          break;
        case "arm64":
          targetTriple = "aarch64-apple-darwin";
          break;
        default:
          break;
      }
      break;
    case "win32":
      switch (arch) {
        case "x64":
          targetTriple = "x86_64-pc-windows-msvc";
          break;
        case "arm64":
          targetTriple = "aarch64-pc-windows-msvc";
          break;
        default:
          break;
      }
      break;
    default:
      break;
  }

  if (!targetTriple) {
    throw new Error(`Unsupported platform: ${platform} (${arch})`);
  }

  const vendorRoot = path.join(scriptDirName, "..", "vendor");
  const archRoot = path.join(vendorRoot, targetTriple);
  const codexBinaryName = process.platform === "win32" ? "codex.exe" : "codex";
  const binaryPath = path.join(archRoot, "codex", codexBinaryName);

  return binaryPath;
}
