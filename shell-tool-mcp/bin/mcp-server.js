#!/usr/bin/env node
// Launches the codex-exec-mcp-server binary bundled in this package.

import { spawn } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const LINUX_BASH_VARIANTS = [
  { name: "ubuntu-24.04", ids: ["ubuntu"], versions: ["24.04"] },
  { name: "ubuntu-22.04", ids: ["ubuntu"], versions: ["22.04"] },
  { name: "ubuntu-20.04", ids: ["ubuntu"], versions: ["20.04"] },
  { name: "debian-12", ids: ["debian"], versions: ["12"] },
  { name: "debian-11", ids: ["debian"], versions: ["11"] },
  { name: "debian-10", ids: ["debian"], versions: ["10"] },
  { name: "centos-9", ids: ["centos", "rhel", "rocky", "almalinux"], versions: ["9"] },
  { name: "centos-8", ids: ["centos", "rhel", "rocky", "almalinux"], versions: ["8"] },
  { name: "centos-7", ids: ["centos", "rhel"], versions: ["7"] },
];

const DARWIN_BASH_VARIANTS = [
  { name: "macos-15", minDarwin: 24 },
  { name: "macos-14", minDarwin: 23 },
  { name: "macos-13", minDarwin: 22 },
];

function resolveTargetTriple(platform, arch) {
  if (platform === "linux") {
    if (arch === "x64") {
      return "x86_64-unknown-linux-musl";
    }
    if (arch === "arm64") {
      return "aarch64-unknown-linux-musl";
    }
  } else if (platform === "darwin") {
    if (arch === "x64") {
      return "x86_64-apple-darwin";
    }
    if (arch === "arm64") {
      return "aarch64-apple-darwin";
    }
  }
  throw new Error(`Unsupported platform: ${platform} (${arch})`);
}

function parseOsRelease() {
  try {
    const contents = readFileSync("/etc/os-release", "utf8");
    const lines = contents.split("\n").filter(Boolean);
    const info = {};
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
  } catch {
    return { id: "", idLike: [], versionId: "" };
  }
}

function variantExists(bashRoot, name) {
  const candidate = path.join(bashRoot, name, "bash");
  return existsSync(candidate);
}

function listAvailableVariants(bashRoot) {
  try {
    return readdirSync(bashRoot, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => entry.name)
      .filter((name) => variantExists(bashRoot, name));
  } catch {
    return [];
  }
}

function selectLinuxBash(bashRoot) {
  const info = parseOsRelease();
  const versionId = info.versionId;
  const candidates = [];
  for (const variant of LINUX_BASH_VARIANTS) {
    const matchesId =
      variant.ids.includes(info.id) ||
      variant.ids.some((id) => info.idLike.includes(id));
    if (!matchesId) {
      continue;
    }
    const matchesVersion =
      versionId &&
      variant.versions.some((prefix) => versionId.startsWith(prefix));
    candidates.push({ variant, matchesVersion });
  }

  const pickVariant = (list) =>
    list.find(({ variant: candidate }) => variantExists(bashRoot, candidate.name))
      ?.variant;

  const preferred = pickVariant(candidates.filter((item) => item.matchesVersion));
  if (preferred) {
    return { path: path.join(bashRoot, preferred.name, "bash"), variant: preferred.name };
  }

  const fallbackMatch = pickVariant(candidates);
  if (fallbackMatch) {
    return { path: path.join(bashRoot, fallbackMatch.name, "bash"), variant: fallbackMatch.name };
  }

  const available = pickVariant(
    LINUX_BASH_VARIANTS.map((variant) => ({ variant, matchesVersion: false })),
  );
  if (available) {
    return { path: path.join(bashRoot, available.name, "bash"), variant: available.name };
  }

  const known = listAvailableVariants(bashRoot);
  const detail = known.length
    ? `Available variants: ${known.join(", ")}`
    : "No bundled Bash binaries were found.";
  throw new Error(
    `Unable to select a Bash variant for ${info.id || "unknown"} ${versionId || ""}. ${detail}`,
  );
}

