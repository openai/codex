import { log as codeXLog } from "./agent/log";

/**
 * Log levels for MCP logging
 */
export enum LogLevel {
  DEBUG = 0,
  INFO = 1,
  WARN = 2,
  ERROR = 3,
  NONE = 4,
}

// Default log level - can be overridden at runtime
let currentLogLevel = LogLevel.INFO;

// Whether to output debug logs to console
let debugToConsole = false;

/**
 * Configure the MCP logger
 * @param options Configuration options for the logger
 */
export function configureMcpLogger(options: {
  level?: LogLevel;
  debugToConsole?: boolean;
}): void {
  if (options.level !== undefined) {
    currentLogLevel = options.level;
  }

  if (options.debugToConsole !== undefined) {
    debugToConsole = options.debugToConsole;
  }
}

/**
 * Enable or disable debug output to console
 * @param enabled Whether to output debug logs to console
 */
export function enableConsoleDebug(enabled: boolean): void {
  debugToConsole = enabled;
  mcpLog(
    LogLevel.INFO,
    `Console debug output ${enabled ? "enabled" : "disabled"}`,
  );
}

/**
 * Set the current log level
 * @param level The log level to set
 */
export function setLogLevel(level: LogLevel): void {
  const oldLevel = currentLogLevel;
  currentLogLevel = level;
  mcpLog(
    LogLevel.INFO,
    `Log level changed from ${LogLevel[oldLevel]} to ${LogLevel[level]}`,
  );
}

/**
 * Get the current log level
 * @returns The current log level
 */
export function getLogLevel(): LogLevel {
  return currentLogLevel;
}

/**
 * Check if a given log level is enabled
 * @param level The log level to check
 * @returns Whether the log level is enabled
 */
export function isLevelEnabled(level: LogLevel): boolean {
  return level >= currentLogLevel;
}

/**
 * Log a message at the specified log level
 * @param level The log level for the message
 * @param message The message to log
 * @param args Optional additional arguments for the message
 */
export function mcpLog(
  level: LogLevel,
  message: string,
  ...args: Array<any>
): void {
  if (level >= currentLogLevel) {
    const prefix = LogLevel[level];
    const timestamp = new Date().toISOString();

    // Format the message with timestamp and level
    const formattedMessage = `[MCP:${prefix}][${timestamp}] ${message}`;

    // Always log to the CodeX log file
    codeXLog(formattedMessage);

    // Additional objects get stringified for the log
    if (args.length > 0) {
      try {
        const argsStr = args
          .map((arg) =>
            typeof arg === "object"
              ? JSON.stringify(arg, null, 2)
              : String(arg),
          )
          .join(" ");
        codeXLog(argsStr);
      } catch (err) {
        codeXLog(`[Error stringifying log args: ${err}]`);
      }
    }

    // Output to console based on settings
    if (debugToConsole || level > LogLevel.DEBUG) {
      // Use appropriate console method based on level
      switch (level) {
        case LogLevel.ERROR:
          console.error(formattedMessage, ...args);
          break;
        case LogLevel.WARN:
          console.warn(formattedMessage, ...args);
          break;
        case LogLevel.INFO:
          console.log(formattedMessage, ...args);
          break;
        case LogLevel.DEBUG:
          console.debug(formattedMessage, ...args);
          break;
      }
    }
  }
}

/**
 * Log a debug message
 * @param message The message to log
 * @param args Optional additional arguments for the message
 */
export function debug(message: string, ...args: Array<any>): void {
  mcpLog(LogLevel.DEBUG, message, ...args);
}

/**
 * Log an info message
 * @param message The message to log
 * @param args Optional additional arguments for the message
 */
export function info(message: string, ...args: Array<any>): void {
  mcpLog(LogLevel.INFO, message, ...args);
}

/**
 * Log a warning message
 * @param message The message to log
 * @param args Optional additional arguments for the message
 */
export function warn(message: string, ...args: Array<any>): void {
  mcpLog(LogLevel.WARN, message, ...args);
}

/**
 * Log an error message
 * @param message The message to log
 * @param args Optional additional arguments for the message
 */
export function error(message: string, ...args: Array<any>): void {
  mcpLog(LogLevel.ERROR, message, ...args);
}

// Initialize logger based on environment variables
if (process.env["MCP_DEBUG"] === "1") {
  configureMcpLogger({
    level: LogLevel.DEBUG,
    debugToConsole: true,
  });
  debug("MCP debug logging enabled");
}
