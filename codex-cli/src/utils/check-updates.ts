import type { AgentName } from "package-manager-detector";

import { detectInstallerByPath } from "./package-manager-detector";
import { CLI_VERSION } from "../version";
import boxen from "boxen";
import chalk from "chalk";
import { getLatestVersion } from "fast-npm-meta";
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { getUserAgent } from "package-manager-detector";
import semver from "semver";

interface UpdateCheckState {
  lastUpdateCheck?: string;
}

interface UpdateCheckInfo {
  currentVersion: string;
  latestVersion: string;
}

export interface UpdateOptions {
  manager: AgentName;
  packageName: string;
}

const UPDATE_CHECK_FREQUENCY = 1000 * 60 * 60 * 24; // 1 day

export function renderUpdateCommand({
  manager,
  packageName,
}: UpdateOptions): string {
  const updateCommands: Record<AgentName, string> = {
    npm: `npm install -g ${packageName}`,
    pnpm: `pnpm add -g ${packageName}`,
    bun: `bun add -g ${packageName}`,
    /** Only works in yarn@v1 */
    yarn: `yarn global add ${packageName}`,
    deno: `deno install -g npm:${packageName}`,
  };

  return updateCommands[manager];
}

function renderUpdateMessage(options: UpdateOptions) {
  const updateCommand = renderUpdateCommand(options);
  return `To update, run ${chalk.magenta(updateCommand)} to update.`;
}

async function writeState(stateFilePath: string, state: UpdateCheckState) {
  await writeFile(stateFilePath, JSON.stringify(state, null, 2), {
    encoding: "utf8",
  });
}

async function getUpdateCheckInfo(
  packageName: string,
): Promise<UpdateCheckInfo | undefined> {
  const metadata = await getLatestVersion(packageName, {
    force: true,
    throw: false,
  });

  if ("error" in metadata || !metadata?.version) {
    return;
  }

  return {
    currentVersion: CLI_VERSION,
    latestVersion: metadata.version,
  };
}

export async function checkForUpdates(): Promise<void> {
  const { CONFIG_DIR } = await import("./config");
  const stateFile = join(CONFIG_DIR, "update-check.json");

  // Load previous check timestamp
  let state: UpdateCheckState | undefined;
  try {
    state = JSON.parse(await readFile(stateFile, "utf8"));
  } catch {
    // ignore
  }

  // Bail out if we checked less than the configured frequency ago
  if (
    state?.lastUpdateCheck &&
    Date.now() - new Date(state.lastUpdateCheck).valueOf() <
      UPDATE_CHECK_FREQUENCY
  ) {
    return;
  }

  // Fetch current vs latest from the registry
  const { name: packageName } = await import("../../package.json");
  const packageInfo = await getUpdateCheckInfo(packageName);

  await writeState(stateFile, {
    ...state,
    lastUpdateCheck: new Date().toUTCString(),
  });

  if (
    !packageInfo ||
    !semver.gt(packageInfo.latestVersion, packageInfo.currentVersion)
  ) {
    return;
  }

  // Detect global installer
  let managerName = await detectInstallerByPath();

  // Fallback to the local package manager
  if (!managerName) {
    const local = getUserAgent();
    if (!local) {
      // No package managers found, skip it.
      return;
    }
    managerName = local;
  }

  const updateMessage = renderUpdateMessage({
    manager: managerName,
    packageName,
  });

  const box = boxen(
    `\
Update available! ${chalk.red(packageInfo.currentVersion)} → ${chalk.green(
      packageInfo.latestVersion,
    )}.
${updateMessage}`,
    {
      padding: 1,
      margin: 1,
      align: "center",
      borderColor: "yellow",
      borderStyle: "round",
    },
  );

  // eslint-disable-next-line no-console
  console.log(box);
}
