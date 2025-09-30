import { spawn } from "child_process";
import readline from "node:readline";

import { SandboxMode } from "./turnOptions";

export type CodexExecArgs = {
  input: string;

  baseUrl?: string;
  apiKey?: string;
  threadId?: string | null;
  model?: string;
  sandboxMode?: SandboxMode;
};

export class CodexExec {
  private executablePath: string;
  constructor(executablePath: string) {
    this.executablePath = executablePath;
  }

  async *run(args: CodexExecArgs): AsyncGenerator<string> {
    const commandArgs: string[] = ["exec", "--experimental-json"];

    if (args.model) {
      commandArgs.push("--model", args.model);
    }

    if (args.sandboxMode) {
      commandArgs.push("--sandbox", args.sandboxMode);
    }

    if (args.threadId) {
      commandArgs.push("resume", args.threadId, args.input);
    } else {
      commandArgs.push(args.input);
    }

    const env = {
      ...process.env,
    };
    if (args.baseUrl) {
      env.OPENAI_BASE_URL = args.baseUrl;
    }
    if (args.apiKey) {
      env.OPENAI_API_KEY = args.apiKey;
    }

    const child = spawn(this.executablePath, commandArgs, {
      env,
    });

    let spawnError: unknown | null = null;
    child.once("error", (err) => (spawnError = err));

    if (!child.stdout) {
      child.kill();
      throw new Error("Child process has no stdout");
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

      const exitCode = new Promise((resolve) => {
        child.once("exit", (code) => { 
          if (code === 0) {
            resolve(code);
          } else {
            throw new Error(`Codex Exec exited with code ${code}`);
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
