import fs from "fs";
import path from "path";
import glob from "fast-glob";

export function resolveSmartPath(
  requestedPath: string,
  cwd = process.cwd(),
): string {
  const fullPath = path.resolve(cwd, requestedPath);
  if (fs.existsSync(fullPath)) return fullPath;

  const requestedBase = path.basename(requestedPath).replace(/\.[jt]sx?$/, "");
  const candidates = glob.sync("src/**/*.{ts,tsx,js,jsx}", {
    cwd,
    absolute: true,
  });

  const ranked = candidates
    .map((candidate) => ({
      path: candidate,
      score: fuzzyScore(candidate, requestedBase),
    }))
    .filter(({ score }) => score > 0)
    .sort((a, b) => b.score - a.score);

  if (ranked.length > 0) {
    const bestMatch = ranked[0]?.path;
    if (bestMatch) {
      console.warn(
        `⚠️ Requested path "${requestedPath}" not found. Falling back to: ${bestMatch}`,
      );
      return bestMatch;
    }
  }

  throw new Error(`❌ Could not resolve path for: ${requestedPath}`);
}

function fuzzyScore(filePath: string, target: string): number {
  const norm = filePath.toLowerCase();
  const name = path.basename(norm, path.extname(norm));
  if (name === target) return 100;
  if (name.includes(target)) return 50;
  if (norm.includes(target)) return 30;
  return 0;
}
