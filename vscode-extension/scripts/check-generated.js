/* eslint-disable no-console */
const fs = require("node:fs");
const path = require("node:path");

function mustExist(relPath) {
  const p = path.resolve(__dirname, "..", relPath);
  if (!fs.existsSync(p)) {
    console.error(`[check-generated] missing: ${relPath}`);
    return false;
  }
  return true;
}

function main() {
  const ok =
    mustExist("src/generated/ClientRequest.ts") &&
    mustExist("src/generated/ServerNotification.ts") &&
    mustExist("src/generated/v2/ThreadStartParams.ts") &&
    mustExist("src/generated/v2/TurnStartParams.ts");

  if (!ok) {
    console.error(
      "[check-generated] Generated protocol files are missing. Run `pnpm run regen:protocol` (optionally: `pnpm run regen:protocol -- --codex-bin <path-to-codex>` or set `CODEX_BIN`).",
    );
    process.exit(1);
  }
}

main();
