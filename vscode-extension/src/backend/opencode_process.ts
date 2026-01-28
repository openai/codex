import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import * as readline from "node:readline";
import * as vscode from "vscode";

export type OpencodeServerOptions = {
  command: string;
  args: string[];
  cwd: string;
  output: vscode.OutputChannel;
};

export class OpencodeServerProcess implements vscode.Disposable {
  private readonly child: ChildProcessWithoutNullStreams;
  private resolvedBaseUrl: URL | null = null;
  private readonly lineReaders: readline.Interface[];

  private constructor(
    child: ChildProcessWithoutNullStreams,
    private readonly output: vscode.OutputChannel,
  ) {
    this.child = child;
    const stdout = readline.createInterface({ input: child.stdout });
    const stderr = readline.createInterface({ input: child.stderr });
    this.lineReaders = [stdout, stderr];

    for (const rl of this.lineReaders) {
      rl.on("line", (line) => this.onLine(line));
    }
  }

  public static async spawn(
    opts: OpencodeServerOptions,
  ): Promise<OpencodeServerProcess> {
    const child = spawn(opts.command, opts.args, {
      cwd: opts.cwd,
      stdio: ["pipe", "pipe", "pipe"],
      env: process.env,
    });
    child.stdin.end();
    const proc = new OpencodeServerProcess(child, opts.output);
    await proc.waitForBaseUrl(10_000);
    return proc;
  }

  public getBaseUrl(): URL {
    if (!this.resolvedBaseUrl) {
      throw new Error("opencode server URL is not known yet");
    }
    return this.resolvedBaseUrl;
  }

  public onDidExit(
    handler: (info: {
      code: number | null;
      signal: NodeJS.Signals | null;
    }) => void,
  ): void {
    this.child.on("exit", (code, signal) => {
      handler({ code, signal });
    });
  }

  public dispose(): void {
    for (const rl of this.lineReaders) rl.close();
    this.child.removeAllListeners();
    try {
      this.child.kill();
    } catch {
      // ignore
    }
  }

  private onLine(line: string): void {
    const trimmed = String(line ?? "").trimEnd();
    if (trimmed) this.output.appendLine(`[opencode] ${trimmed}`);

    // opencode prints:
    // "opencode server listening on http://<hostname>:<port>"
    if (this.resolvedBaseUrl) return;
    const m = trimmed.match(/opencode server listening on (https?:\/\/\S+)/i);
    if (!m) return;
    const rawUrl = m[1];
    if (!rawUrl) return;
    try {
      this.resolvedBaseUrl = new URL(rawUrl);
    } catch {
      // ignore malformed URL; keep waiting
    }
  }

  private async waitForBaseUrl(timeoutMs: number): Promise<void> {
    if (this.resolvedBaseUrl) return;
    const startedAt = Date.now();
    while (!this.resolvedBaseUrl) {
      if (Date.now() - startedAt > timeoutMs) {
        throw new Error("Timed out waiting for opencode server to start");
      }
      await new Promise((r) => setTimeout(r, 50));
    }
  }
}
