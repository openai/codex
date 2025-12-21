import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const packageRoot = path.resolve(__dirname, "..");
const vendorRoot = path.join(packageRoot, "vendor");

const targets = [
  "aarch64-unknown-linux-musl",
  "x86_64-unknown-linux-musl",
  "aarch64-apple-darwin",
  "x86_64-apple-darwin",
  "aarch64-pc-windows-msvc",
  "x86_64-pc-windows-msvc",
];

if (!fs.existsSync(vendorRoot)) {
  console.error("Missing vendor/ directory. Build artifacts before packing.");
  process.exit(1);
}

const missing = [];
for (const target of targets) {
  const binaryName = target.includes("windows") ? "codexel.exe" : "codexel";
  const binaryPath = path.join(vendorRoot, target, "codex", binaryName);
  if (!fs.existsSync(binaryPath)) {
    missing.push(`${target}/codex/${binaryName}`);
  }
}

if (missing.length > 0) {
  console.error("Missing vendor binaries for publish:");
  for (const entry of missing) {
    console.error(`  - ${entry}`);
  }
  console.error(
    "Populate codex-cli/vendor/ via the release workflow before packing."
  );
  process.exit(1);
}
