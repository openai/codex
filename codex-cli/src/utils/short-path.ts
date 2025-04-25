import path from "path";

export function shortenPath(p: string, maxLength = 40): string {
  const home = process.env["HOME"];
  // Replace home directory with '~' if applicable.
  const displayPath =
    home !== undefined && p.startsWith(home) ? p.replace(home, "~") : p;
  if (displayPath.length <= maxLength) {
    return displayPath;
  }

  const parts = displayPath.split(path.sep);
  let result = "";
  for (let i = parts.length - 1; i >= 0; i--) {
    const candidate = path.join("~", "...", ...parts.slice(i));
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
