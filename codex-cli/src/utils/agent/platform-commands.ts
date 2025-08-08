/**
 * Utility functions for handling platform-specific commands
 */

import { log } from "../logger/log.js";
import { spawnSync } from "child_process";
import path from "path";
import { fileURLToPath } from "url";

// On Windows, many useful commands are implemented as shell built-ins rather
// than standalone executables. `COMSPEC` points to the command interpreter
// (typically `cmd.exe`) which we use when invoking such built-ins.
const COMSPEC = process.env["COMSPEC"] || "cmd";

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
export function adaptCommandForPlatform(command: Array<string>): {
  command: Array<string>;
  message?: string;
} {
  // If not on Windows, return the original command
  if (process.platform !== "win32") {
    return { command };
  }

  // Nothing to adapt if the command is empty
  if (command.length === 0) {
    return { command };
  }

  try {
    const __filename = fileURLToPath(import.meta.url);
    const scriptPath = path.join(
      path.dirname(__filename),
      "../../scripts/command_validator.py",
    );
    const result = spawnSync("python", [scriptPath, JSON.stringify(command)], {
      encoding: "utf-8",
    });
    if (result.status === 0 && result.stdout) {
      const parsed = JSON.parse(result.stdout.trim());
      if (Array.isArray(parsed.command)) {
        return { command: parsed.command, message: parsed.message };
      }
    } else if (result.stderr) {
      log(`command_validator stderr: ${result.stderr}`);
    }
  } catch (err) {
    log(`command_validator failed: ${String(err)}`);
  }

  // Fallback to simple mapping
  const cmd = command[0];

  // Special cases for commands that are Windows shell built-ins.
  if (cmd === "pwd") {
    log("Adapting command 'pwd' for Windows platform (fallback)");
    return { command: [COMSPEC, "/c", "cd"], message: undefined };
  }
  if (cmd === "env" || cmd === "printenv") {
    log(`Adapting command '${cmd}' for Windows platform (fallback)`);
    return { command: [COMSPEC, "/c", "set"], message: undefined };
  }

  // If cmd is undefined or the command doesn't need adaptation, return it as is
  if (!cmd || !COMMAND_MAP[cmd]) {
    return { command };
  }

  log(`Adapting command '${cmd}' for Windows platform (fallback)`);

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

  log(`Adapted command (fallback): ${adaptedCommand.join(" ")}`);

  return { command: adaptedCommand };
}
