/* eslint-disable no-console */
const { execFileSync } = require("node:child_process");
const path = require("node:path");

function readArg(flag) {
  const idx = process.argv.indexOf(flag);
  if (idx === -1) return null;
  const value = process.argv[idx + 1];
  if (!value) throw new Error(`Missing value for ${flag}`);
  return value;
}

function main() {
  const codexBin = readArg("--codex-bin") ?? process.env.CODEX_BIN ?? "codex";
  const outDir = path.resolve(__dirname, "..", "src", "generated");

  console.log(`[regen-protocol] codexBin=${codexBin}`);
  console.log(`[regen-protocol] outDir=${outDir}`);

  execFileSync(codexBin, ["app-server", "generate-ts", "--out", outDir], {
    stdio: "inherit",
  });
  execFileSync(
    codexBin,
    ["app-server", "generate-json-schema", "--out", outDir],
    {
      stdio: "inherit",
    },
  );
}

main();
