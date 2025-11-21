import { readFileSync } from "node:fs";
import { OsReleaseInfo } from "./types";

export function parseOsRelease(contents: string): OsReleaseInfo {
  const lines = contents.split("\n").filter(Boolean);
  const info: Record<string, string> = {};
  for (const line of lines) {
    const [rawKey, rawValue] = line.split("=", 2);
    if (!rawKey || rawValue === undefined) {
      continue;
    }
    const key = rawKey.toLowerCase();
    const value = rawValue.replace(/^"/, "").replace(/"$/, "");
    info[key] = value;
  }
  const idLike = (info.id_like || "")
    .split(/\s+/)
    .map((item) => item.trim().toLowerCase())
    .filter(Boolean);
  return {
    id: (info.id || "").toLowerCase(),
    idLike,
    versionId: info.version_id || "",
  };
}

export function readOsRelease(pathname = "/etc/os-release"): OsReleaseInfo {
  try {
    const contents = readFileSync(pathname, "utf8");
    return parseOsRelease(contents);
  } catch {
    return { id: "", idLike: [], versionId: "" };
  }
}
