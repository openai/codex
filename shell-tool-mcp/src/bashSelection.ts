import path from "node:path";
import os from "node:os";
import { DARWIN_BASH_VARIANTS, LINUX_BASH_VARIANTS } from "./constants";
import { BashSelection, OsReleaseInfo } from "./types";

function supportedDetail(variants: ReadonlyArray<{ name: string }>): string {
  return `Supported variants: ${variants.map((variant) => variant.name).join(", ")}`;
}

export function selectLinuxBash(
  bashRoot: string,
  info: OsReleaseInfo,
): BashSelection {
  const versionId = info.versionId;
  const candidates: Array<{
    variant: (typeof LINUX_BASH_VARIANTS)[number];
    matchesVersion: boolean;
  }> = [];
  for (const variant of LINUX_BASH_VARIANTS) {
    const matchesId =
      variant.ids.includes(info.id) ||
      variant.ids.some((id) => info.idLike.includes(id));
    if (!matchesId) {
      continue;
    }
    const matchesVersion = Boolean(
      versionId &&
        variant.versions.some((prefix) => versionId.startsWith(prefix)),
    );
    candidates.push({ variant, matchesVersion });
  }

  const pickVariant = (list: typeof candidates) =>
    list.find((item) => item.variant)?.variant;

  const preferred = pickVariant(
    candidates.filter((item) => item.matchesVersion),
  );
  if (preferred) {
    return {
      path: path.join(bashRoot, preferred.name, "bash"),
      variant: preferred.name,
    };
  }

  const fallbackMatch = pickVariant(candidates);
  if (fallbackMatch) {
    return {
      path: path.join(bashRoot, fallbackMatch.name, "bash"),
      variant: fallbackMatch.name,
    };
  }

  const fallback = LINUX_BASH_VARIANTS[0];
  if (fallback) {
    return {
      path: path.join(bashRoot, fallback.name, "bash"),
      variant: fallback.name,
    };
  }

  const detail = supportedDetail(LINUX_BASH_VARIANTS);
  throw new Error(
    `Unable to select a Bash variant for ${info.id || "unknown"} ${versionId || ""}. ${detail}`,
  );
}

export function selectDarwinBash(
  bashRoot: string,
  darwinRelease: string,
): BashSelection {
  const darwinMajor = Number.parseInt(darwinRelease.split(".")[0] || "0", 10);
  const preferred = DARWIN_BASH_VARIANTS.find(
    (variant) => darwinMajor >= variant.minDarwin,
  );
  if (preferred) {
    return {
      path: path.join(bashRoot, preferred.name, "bash"),
      variant: preferred.name,
    };
  }

  const fallback = DARWIN_BASH_VARIANTS[0];
  if (fallback) {
    return {
      path: path.join(bashRoot, fallback.name, "bash"),
      variant: fallback.name,
    };
  }

  const detail = supportedDetail(DARWIN_BASH_VARIANTS);
  throw new Error(
    `Unable to select a macOS Bash build (darwin ${darwinMajor}). ${detail}`,
  );
}

export function resolveBashPath(
  targetRoot: string,
  platform: NodeJS.Platform,
  darwinRelease = os.release(),
  osInfo: OsReleaseInfo | null = null,
): BashSelection {
  const bashRoot = path.join(targetRoot, "bash");

  if (platform === "linux") {
    if (!osInfo) {
      throw new Error("Linux OS info is required to select a Bash variant.");
    }
    return selectLinuxBash(bashRoot, osInfo);
  }
  if (platform === "darwin") {
    return selectDarwinBash(bashRoot, darwinRelease);
  }
  throw new Error(`Unsupported platform for Bash selection: ${platform}`);
}
