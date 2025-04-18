import type { ChildProcess } from "child_process";

import { McpTimeoutError, McpConnectionError } from "./mcp-errors";
import { debug, error as logError } from "./mcp-logger";
import { spawn } from "child_process";
import { EventEmitter } from "events";

/**
 * Options for creating an MCP Stdio Client
 */
export interface McpStdioClientOptions {
  /** Name of themcpServer (for logging purposes) */
  mcpServerName: string;
  /** Environment variables to pass to the process */
  env?: Record<string, string>;
  /** Regex pattern to detectmcpServer readiness */
  readyPattern?: RegExp;
  /** Timeout for request operations (milliseconds) */
  requestTimeoutMs?: number;
  /** Debug mode flag */
  debug?: boolean;
}

/**
 * Event types emitted by the Stdio client
 */
export type McpStdioClientEvent =
  | { type: "log"; message: string }
  | { type: "error"; message: string }
  | { type: "ready" }
  | { type: "exit"; code: number | null };

/**
 * Request to send to the MCP server
 */
export interface McpRequest {
  method: string;
  params?: Record<string, any>;
  id?: string | number;
}

/**
 * Response from the MCP server
 */
export interface McpResponse {
  id?: string | number;
  result?: any;
  error?: {
    code?: number;
    message?: string;
    data?: any;
  };
}

/**
 * A client for communicating with MCP servers over stdio
 *
 * This class handles:
 * - Process lifecycle management
 * - Line buffering and JSON parsing
 * - Request/response correlation
 * - Timeouts
 * - Error recovery
 */
export class McpStdioClient extends EventEmitter {
  private process: ChildProcess | null = null;
  private buffer: string = "";
  private requests: Map<
    string | number,
    {
      resolve: (value: any) => void;
      reject: (reason: any) => void;
      timeout: NodeJS.Timeout;
    }
  > = new Map();
  private lastRequestId: number = 0;
  private isKilled: boolean = false;
  private options: Required<McpStdioClientOptions>;
  private readyState: "not_ready" | "ready" | "error" = "not_ready";
  private abortController: AbortController;

  /**
   * Create a new MCP stdio client
   * @param command Command to run
   * @param args Arguments for the command
   * @param options Client options
   */
  constructor(
    private readonly command: string,
    private readonly args: Array<string> = [],
    options: McpStdioClientOptions,
  ) {
    super();

    // Set default options
    this.options = {
      mcpServerName: options.mcpServerName,
      env: options.env || {},
      readyPattern: options.readyPattern || /server running|ready|started/i,
      requestTimeoutMs: options.requestTimeoutMs || 10000,
      debug: options.debug || false,
    };

    this.abortController = new AbortController();

    // Set max listeners to avoid Node.js warnings
    this.setMaxListeners(50);
  }

  /**
   * Add an event listener for a specific event type
   * @param listener The event listener function
   */
  addEventListener(listener: (event: McpStdioClientEvent) => void): void {
    this.on("event", listener);
  }

  /**
   * Remove an event listener
   * @param listener The event listener function to remove
   */
  removeEventListener(listener: (event: McpStdioClientEvent) => void): void {
    this.off("event", listener);
  }

  /**
   * Start the process and initialize the connection
   */
  async start(): Promise<void> {
    if (this.process) {
      debug(`Process for ${this.options.mcpServerName} already started`);
      return;
    }

    this.isKilled = false;

    try {
      // Start process with environment variables
      this.process = spawn(this.command, this.args, {
        env: { ...process.env, ...this.options.env },
        stdio: ["pipe", "pipe", "pipe"],
        signal: this.abortController.signal,
      });

      // Set up event handlers
      this.setupProcessHandlers();

      // Emit log event
      this.emitEvent({
        type: "log",
        message: `Started process: ${this.command} ${this.args.join(" ")}`,
      });

      // Wait for themcpServer to be ready
      await this.waitForReady();
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      this.emitEvent({
        type: "error",
        message: `Failed to start process: ${errorMessage}`,
      });
      throw new McpConnectionError(
        this.options.mcpServerName,
        err instanceof Error ? err : new Error(errorMessage),
      );
    }
  }

