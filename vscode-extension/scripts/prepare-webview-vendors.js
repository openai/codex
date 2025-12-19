/* eslint-disable no-console */

const fs = require("node:fs");
const path = require("node:path");

function main() {
  const repoRoot = path.resolve(__dirname, "..");
  const src = (() => {
    const rel = path.join("markdown-it", "dist", "markdown-it.min.js");
    try {
      return require.resolve(rel, { paths: [repoRoot] });
    } catch {
      return require.resolve(rel, { paths: [path.resolve(repoRoot, "..")] });
    }
  })();
  const destDir = path.resolve(__dirname, "../resources/vendor");
  const dest = path.join(destDir, "markdown-it.min.js");

  if (!fs.existsSync(src)) {
    throw new Error(`markdown-it not found at ${src}. Run pnpm install.`);
  }

  fs.mkdirSync(destDir, { recursive: true });
  fs.copyFileSync(src, dest);
  console.log(`Prepared vendor: ${path.relative(repoRoot, dest)}`);
}

main();
