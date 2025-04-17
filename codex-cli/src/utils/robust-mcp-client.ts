import type { ChildProcessWithoutNullStreams } from "child_process";

import { log } from "./agent/log";
import { spawn } from "child_process";

/**
 * Event types for the RobustStdioMcpClient
 */
export type McpClientEvent =
  | { type: "response"; data: any }
  | { type: "log"; message: string }
  | { type: "error"; message: string }
  | { type: "ready" }
  | { type: "exit"; code: number | null };

/**
 * RobustStdioMcpClient: A robust client for MCP servers that use stdio.
 *
 * This client:
 * - Buffers stdout and processes it line by line
 * - Parses each line as JSON and handles invalid JSON gracefully
 * - Logs stderr output
 * - Detects when the server is ready based on stderr messages
 * - Provides a simple API for sending requests and receiving responses
 */
export class RobustStdioMcpClient {
  private proc: ChildProcessWithoutNullStreams;
  private stdoutBuffer = "";
  private ready = false;
  private pendingRequests: Array<{
    resolve: (value: any) => void;
    reject: (reason: any) => void;
    id?: string | number;
  }> = [];
  private eventListeners: Array<(event: McpClientEvent) => void> = [];
  private serverName: string;

  /**
   * Create a new RobustStdioMcpClient
   *
   * @param cmd The command to run
   * @param args The arguments to pass to the command
   * @param options Additional options
   */
  constructor(
    cmd: string,
    args: Array<string> = [],
    private options: {
      serverName: string;
      env?: Record<string, string>;
      readyPattern?: RegExp;
    },
  ) {
    this.serverName = options.serverName || "unknown";

    log(`[MCP ${this.serverName}] Spawning process: ${cmd} ${args.join(" ")}`);

    this.proc = spawn(cmd, args, {
      stdio: ["pipe", "pipe", "pipe"],
      env: { ...process.env, ...(options.env || {}) },
    });

    this.proc.stdout.on("data", (data) => this.handleStdout(data));
    this.proc.stderr.on("data", (data) => this.handleStderr(data));

    this.proc.on("exit", (code) => {
      log(`[MCP ${this.serverName}] Process exited with code ${code}`);
      this.emitEvent({ type: "exit", code });

      // Reject any pending requests
      for (const request of this.pendingRequests) {
        request.reject(new Error(`MCP server exited with code ${code}`));
      }
      this.pendingRequests = [];
    });
  }

  /**
   * Add an event listener
   */
  addEventListener(listener: (event: McpClientEvent) => void): () => void {
    this.eventListeners.push(listener);
    return () => {
      this.eventListeners = this.eventListeners.filter((l) => l !== listener);
    };
  }

  /**
   * Emit an event to all listeners
   */
  private emitEvent(event: McpClientEvent): void {
    for (const listener of this.eventListeners) {
      try {
        listener(event);
      } catch (err) {
        console.error("Error in MCP event listener:", err);
      }
    }
  }

