#!/usr/bin/env node
"use strict";

const fs = require("node:fs/promises");
const os = require("node:os");
const path = require("node:path");

async function fileExists(filePath) {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}

async function main() {
  const packageRoot = path.resolve(__dirname, "..");
  const sourcePath = path.join(packageRoot, "bin", "mcp-server.js");
  if (!(await fileExists(sourcePath))) {
    throw new Error(
      `Build output not found at ${sourcePath}. Run "pnpm run build" first.`,
    );
  }

  const codexHome =
    typeof process.env.CODEX_HOME === "string" && process.env.CODEX_HOME.trim()
      ? process.env.CODEX_HOME.trim()
      : os.homedir() && os.homedir().length > 0
        ? path.join(os.homedir(), ".codex")
        : path.join(process.cwd(), ".codex");

  const destPath = path.join(
    codexHome,
    "plugins",
    "google-workspace-mcp",
    "bin",
    "mcp-server.js",
  );

  await fs.mkdir(path.dirname(destPath), { recursive: true });
  await fs.copyFile(sourcePath, destPath);
  await fs.chmod(destPath, 0o755);

  // eslint-disable-next-line no-console
  console.log(`Installed ${sourcePath} -> ${destPath}`);
}

void main().catch((err) => {
  // eslint-disable-next-line no-console
  console.error(err instanceof Error ? err.message : String(err));
  process.exit(1);
});
