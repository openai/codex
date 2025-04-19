/**
 * Utility functions for handling platform-specific commands
 */

import { log, isLoggingEnabled } from "./log.js";

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

  // Additionally translate any Linux mount paths (/mnt/c/...) to native
  // Windows drive paths and normalise back‑slashes so that PowerShell and
  // cmd.exe commands work without modification.
  for (let i = 1; i < adaptedCommand.length; i++) {
    const token = adaptedCommand[i];
    if (typeof token === "string") {
      let transformed = token;
      // If the command operates on $Env:USERPROFILE\Downloads and attempts to
      // write a *relative* file like "downloads.html" we prepend the full
      // Downloads path so the output ends up where the user expects.  This
      // kicks in when the token follows an Out‑File/Start‑Process flag.
      if (
        /Out-File|Start-Process/i.test(adaptedCommand[0] ?? "") &&
        !/^[A-Z]:\\/.test(transformed) &&
        /downloads/i.test(transformed) &&
        !transformed.includes("\\")
      ) {
        const dl = `${process.env["USERPROFILE"]}\\Downloads`;
        transformed = `${dl}\\${transformed}`;
      }
      // /mnt/c/Users -> C:\Users
      transformed = transformed.replace(/^\/mnt\/([a-z])\//i, (_m, d) => {
        return `${d.toUpperCase()}:\\`;
      });
      // Replace forward slashes with backslashes if the token now looks like a
      // Windows absolute path.
      if (/^[A-Z]:\//.test(transformed)) {
        transformed = transformed.replaceAll("/", "\\");
      }
      adaptedCommand[i] = transformed;
    }
  }

  if (isLoggingEnabled()) {
    log(`Adapted command: ${adaptedCommand.join(" ")}`);
  }

  return adaptedCommand;
}
