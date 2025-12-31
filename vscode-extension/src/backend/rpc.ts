import * as readline from "node:readline";
import type { ChildProcessWithoutNullStreams } from "node:child_process";
import { EventEmitter } from "node:events";
import * as vscode from "vscode";

import type { ClientNotification } from "../generated/ClientNotification";
import type { ClientRequest } from "../generated/ClientRequest";
import type { RequestId } from "../generated/RequestId";
import type { ServerRequest } from "../generated/ServerRequest";
import type { AnyServerNotification } from "./types";

type JsonRpcResponse = { id: RequestId; result?: unknown; error?: unknown };

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export class RpcClient extends EventEmitter implements vscode.Disposable {
  private nextId = 0;
  private readonly pending = new Map<
    number,
    { resolve: (v: unknown) => void; reject: (e: unknown) => void }
  >();
  private readonly stdoutRl: readline.Interface;
  private readonly stderrRl: readline.Interface;

  public constructor(
    private readonly child: ChildProcessWithoutNullStreams,
    private readonly output: vscode.OutputChannel,
    private readonly logRpcPayloads: boolean,
  ) {
    super();

    this.stdoutRl = readline.createInterface({ input: this.child.stdout });
    this.stderrRl = readline.createInterface({ input: this.child.stderr });

    this.stdoutRl.on("line", (line) => this.onStdoutLine(line));
    this.stderrRl.on("line", (line) =>
      this.output.appendLine(`[backend stderr] ${line}`),
    );

    this.child.on("exit", (code, signal) => {
      this.output.appendLine(
        `Backend exited: code=${code ?? "null"} signal=${signal ?? "null"}`,
      );
      for (const { reject } of this.pending.values())
        reject(new Error("Backend exited"));
      this.pending.clear();
      this.emit("exit", {
        code: code ?? null,
        signal: (signal ?? null) as NodeJS.Signals | null,
      });
    });
    this.child.on("error", (err) => {
      this.output.appendLine(`Backend process error: ${String(err)}`);
      this.emit("error", err);
    });
  }

  public dispose(): void {
    this.stdoutRl.close();
    this.stderrRl.close();
    for (const { reject } of this.pending.values())
      reject(new Error("RpcClient disposed"));
    this.pending.clear();
    this.removeAllListeners();
  }

  public request<TResponse>(
    request: Omit<ClientRequest, "id">,
  ): Promise<TResponse> {
    const id = this.nextId++;
    const payload = { ...request, id } as ClientRequest;
    this.writeJson(payload);
    return new Promise<TResponse>((resolve, reject) => {
      this.pending.set(id, {
        resolve: resolve as (v: unknown) => void,
        reject,
      });
    });
  }

  public notify(notification: ClientNotification): void {
    this.writeJson(notification);
  }

  public respond<T>(id: RequestId, result: T): void {
    this.writeJson({ id, result });
  }

  private writeJson(obj: unknown): void {
    const line = JSON.stringify(obj);
    if (this.logRpcPayloads) this.output.appendLine(`[rpc ->] ${line}`);
    this.child.stdin.write(`${line}\n`);
  }

  private onStdoutLine(line: string): void {
    if (!line.trim()) return;
    if (this.logRpcPayloads) this.output.appendLine(`[rpc <-] ${line}`);

    let msg: unknown;
    try {
      msg = JSON.parse(line) as unknown;
    } catch (err) {
      this.output.appendLine(
        `Failed to parse backend JSONL message: ${String(err)}; line=${line}`,
      );
      return;
    }

    if (!isObject(msg)) {
      this.output.appendLine(`Unexpected JSON-RPC message: ${line}`);
      return;
    }

    const id = msg["id"];
    const method = msg["method"];

    if (id !== undefined && method === undefined) {
      const response = msg as JsonRpcResponse;
      if (typeof response.id !== "number") {
        this.output.appendLine(`Unexpected response id type: ${line}`);
        return;
      }
      const pending = this.pending.get(response.id);
      if (!pending) {
        this.output.appendLine(`No pending request for id=${response.id}`);
        return;
      }
      this.pending.delete(response.id);
      if (response.error !== undefined) pending.reject(response.error);
      else pending.resolve(response.result);
      return;
    }

    if (typeof method === "string") {
      if (id !== undefined) this.emit("serverRequest", msg as ServerRequest);
      else this.emit("serverNotification", msg as AnyServerNotification);
      return;
    }

    this.output.appendLine(`Unrecognized JSON-RPC message shape: ${line}`);
  }
}
