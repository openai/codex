/**
 * Utility functions for handling platform-specific commands
 */

import { log, isLoggingEnabled } from "./log.js";
import * as fs from "fs";
import * as path from "path";

/**
 * Map of Unix commands to their Windows equivalents
 */
const COMMAND_MAP: Record<string, string> = {
  ls: "dir",
  grep: "findstr",
  cat: "type",
  rm: "del",
  cp: "copy",
  mv: "move",
  touch: "echo.>",
  mkdir: "md",
};

/**
 * Map of common Unix command options to their Windows equivalents
 */
const OPTION_MAP: Record<string, Record<string, string>> = {
  ls: {
    "-l": "/p",
    "-a": "/a",
    "-R": "/s",
  },
  grep: {
    "-i": "/i",
    "-r": "/s",
  },
};

/**
 * Adapts a command for the current platform.
 * On Windows, this will translate Unix commands to their Windows equivalents.
 * On Unix-like systems, this will return the original command.
 *
 * @param command The command array to adapt
 * @returns The adapted command array
 */
export function adaptCommandForPlatform(command: Array<string>): Array<string> {
  // First, check if we need to adapt Go commands for sandbox compatibility
  const adaptedForSandbox = adaptGoCommandForSandbox(command);
  if (adaptedForSandbox) {
    return adaptedForSandbox;
  }

  // If not on Windows, return the original command
  if (process.platform !== "win32") {
    return command;
  }

  // Nothing to adapt if the command is empty
  if (command.length === 0) {
    return command;
  }

  const cmd = command[0];

  // If cmd is undefined or the command doesn't need adaptation, return it as is
  if (!cmd || !COMMAND_MAP[cmd]) {
    return command;
  }

  if (isLoggingEnabled()) {
    log(`Adapting command '${cmd}' for Windows platform`);
  }

  // Create a new command array with the adapted command
  const adaptedCommand = [...command];
  adaptedCommand[0] = COMMAND_MAP[cmd];

  // Adapt options if needed
  const optionsForCmd = OPTION_MAP[cmd];
  if (optionsForCmd) {
    for (let i = 1; i < adaptedCommand.length; i++) {
      const option = adaptedCommand[i];
      if (option && optionsForCmd[option]) {
        adaptedCommand[i] = optionsForCmd[option];
      }
    }
  }

  if (isLoggingEnabled()) {
    log(`Adapted command: ${adaptedCommand.join(" ")}`);
  }

  return adaptedCommand;
}

/**
 * Adapts Go commands to work within the sandbox environment on macOS.
 * This solves the issue where Go tries to create temporary directories in
 * system locations that are not accessible from the sandbox.
 *
 * @param command The command array to adapt
 * @returns The adapted command array or null if no adaptation is needed
 */
function adaptGoCommandForSandbox(
  command: Array<string>,
): Array<string> | null {
  // Only adapt on macOS and when the command is Go-related
  if (
    process.platform !== "darwin" ||
    command.length === 0 ||
    command[0] !== "go"
  ) {
    return null;
  }

  // Create a tmp directory in the current working directory for Go to use
  const tmpDir = path.join(process.cwd(), ".tmp");
  try {
    if (!fs.existsSync(tmpDir)) {
      fs.mkdirSync(tmpDir, { recursive: true });
    }
  } catch (error) {
    log(`Warning: Failed to create temporary directory for Go: ${error}`);
    return null;
  }

  if (isLoggingEnabled()) {
    log(`Adapting Go command for macOS sandbox: setting GOTMPDIR=${tmpDir}`);
  }

  // Prepend environment variable to the command
  return ["env", `GOTMPDIR=${tmpDir}`, ...command];
}