function selectDarwinBash(bashRoot) {
  const darwinMajor = Number.parseInt(os.release().split(".")[0] || "0", 10);
  const pickVariant = (variantList) =>
    variantList.find((variant) => variantExists(bashRoot, variant.name));

  const preferred = pickVariant(
    DARWIN_BASH_VARIANTS.filter((variant) => darwinMajor >= variant.minDarwin),
  );
  if (preferred) {
    return { path: path.join(bashRoot, preferred.name, "bash"), variant: preferred.name };
  }

  const available = pickVariant(DARWIN_BASH_VARIANTS);
  if (available) {
    return { path: path.join(bashRoot, available.name, "bash"), variant: available.name };
  }

  const known = listAvailableVariants(bashRoot);
  const detail = known.length
    ? `Available variants: ${known.join(", ")}`
    : "No bundled Bash binaries were found.";
  throw new Error(`Unable to select a macOS Bash build (darwin ${darwinMajor}). ${detail}`);
}

function resolveBashPath(targetRoot) {
  const override = process.env.CODEX_BASH_PATH;
  if (override) {
    if (!existsSync(override)) {
      throw new Error(`CODEX_BASH_PATH was set to ${override}, but it does not exist.`);
    }
    return { path: override, variant: "env" };
  }

  const bashRoot = path.join(targetRoot, "bash");
  if (!existsSync(bashRoot)) {
    throw new Error(`Bundled Bash directory missing: ${bashRoot}`);
  }

  if (process.platform === "linux") {
    return selectLinuxBash(bashRoot);
  }
  if (process.platform === "darwin") {
    return selectDarwinBash(bashRoot);
  }
  throw new Error(`Unsupported platform for Bash selection: ${process.platform}`);
}

const ensurePathExists = (checkPath, label) => {
  if (!existsSync(checkPath)) {
    throw new Error(`Expected ${label} at ${checkPath}, but it was not found.`);
  }
};

const targetTriple = resolveTargetTriple(process.platform, process.arch);
const vendorRoot = path.join(__dirname, "..", "vendor");
const targetRoot = path.join(vendorRoot, targetTriple);
ensurePathExists(targetRoot, `vendor directory for ${targetTriple}`);

const execveWrapperPath = path.join(targetRoot, "codex-execve-wrapper");
ensurePathExists(execveWrapperPath, "execve wrapper");

const serverPath = path.join(targetRoot, "codex-exec-mcp-server");
ensurePathExists(serverPath, "codex-exec-mcp-server");

const { path: bashPath, variant: bashVariant } = resolveBashPath(targetRoot);

const childEnv = {
  ...process.env,
  CODEX_BASH_PATH: bashPath,
};
if (bashVariant) {
  childEnv.CODEX_BASH_VARIANT = bashVariant;
}

const args = ["--execve", execveWrapperPath, "--bash", bashPath, ...process.argv.slice(2)];
const child = spawn(serverPath, args, {
  stdio: "inherit",
  env: childEnv,
});

const forwardSignal = (signal) => {
  if (child.killed) {
    return;
  }
  try {
    child.kill(signal);
  } catch {
    /* ignore */
  }
};

["SIGINT", "SIGTERM", "SIGHUP"].forEach((sig) => {
  process.on(sig, () => forwardSignal(sig));
});

child.on("error", (err) => {
  // eslint-disable-next-line no-console
  console.error(err);
  process.exit(1);
});

const childResult = await new Promise((resolve) => {
  child.on("exit", (code, signal) => {
    if (signal) {
      resolve({ type: "signal", signal });
    } else {
      resolve({ type: "code", exitCode: code ?? 1 });
    }
  });
});

if (childResult.type === "signal") {
  // This environment running under `node --test` may not allow rethrowing a signal.
  // Wrap in a try to avoid masking the original termination reason.
  try {
    process.kill(process.pid, childResult.signal);
  } catch {
    process.exit(1);
  }
} else {
  process.exit(childResult.exitCode);
}
