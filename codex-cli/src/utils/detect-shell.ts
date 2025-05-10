import type { ParseEntry } from "shell-quote";

import { spawnSync } from "child_process";
import os from "os";
import path from "path";
import { parse } from "shell-quote";

export interface ShellDetection {
  shellOption?: boolean | string;
  environment: string;
}

function requiresShell(cmd: Array<string>): boolean {
  if (cmd.length === 1 && cmd[0]) {
    const tokens = parse(cmd[0]) as Array<ParseEntry>;
    return tokens.some((t) => typeof t === "object" && "op" in t);
  }
  return false;
}

export function detectShell(cmd?: Array<string>): ShellDetection {
  const override = process.env["CODEX_SHELL"];
  if (override) {
    return { shellOption: override, environment: path.basename(override) };
  }

  // Windows
  if (os.platform() === "win32") {
    // Check git bash
    const msystem = process.env["MSYSTEM"];
    if (msystem && /mingw/i.test(msystem)) {
      return { shellOption: "bash.exe", environment: "Git Bash" };
    }
    // Check pwsh vs powershell
    const keys = Object.keys(process.env).map((k) => k.toLowerCase());
    if (keys.includes("psedition")) {
      return { shellOption: "pwsh.exe", environment: "PowerShell Core" };
    }
    if (keys.includes("psmodulepath")) {
      return { shellOption: "powershell.exe", environment: "PowerShell" };
    }
    // Fallback to parent process
    const out =
      spawnSync("tasklist", ["/fi", `PID eq ${process.ppid}`, "/nh"], {
        encoding: "utf8",
      }).stdout?.trim() || "";
    const exe = out.split(/\s+/)[0]?.toLowerCase();
    if (exe === "bash.exe") {
      return { shellOption: "bash.exe", environment: "Git Bash" };
    }
    if (exe === "pwsh.exe") {
      return { shellOption: "pwsh.exe", environment: "PowerShell Core" };
    }
    if (exe === "powershell.exe") {
      return { shellOption: "powershell.exe", environment: "PowerShell" };
    }

    // final fallback
    const com = process.env["ComSpec"];
    return {
      shellOption: com || true,
      environment: com ? path.basename(com) : "cmd",
    };
  }

  // UNIX‑like
  let shellOption: boolean | string | undefined;
  // Check if the command requires a shell
  if (cmd && requiresShell(cmd)) {
    shellOption = process.env["SHELL"] || true;
  }
  const shellEnv = process.env["SHELL"] || "";
  const base = path.basename(shellEnv);
  const envName =
    base === "bash" ? "Bash" : base === "zsh" ? "Zsh" : base || "sh";
  return { shellOption, environment: envName };
}
