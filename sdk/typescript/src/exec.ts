import { spawn } from "child_process";
import readline from "node:readline";

export type CodexExecArgs = {
  input: string;

  baseUrl?: string;
  apiKey?: string;
  sessionId?: string | null;
};

export class CodexExec {
  private executablePath: string;
  constructor(executablePath: string) {
    this.executablePath = executablePath;
  }

  async *run(args: CodexExecArgs): AsyncGenerator<string> {
    const commandArgs: string[] = ["exec", "--experimental-json"];
    if (args.sessionId) {
      commandArgs.push("resume", args.sessionId, args.input);
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

      // Wait for actual exit after streams close
      const exitCode: number | null = await new Promise((resolve) => {
        child.once("exit", (code) => resolve(code));
      });

      if (spawnError) throw spawnError;
      if ((exitCode ?? 0) !== 0) {
        throw new Error(`Codex Exec exited with code ${exitCode}`);
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
