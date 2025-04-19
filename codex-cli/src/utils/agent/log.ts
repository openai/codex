import * as fsSync from "fs";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";

interface Logger {
  /** Checking this can be used to avoid constructing a large log message. */
  isLoggingEnabled(): boolean;

  log(message: string): void;
}

class AsyncLogger implements Logger {
  private queue: Array<string> = [];
  private isWriting: boolean = false;

  constructor(private filePath: string) {
    this.filePath = filePath;
  }

  isLoggingEnabled(): boolean {
    return true;
  }

  log(message: string): void {
    const entry = `[${now()}] ${message}\n`;
    this.queue.push(entry);
    this.maybeWrite();
  }

  private async maybeWrite(): Promise<void> {
    if (this.isWriting || this.queue.length === 0) {
      return;
    }

    this.isWriting = true;
    const messages = this.queue.join("");
    this.queue = [];

    try {
      await fs.appendFile(this.filePath, messages);
    } finally {
      this.isWriting = false;
    }

    this.maybeWrite();
  }
}

class EmptyLogger implements Logger {
  isLoggingEnabled(): boolean {
    return false;
  }

  log(_message: string): void {
    // No-op
  }
}

function now() {
  const date = new Date();
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  const hours = String(date.getHours()).padStart(2, "0");
  const minutes = String(date.getMinutes()).padStart(2, "0");
  const seconds = String(date.getSeconds()).padStart(2, "0");
  return `${year}-${month}-${day}T${hours}:${minutes}:${seconds}`;
}

let logger: Logger;

/**
 * Creates a .log file for this session, but also symlinks codex-cli-latest.log
 * to the current log file so you can reliably run:
 *
 * - Mac/Windows: `tail -F "$TMPDIR/oai-codex/codex-cli-latest.log"`
 * - Linux: `tail -F ~/.local/oai-codex/codex-cli-latest.log`
 */
export function initLogger(): Logger {
  if (logger) {
    return logger;
  } else if (!process.env["DEBUG"]) {
    logger = new EmptyLogger();
    return logger;
  }

  const isMac = process.platform === "darwin";
  const isWin = process.platform === "win32";

  // On Mac and Windows, os.tmpdir() returns a user-specific folder, so prefer
  // it there. On Linux, use ~/.local/oai-codex so logs are not world-readable.
  const logDir =
    isMac || isWin
      ? path.join(os.tmpdir(), "oai-codex")
      : path.join(os.homedir(), ".local", "oai-codex");
  fsSync.mkdirSync(logDir, { recursive: true });
  const logFile = path.join(logDir, `codex-cli-${now()}.log`);
  // Write the empty string so the file exists and can be tail'd.
  fsSync.writeFileSync(logFile, "");

  // Symlink to codex-cli-latest.log on UNIX because Windows is funny about
  // symlinks.
  if (!isWin) {
    const latestLink = path.join(logDir, "codex-cli-latest.log");
    try {
      fsSync.symlinkSync(logFile, latestLink, "file");
    } catch (err: unknown) {
      const error = err as NodeJS.ErrnoException;
      if (error.code === "EEXIST") {
        fsSync.unlinkSync(latestLink);
        fsSync.symlinkSync(logFile, latestLink, "file");
      } else {
        throw err;
      }
    }
  }

  logger = new AsyncLogger(logFile);
  return logger;
}

export function log(message: string): void {
  (logger ?? initLogger()).log(message);
}

/**
 * USE SPARINGLY! This function should only be used to guard a call to log() if
 * the log message is large and you want to avoid constructing it if logging is
 * disabled.
 *
 * `log()` is already a no-op if DEBUG is not set, so an extra
 * `isLoggingEnabled()` check is unnecessary.
 */
export function isLoggingEnabled(): boolean {
  return (logger ?? initLogger()).isLoggingEnabled();
}
