import fs from "fs";

/**
 * Portable helper that returns the shell executable and its default argument
 * list for the current platform.
 *
 *   - On POSIX we fall back to the user's $SHELL or /bin/bash -l -c.
 *   - On Windows we prefer PowerShell if present (better ANSI + UTF‑8) and
 *     gracefully fall back to cmd.exe otherwise.
 *
 * Users (or CI) can override behaviour by exporting CODEX_SHELL and optional
 * CODEX_SHELL_ARGS, or by using the CLI flags --shell / --shell-arg once those
 * are wired up.
 */

export interface ShellSpec {
  /** executable path */
  cmd: string;
  /** argument list that will be prepended before the command string */
  args: string[];
}

export function getShell(): ShellSpec {
  // Honour explicit overrides first.
  const envShell = process.env["CODEX_SHELL"];
  if (envShell) {
    return {
      cmd: envShell,
      args: (process.env["CODEX_SHELL_ARGS"] ?? "").split(" ").filter(Boolean),
    };
  }

  // Windows ‑‑ try PowerShell, else cmd.exe.
  if (process.platform === "win32") {
    const psPath = `${process.env["SystemRoot"] ?? "C:"}\\System32\\WindowsPowerShell\\v1.0\\powershell.exe`;
    if (fs.existsSync(psPath)) {
      return {
        cmd: psPath,
        args: [
          "-NoLogo",
          "-NoProfile",
          "-ExecutionPolicy",
          "Bypass",
          "-Command",
        ],
      };
    }
    return {
      cmd: process.env["ComSpec"] ?? "cmd.exe",
      args: ["/d", "/s", "/c"],
    };
  }

  // POSIX default: the user's login shell or bash.
  return {
    cmd: process.env["SHELL"] ?? "/bin/bash",
    args: ["-l", "-c"],
  };
} 