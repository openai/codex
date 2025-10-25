import { spawn } from "node:child_process";

import { findCodexPath } from "./binaryPath";

export type CliRunResult = {
  stdout: string;
  stderr: string;
};

export class CodexCliError extends Error {
  exitCode: number | null;
  stdout: string;
  stderr: string;

  constructor(message: string, exitCode: number | null, stdout: string, stderr: string) {
    super(message);
    this.exitCode = exitCode;
    this.stdout = stdout;
    this.stderr = stderr;
  }
}

export class CodexCliRunner {
  private readonly executablePath: string;

  constructor(executablePath: string | null = null) {
    this.executablePath = findCodexPath(executablePath);
  }

  async run(
    args: string[],
    options: {
      configArgs?: string[];
      env?: NodeJS.ProcessEnv;
    } = {},
  ): Promise<CliRunResult> {
    const stdoutChunks: Buffer[] = [];
    const stderrChunks: Buffer[] = [];

    const child = spawn(this.executablePath, [...(options.configArgs ?? []), ...args], {
      env: options.env ? { ...process.env, ...options.env } : process.env,
      stdio: ["ignore", "pipe", "pipe"],
    });

    let spawnError: unknown | null = null;
    child.once("error", (error) => {
      spawnError = error;
    });

    if (child.stdout) {
      child.stdout.on("data", (chunk) => {
        stdoutChunks.push(chunk);
      });
    }

    if (child.stderr) {
      child.stderr.on("data", (chunk) => {
        stderrChunks.push(chunk);
      });
    }

    await new Promise<void>((resolve, reject) => {
      child.once("close", (code) => {
        if (spawnError) {
          reject(spawnError);
          return;
        }
        if (code !== 0) {
          const stdout = Buffer.concat(stdoutChunks).toString("utf8");
          const stderr = Buffer.concat(stderrChunks).toString("utf8");
          reject(new CodexCliError(`codex exited with code ${code}`, code, stdout, stderr));
          return;
        }
        resolve();
      });

      child.once("error", reject);
    });

    return {
      stdout: Buffer.concat(stdoutChunks).toString("utf8"),
      stderr: Buffer.concat(stderrChunks).toString("utf8"),
    };
  }
}
