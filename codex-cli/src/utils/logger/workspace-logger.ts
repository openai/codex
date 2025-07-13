import * as fs from "fs";
import * as fsPromises from "fs/promises";
import * as path from "path";

export enum LogEntryType {
  SESSION_START = "SESSION_START",
  SESSION_END = "SESSION_END",
  USER_INPUT = "USER_INPUT",
  MODEL_REASONING = "MODEL_REASONING",
  MODEL_ACTION = "MODEL_ACTION",
  FILE_CHANGE = "FILE_CHANGE",
  COMMAND = "COMMAND",
  ERROR = "ERROR",
}

export class WorkspaceLogger {
  private logFilePath: string;
  private sessionStartTime: Date;
  private fileChanges = 0;
  private commandsExecuted = 0;
  private isWriting: boolean = false;
  private queue: Array<string> = [];

  constructor(workspacePath: string) {
    this.sessionStartTime = new Date();
    const timestamp = this.formatTimestamp(this.sessionStartTime, "file");
    const logDir = path.join(workspacePath, ".codex", "logs");

    // Create directory if it doesn't exist
    fs.mkdirSync(logDir, { recursive: true });

    this.logFilePath = path.join(logDir, `codex-session-${timestamp}.log`);
    this.logSessionStart(workspacePath);
  }

  private formatTimestamp(date: Date, format: "log" | "file" = "log"): string {
    if (format === "file") {
      return date.toISOString().replace(/:/g, "-").replace(/\..+/, "");
    }
    return date.toISOString().split(".")[0].replace("T", " ");
  }

  private async appendToLog(entry: string): Promise<void> {
    this.queue.push(entry + "\n");
    await this.flushQueue();
  }

  private async flushQueue(): Promise<void> {
    if (this.isWriting || this.queue.length === 0) {
      return;
    }

    this.isWriting = true;
    const entries = this.queue.join("");
    this.queue = [];

    try {
    await fsPromises.appendFile(this.logFilePath, entries);
    } catch (error) {
    this.queue.push(`[ERROR] Failed to write to workspace log: ${error}\n`);
    // eslint-disable-next-line no-console
    console.error("Failed to write to workspace log:", error);
    } finally {
    this.isWriting = false;
    }

    if (this.queue.length > 0) {
      await this.flushQueue();
    }
  }

  private async logEntry(
    type: LogEntryType,
    content: string,
    details?: Array<string>,
  ): Promise<void> {
    const timestamp = this.formatTimestamp(new Date());
    let entry = `[${timestamp}] [${type}] ${content}`;

    if (details && details.length > 0) {
      entry += "\n    " + details.join("\n    ");
    }

    await this.appendToLog(entry);
  }

  async logSessionStart(workspacePath: string): Promise<void> {
    await this.logEntry(
      LogEntryType.SESSION_START,
      "New Codex session initialized",
      [
        `Working directory: ${workspacePath}`,
        `Session ID: ${Math.random().toString(36).substring(2, 10)}`,
      ],
    );
  }

  async logSessionEnd(): Promise<void> {
    const endTime = new Date();
    const duration =
      (endTime.getTime() - this.sessionStartTime.getTime()) / 1000;
    const minutes = Math.floor(duration / 60);
    const seconds = Math.floor(duration % 60);

    await this.logEntry(LogEntryType.SESSION_END, "Session completed", [
      `Duration: ${minutes}m ${seconds}s`,
      `Files changed: ${this.fileChanges}`,
      `Commands executed: ${this.commandsExecuted}`,
    ]);
  }

  async logUserInput(input: string, rawInput?: string): Promise<void> {
    const details = rawInput ? [`Raw prompt: "${rawInput}"`] : [];
    await this.logEntry(LogEntryType.USER_INPUT, input, details);
  }

  async logModelReasoning(reasoning: string): Promise<void> {
    const details = reasoning
      .split("\n")
      .filter((line) => line.trim().length > 0);

    await this.logEntry(
      LogEntryType.MODEL_REASONING,
      "Planning approach",
      details,
    );
  }

  async logModelAction(action: string, target?: string): Promise<void> {
    const details = target ? [`Target: ${target}`] : [];
    await this.logEntry(LogEntryType.MODEL_ACTION, action, details);
  }

  async logFileChange(
    changeType: "Created" | "Modified" | "Deleted",
    filePath: string,
  ): Promise<void> {
    this.fileChanges++;
    const details = [];

    if (changeType !== "Deleted" && fs.existsSync(filePath)) {
      const stats = fs.statSync(filePath);
      details.push(`Size: ${stats.size} bytes`);
    }

    await this.logEntry(
      LogEntryType.FILE_CHANGE,
      `${changeType}: ${filePath}`,
      details,
    );
  }

  async logCommand(
    command: string,
    exitCode: number,
    duration: number,
    output?: string,
  ): Promise<void> {
    this.commandsExecuted++;
    const details = [
      `Exit code: ${exitCode}`,
      `Duration: ${duration.toFixed(1)}s`,
    ];

    if (output) {
      details.push(
        `Output: ${output.length > 100 ? output.substring(0, 100) + "..." : output}`,
      );
    }

    await this.logEntry(LogEntryType.COMMAND, command, details);
  }

  async logError(message: string, error?: Error): Promise<void> {
    const details = [];
    if (error) {
      details.push(`Error: ${error.message}`);
      if (error.stack) {
        details.push(
          `Stack: ${error.stack.substring(0, 500)}${error.stack.length > 500 ? "..." : ""}`,
        );
      }
    }

    await this.logEntry(LogEntryType.ERROR, message, details);
  }
}

// Singleton instance
let workspaceLogger: WorkspaceLogger | null = null;

export function initWorkspaceLogger(workspacePath: string): WorkspaceLogger {
  if (!workspaceLogger) {
    workspaceLogger = new WorkspaceLogger(workspacePath);
  }
  return workspaceLogger;
}

export function getWorkspaceLogger(): WorkspaceLogger | null {
  return workspaceLogger;
}
