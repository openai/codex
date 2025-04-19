import { CONFIG_DIR } from "./config";
import boxen from "boxen";
import chalk from "chalk";
import * as cp from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import which from "which";

interface UpdateCheckState {
  lastUpdateCheck?: string;
}

interface PackageInfo {
  current: string;
  wanted: string;
  latest: string;
  dependent: string;
  location: string;
}

interface UpdateCheckInfo {
  currentVersion: string;
  latestVersion: string;
}

const UPDATE_CHECK_FREQUENCY = 1000 * 60 * 60 * 24; // 1 day

export async function getNPMCommandPath(): Promise<string | undefined> {
  try {
    return await which(process.platform === "win32" ? "npm.cmd" : "npm");
  } catch {
    return undefined;
  }
}

export async function checkOutdated(
  npmCommandPath: string,
): Promise<UpdateCheckInfo | undefined> {
  return new Promise((resolve, _reject) => {
    // TODO: support local installation
    // Right now we're using "--global", which only checks global packages.
    // But codex might be installed locally — we should check the local version first,
    // and only fall back to the global one if needed.
    const args = ["outdated", "--global", "--json", "--", "@openai/codex"];
    // corepack npm wrapper would automatically update package.json. disable that behavior.
    // COREPACK_ENABLE_AUTO_PIN disables the package.json overwrite, and
    // COREPACK_ENABLE_PROJECT_SPEC makes the npm view command succeed
    //   even if packageManager specified a package manager other than npm.
    const env = {
      ...process.env,
      COREPACK_ENABLE_AUTO_PIN: "0",
      COREPACK_ENABLE_PROJECT_SPEC: "0",
    };
    let options: cp.ExecFileOptions = { env };
    let commandPath = npmCommandPath;
    if (process.platform === "win32") {
      options = { ...options, shell: true };
      commandPath = `"${npmCommandPath}"`;
    }
    cp.execFile(commandPath, args, options, async (_error, stdout) => {
      try {
        const { name: packageName } = await import("../../package.json");
        const content: Record<string, PackageInfo> = JSON.parse(stdout);
        if (!content[packageName]) {
          // package not installed or not outdated
          resolve(undefined);
          return;
        }

        const currentVersion = content[packageName].current;
        const latestVersion = content[packageName].latest;

        resolve({ currentVersion, latestVersion });
        return;
      } catch {
        // ignore
      }
      resolve(undefined);
    });
  });
}

export async function checkForUpdates(): Promise<void> {
  const stateFile = join(CONFIG_DIR, "update-check.json");
  let state: UpdateCheckState | undefined;
  try {
    state = JSON.parse(await readFile(stateFile, "utf8"));
  } catch {
    // ignore
  }

  if (
    state?.lastUpdateCheck &&
    Date.now() - new Date(state.lastUpdateCheck).valueOf() <
      UPDATE_CHECK_FREQUENCY
  ) {
    return;
  }

  const npmCommandPath = await getNPMCommandPath();
  if (!npmCommandPath) {
    return;
  }

  const packageInfo = await checkOutdated(npmCommandPath);

  await writeState(stateFile, {
    ...state,
    lastUpdateCheck: new Date().toUTCString(),
  });

  if (!packageInfo) {
    return;
  }

  const updateMessage = `To update, run: ${chalk.cyan(
    "npm install -g @openai/codex",
  )} to update.`;

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

async function writeState(stateFilePath: string, state: UpdateCheckState) {
  await writeFile(stateFilePath, JSON.stringify(state, null, 2), {
    encoding: "utf8",
  });
}
