import type { AgentName } from "package-manager-detector";

import { execFileSync } from "node:child_process";
import { join, resolve } from "node:path";
import which from "which";

function isInstalled(manager: AgentName): boolean {
  try {
    which.sync(manager);
    return true;
  } catch {
    return false;
  }
}

function getGlobalBinDir(manager: AgentName): string | undefined {
  if (!isInstalled(manager)) {
    return;
  }

  try {
    switch (manager) {
      case "npm": {
        const stdout = execFileSync("npm", ["prefix", "-g"], {
          encoding: "utf-8",
        });
        return join(stdout.trim(), "bin");
      }

      case "pnpm": {
        // pnpm bin -g prints the bin dir
        const stdout = execFileSync("pnpm", ["bin", "-g"], {
          encoding: "utf-8",
        });
        return stdout.trim();
      }

      case "bun": {
        // bun pm bin -g prints your bun global bin folder
        const stdout = execFileSync("bun", ["pm", "bin", "-g"], {
          encoding: "utf-8",
        });
        return stdout.trim();
      }

      default:
        return undefined;
    }
  } catch {
    // ignore
  }

  return undefined;
}

export async function detectInstallerByPath(): Promise<AgentName | undefined> {
  // e.g. /usr/local/bin/codex
  const invoked = process.argv[1] && resolve(process.argv[1]);
  if (!invoked) {
    return;
  }

  const supportedManagers: Array<AgentName> = ["npm", "pnpm", "bun"];

  for (const mgr of supportedManagers) {
    const binDir = getGlobalBinDir(mgr);
    if (binDir && invoked.startsWith(binDir)) {
      return mgr;
    }
  }

  return undefined;
}