  /**
   * Handle stdout data
   */
  private handleStdout(data: Buffer): void {
    // Log raw data for debugging
    const rawData = data.toString();
    console.log(
      `[MCP ${this.serverName}] Raw stdout: ${JSON.stringify(rawData)}`,
    );

    this.stdoutBuffer += rawData;
    const lines = this.stdoutBuffer.split("\n");
    this.stdoutBuffer = lines.pop() || ""; // Keep incomplete line for next time

    console.log(
      `[MCP ${this.serverName}] Processing ${
        lines.length
      } lines, buffer remaining: ${JSON.stringify(this.stdoutBuffer)}`,
    );

    for (const line of lines) {
      if (!line.trim()) {
        console.log(`[MCP ${this.serverName}] Skipping empty line`);
        continue;
      }

      console.log(
        `[MCP ${this.serverName}] Processing line: ${JSON.stringify(line)}`,
      );

      try {
        const parsed = JSON.parse(line);
        console.log(
          `[MCP ${this.serverName}] Successfully parsed JSON: ${JSON.stringify(
            parsed,
          )}`,
        );
        log(`[MCP ${this.serverName}] Received: ${JSON.stringify(parsed)}`);
        this.emitEvent({ type: "response", data: parsed });

        // If we have a pending request, resolve it
        if (this.pendingRequests.length > 0) {
          console.log(
            `[MCP ${this.serverName}] Resolving pending request (${this.pendingRequests.length} in queue)`,
          );
          const request = this.pendingRequests.shift();
          if (request) {
            request.resolve(parsed);
          }
        } else {
          console.log(
            `[MCP ${this.serverName}] No pending requests to resolve`,
          );
        }
      } catch (err) {
        console.log(`[MCP ${this.serverName}] Failed to parse JSON: ${err}`);
        log(`[MCP ${this.serverName}] Invalid JSON from stdout: ${line}`);
        this.emitEvent({
          type: "error",
          message: `Invalid JSON from stdout: ${line}`,
        });

        // If this looks like a text response and we have pending requests, try to handle it anyway
        if (line.includes("message") && this.pendingRequests.length > 0) {
          console.log(
            `[MCP ${this.serverName}] Line contains 'message', treating as text response`,
          );
          const request = this.pendingRequests.shift();
          if (request) {
            request.resolve({ content: [{ type: "text", text: line }] });
          }
        }
      }
    }
  }

  /**
   * Handle stderr data
   */
  private handleStderr(data: Buffer): void {
    const msg = data.toString().trim();
    if (!msg) {
      return;
    }

    log(`[MCP ${this.serverName}] Log: ${msg}`);
    this.emitEvent({ type: "log", message: msg });

    // Check if the server is ready
    const readyPattern =
      this.options.readyPattern || /server running|ready|started/i;
    if (!this.ready && readyPattern.test(msg)) {
      log(`[MCP ${this.serverName}] Server is ready`);
      this.ready = true;
      this.emitEvent({ type: "ready" });
    }
  }

  /**
   * Send a request to the server
   * @param payload The payload to send
   * @param timeoutMs Timeout in milliseconds (default: 10000)
   */
  async send(payload: any, timeoutMs = 10000): Promise<any> {
    return new Promise((resolve, reject) => {
      // Set a timeout to prevent hanging
      const timeoutId = setTimeout(() => {
        // Remove this request from pending requests
        this.pendingRequests = this.pendingRequests.filter(
          (req) => req.resolve !== resolve && req.reject !== reject,
        );

        reject(
          new Error(
            `Timeout waiting for response from MCP server (${timeoutMs}ms)`,
          ),
        );
      }, timeoutMs);

      // Add to pending requests with timeout cleanup
      this.pendingRequests.push({
        resolve: (value) => {
          clearTimeout(timeoutId);
          resolve(value);
        },
        reject: (reason) => {
          clearTimeout(timeoutId);
          reject(reason);
        },
        id: payload.id,
      });

      // Send the payload
      const payloadStr = JSON.stringify(payload) + "\n";
      log(`[MCP ${this.serverName}] Sending: ${payloadStr.trim()}`);
      this.proc.stdin.write(payloadStr);

      // Log to console for debugging
      console.log(`[DEBUG] Sent to ${this.serverName}: ${payloadStr.trim()}`);
    });
  }

  /**
   * Check if the server is ready
   */
  isReady(): boolean {
    return this.ready;
  }

  /**
   * Wait for the server to be ready
   */
  async waitForReady(timeoutMs = 5000): Promise<void> {
    if (this.ready) {
      return;
    }

    return new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => {
        cleanup();
        reject(
          new Error(
            `Timeout waiting for MCP server to be ready (${timeoutMs}ms)`,
          ),
        );
      }, timeoutMs);

      const onEvent = (event: McpClientEvent) => {
        if (event.type === "ready") {
          cleanup();
          resolve();
        } else if (event.type === "exit") {
          cleanup();
          reject(
            new Error(
              `MCP server exited before becoming ready (code: ${event.code})`,
            ),
          );
        }
      };

      const removeListener = this.addEventListener(onEvent);

      const cleanup = () => {
        clearTimeout(timeout);
        removeListener();
      };
    });
  }

  /**
   * Kill the server process
   */
  kill(): void {
    this.proc.kill();
  }
}