  /**
   * Set up event handlers for the child process
   */
  private setupProcessHandlers(): void {
    // Safety check - should never happen if called after spawn
    if (!this.process) {
      this.emitEvent({
        type: "error",
        message: "Cannot setup handlers for null process",
      });
      return;
    }

    const process = this.process;

    // Make sure stdout and stderr exist
    if (!process.stdout || !process.stderr) {
      this.emitEvent({
        type: "error",
        message: "Process streams not available",
      });
      return;
    }

    // Handle stdout data
    process.stdout.on("data", (data: Buffer) => {
      const text = data.toString();
      this.handleStdoutData(text);
    });

    // Handle stderr data
    process.stderr.on("data", (data: Buffer) => {
      const text = data.toString().trim();
      if (!text) {
        return;
      }

      // Check for ready patterns in stderr
      if (this.options.readyPattern && this.options.readyPattern.test(text)) {
        if (this.readyState === "not_ready") {
          this.readyState = "ready";
          this.emitEvent({ type: "ready" });
        }
      }

      // Log stderr output
      this.emitEvent({ type: "log", message: `[stderr] ${text}` });
    });

    // Handle process exit
    process.on("exit", (code: number | null) => {
      this.emitEvent({ type: "exit", code });

      if (code !== 0 && !this.isKilled) {
        this.emitEvent({
          type: "error",
          message: `Process exited unexpectedly with code ${code}`,
        });

        // Reject all pending requests
        this.rejectAllRequests(`Process exited unexpectedly with code ${code}`);
      }

      this.process = null;
    });

    // Handle process errors
    process.on("error", (err: Error) => {
      this.readyState = "error";
      this.emitEvent({
        type: "error",
        message: `Process error: ${err.message}`,
      });

      // Reject all pending requests
      this.rejectAllRequests(`Process error: ${err.message}`);
    });
  }

  /**
   * Emit an event to all listeners
   * @param event The event to emit
   */
  private emitEvent(event: McpStdioClientEvent): void {
    if (event.type === "log" && this.options.debug) {
      debug(`[${this.options.mcpServerName}] ${event.message}`);
    } else if (event.type === "error") {
      logError(`[${this.options.mcpServerName}] ${event.message}`);
    }

    this.emit("event", event);
  }

  /**
   * Handle data received on stdout
   * @param text The text data received
   */
  private handleStdoutData(text: string): void {
    // Add new data to the buffer
    this.buffer += text;

    // Process complete lines
    let lineEndIndex: number;
    while ((lineEndIndex = this.buffer.indexOf("\n")) !== -1) {
      const line = this.buffer.substring(0, lineEndIndex).trim();
      this.buffer = this.buffer.substring(lineEndIndex + 1);

      if (!line) {
        continue;
      }

      try {
        // Try to parse the line as JSON
        const response = JSON.parse(line);

        // Check for protocol response (for init)
        if (
          response.protocol ||
          (response.type === "init_response" && response.data?.protocol)
        ) {
          if (this.readyState === "not_ready") {
            this.readyState = "ready";
            this.emitEvent({ type: "ready" });
          }
        }

        // Process response if it has an ID
        if (response.id && this.requests.has(response.id)) {
          const { resolve, timeout } = this.requests.get(response.id)!;
          clearTimeout(timeout);
          this.requests.delete(response.id);

          if (response.error) {
            this.emitEvent({
              type: "error",
              message: `Error response for request ${
                response.id
              }: ${JSON.stringify(response.error)}`,
            });
          }

          resolve(response);
        }
      } catch (err) {
        // If it's not valid JSON, log it
        this.emitEvent({
          type: "log",
          message: `Received non-JSON data: ${line}`,
        });
      }
    }
  }

