import { normalizePathForDisplay } from "./normalize-path.js";

export function shortenPath(p: string, maxLength = 40): string {
  const home = process.env["HOME"];
  // Replace home directory with '~' if applicable.
  const normalized = normalizePathForDisplay(p);
  const displayPath =
    home !== undefined && normalized.startsWith(home)
      ? normalized.replace(home, "~")
      : normalized;
  if (displayPath.length <= maxLength) {
    return displayPath;
  }

  const parts = displayPath.split("/");
  let result = "";
  for (let i = parts.length - 1; i >= 0; i--) {
    const candidate = ["~", "...", ...parts.slice(i)].join("/");
    if (candidate.length <= maxLength) {
      result = candidate;
    } else {
      break;
    }
  }
  return result || displayPath.slice(-maxLength);
}

export function shortCwd(maxLength = 40): string {
  return shortenPath(process.cwd(), maxLength);
}
