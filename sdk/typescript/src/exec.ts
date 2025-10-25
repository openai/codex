import { spawn } from "node:child_process";
import readline from "node:readline";

import { SandboxMode } from "./threadOptions";
import { ConfigOverrideStore } from "./configOverrides";
import { findCodexPath } from "./binaryPath";

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
  // --skip-git-repo-check
  skipGitRepoCheck?: boolean;
  // --output-schema
  outputSchemaFile?: string;
};

const INTERNAL_ORIGINATOR_ENV = "CODEX_INTERNAL_ORIGINATOR_OVERRIDE";
const TYPESCRIPT_SDK_ORIGINATOR = "codex_sdk_ts";

export class CodexExec {
  private executablePath: string;
  private readonly configOverrides: ConfigOverrideStore;

  constructor(
    executablePath: string | null = null,
    configOverrides: ConfigOverrideStore | null = null,
  ) {
    this.executablePath = findCodexPath(executablePath);
    this.configOverrides = configOverrides ?? new ConfigOverrideStore();
  }

  get overrideStore(): ConfigOverrideStore {
    return this.configOverrides;
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

    if (args.skipGitRepoCheck) {
      commandArgs.push("--skip-git-repo-check");
    }

    if (args.outputSchemaFile) {
      commandArgs.push("--output-schema", args.outputSchemaFile);
    }

    if (args.images?.length) {
      for (const image of args.images) {
        commandArgs.push("--image", image);
      }
    }

    if (args.threadId) {
      commandArgs.push("resume", args.threadId);
    }

    const env = {
      ...process.env,
    };
    if (!env[INTERNAL_ORIGINATOR_ENV]) {
      env[INTERNAL_ORIGINATOR_ENV] = TYPESCRIPT_SDK_ORIGINATOR;
    }
    if (args.baseUrl) {
      env.OPENAI_BASE_URL = args.baseUrl;
    }
    if (args.apiKey) {
      env.CODEX_API_KEY = args.apiKey;
    }

    const finalArgs = [...this.configOverrides.toCliArgs(), ...commandArgs];

    const child = spawn(this.executablePath, finalArgs, {
      env,
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

    const rl = readline.createInterface({
      input: child.stdout,
      crlfDelay: Infinity,
    });

    try {
      for await (const line of rl) {
        // `line` is a string (Node sets default encoding to utf8 for readline)
        yield line as string;
      }

      const exitCode = new Promise((resolve, reject) => {
        child.once("exit", (code) => {
          if (code === 0) {
            resolve(code);
          } else {
            const stderrBuffer = Buffer.concat(stderrChunks);
            reject(
              new Error(`Codex Exec exited with code ${code}: ${stderrBuffer.toString("utf8")}`),
            );
          }
        });
      });

      if (spawnError) throw spawnError;
      await exitCode;
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