  /**
   * Wait for themcpServer to be ready
   * @param timeoutMs Timeout in milliseconds
   */
  async waitForReady(timeoutMs: number = 10000): Promise<void> {
    // If already ready, return immediately
    if (this.readyState === "ready") {
      return;
    }

    // If in error state, throw immediately
    if (this.readyState === "error") {
      throw new McpConnectionError(
        this.options.mcpServerName,
        new Error("Server is in error state"),
      );
    }

    return new Promise<void>((resolve, reject) => {
      // Set timeout
      const timeout = setTimeout(() => {
        this.removeListener("event", handler);
        reject(
          new McpTimeoutError(
            this.options.mcpServerName,
            "waitForReady",
            timeoutMs,
          ),
        );
      }, timeoutMs);

      // Handle events
      const handler = (event: McpStdioClientEvent) => {
        if (event.type === "ready") {
          clearTimeout(timeout);
          this.removeListener("event", handler);
          resolve();
        } else if (event.type === "error") {
          clearTimeout(timeout);
          this.removeListener("event", handler);
          reject(
            new McpConnectionError(
              this.options.mcpServerName,
              new Error(event.message),
            ),
          );
        } else if (event.type === "exit" && event.code !== 0) {
          clearTimeout(timeout);
          this.removeListener("event", handler);
          reject(
            new McpConnectionError(
              this.options.mcpServerName,
              new Error(`Process exited with code ${event.code}`),
            ),
          );
        }
      };

      // Listen for events
      this.on("event", handler);

      // Send init request
      this.send({
        method: "init",
      }).catch((err) => {
        clearTimeout(timeout);
        this.removeListener("event", handler);
        reject(err);
      });
    });
  }

  /**
   * Send a request to the server
   * @param request The request to send
   * @param timeoutMs Timeout in milliseconds (optional)
   */
  async send(request: McpRequest, timeoutMs?: number): Promise<any> {
    // Ensure process is started
    if (!this.process) {
      await this.start();
    }

    // Ensure we have a process
    if (!this.process) {
      throw new McpConnectionError(
        this.options.mcpServerName,
        new Error("Failed to start process"),
      );
    }

    // Ensure request has an ID
    if (!request.id) {
      request.id = ++this.lastRequestId;
    }

    return new Promise<any>((resolve, reject) => {
      const actualTimeout = timeoutMs || this.options.requestTimeoutMs;

      // Set up timeout
      const timeout = setTimeout(() => {
        if (this.requests.has(request.id!)) {
          this.requests.delete(request.id!);
          reject(
            new McpTimeoutError(
              this.options.mcpServerName,
              `request '${request.method}'`,
              actualTimeout,
            ),
          );
        }
      }, actualTimeout);

      // Store request
      this.requests.set(request.id!, { resolve, reject, timeout });

      // Send request
      const requestJson = JSON.stringify(request);

      try {
        // Check if process and stdin are available
        if (!this.process || !this.process.stdin) {
          throw new Error("Process or stdin not available");
        }

        this.process.stdin.write(requestJson + "\n");

        this.emitEvent({
          type: "log",
          message: `Sent request: ${requestJson}`,
        });
      } catch (err) {
        clearTimeout(timeout);
        this.requests.delete(request.id!);

        reject(
          new McpConnectionError(
            this.options.mcpServerName,
            err instanceof Error ? err : new Error(String(err)),
          ),
        );
      }
    });
  }

  /**
   * Reject all pending requests
   * @param reason The reason for rejection
   */
  private rejectAllRequests(reason: string): void {
    for (const [id, { reject, timeout }] of this.requests.entries()) {
      clearTimeout(timeout);
      reject(
        new McpConnectionError(this.options.mcpServerName, new Error(reason)),
      );
      this.requests.delete(id);
    }
  }

  /**
   * Kill the process and clean up resources
   */
  kill(): void {
    if (!this.process) {
      return;
    }

    this.isKilled = true;

    try {
      // Abort any operations
      this.abortController.abort();

      // Kill the process
      this.process.kill();

      // Reject all pending requests
      this.rejectAllRequests("Process killed");

      this.emitEvent({ type: "log", message: "Process killed" });
    } catch (err) {
      this.emitEvent({
        type: "error",
        message: `Error killing process: ${
          err instanceof Error ? err.message : String(err)
        }`,
      });
    }

    this.process = null;
  }
}
