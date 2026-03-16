// Reports the path to the appropriate Bash binary bundled in this package.

import os from "node:os";
import path from "node:path";
import { resolveBashPath } from "./bashSelection";
import { readOsRelease } from "./osRelease";
import { resolveTargetTriple } from "./platform";

async function main(): Promise<void> {
  const targetTriple = resolveTargetTriple(process.platform, process.arch);
  const vendorRoot = path.resolve(__dirname, "..", "vendor");
  const targetRoot = path.join(vendorRoot, targetTriple);

  const osInfo = process.platform === "linux" ? readOsRelease() : null;
  const { path: bashPath } = resolveBashPath(
    targetRoot,
    process.platform,
    os.release(),
    osInfo,
  );

  console.log(`Platform Bash is: ${bashPath}`);
}

void main().catch((err) => {
  // eslint-disable-next-line no-console
  console.error(err);
  process.exit(1);
});
