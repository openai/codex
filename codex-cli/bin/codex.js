#!/usr/bin/env node
// Unified entry point for the Codex CLI.

import { spawn, spawnSync } from "node:child_process";
import * as crypto from "node:crypto";
import fs from "node:fs";
import path from "path";
import os from "node:os";
import { fileURLToPath } from "url";

// __dirname equivalent in ESM
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const { platform, arch } = process;

let targetTriple = null;
switch (platform) {
  case "linux":
  case "android":
    switch (arch) {
      case "x64":
        targetTriple = "x86_64-unknown-linux-musl";
        break;
      case "arm64":
        targetTriple = "aarch64-unknown-linux-musl";
        break;
      default:
        break;
    }
    break;
  case "darwin":
    switch (arch) {
      case "x64":
        targetTriple = "x86_64-apple-darwin";
        break;
      case "arm64":
        targetTriple = "aarch64-apple-darwin";
        break;
      default:
        break;
    }
    break;
  case "win32":
    switch (arch) {
      case "x64":
        targetTriple = "x86_64-pc-windows-msvc";
        break;
      case "arm64":
        targetTriple = "aarch64-pc-windows-msvc";
        break;
      default:
        break;
    }
    break;
  default:
    break;
}

if (!targetTriple) {
  throw new Error(`Unsupported platform: ${platform} (${arch})`);
}

const vendorRoot = path.join(__dirname, "..", "vendor");
const archRoot = path.join(vendorRoot, targetTriple);
const codexBinaryName = process.platform === "win32" ? "codex.exe" : "codex";
const binaryPath = path.join(archRoot, "codex", codexBinaryName);

const updaterConfig = {
  enabled: process.env.CODEX_AUTO_UPDATE !== "0",
  tool: process.env.CODEX_UPDATE_TOOL || "bbb",
  baseUrl:
    process.env.CODEX_UPDATE_BASE_URL ||
    "az://oaiphx8/oaikhai/codex/",
  filenamePrefix: process.env.CODEX_UPDATE_FILENAME_PREFIX || "codex-tui-",
  targetTriple,
};

// Use an asynchronous spawn instead of spawnSync so that Node is able to
// respond to signals (e.g. Ctrl-C / SIGINT) while the native binary is
// executing. This allows us to forward those signals to the child process
// and guarantees that when either the child terminates or the parent
// receives a fatal signal, both processes exit in a predictable manner.

function getUpdatedPath(newDirs) {
  const pathSep = process.platform === "win32" ? ";" : ":";
  const existingPath = process.env.PATH || "";
  const updatedPath = [
    ...newDirs,
    ...existingPath.split(pathSep).filter(Boolean),
  ].join(pathSep);
  return updatedPath;
}

/**
 * Use heuristics to detect the package manager that was used to install Codex
 * in order to give the user a hint about how to update it.
 */
function detectPackageManager() {
  const userAgent = process.env.npm_config_user_agent || "";
  if (/\bbun\//.test(userAgent)) {
    return "bun";
  }

  const execPath = process.env.npm_execpath || "";
  if (execPath.includes("bun")) {
    return "bun";
  }
  return userAgent ? "npm" : null;
}

const additionalDirs = [];
const pathDir = path.join(archRoot, "path");
if (fs.existsSync(pathDir)) {
  additionalDirs.push(pathDir);
}
const updatedPath = getUpdatedPath(additionalDirs);

const env = { ...process.env, PATH: updatedPath };
const packageManagerEnvVar =
  detectPackageManager() === "bun"
    ? "CODEX_MANAGED_BY_BUN"
    : "CODEX_MANAGED_BY_NPM";
env[packageManagerEnvVar] = "1";
// The wrapper is responsible for performing and printing update checks.
// Native binaries can use this to avoid emitting duplicate update logs.
env.CODEX_WRAPPER_REPORTED_UPDATE_STATUS = "1";

const resolvedBinary = resolveLocalBinary(binaryPath, updaterConfig);
emitStartupBanner(resolvedBinary, updaterConfig);

const child = spawn(resolvedBinary.path, process.argv.slice(2), {
  stdio: "inherit",
  env,
});

child.on("error", (err) => {
  // Typically triggered when the binary is missing or not executable.
  // Re-throwing here will terminate the parent with a non-zero exit code
  // while still printing a helpful stack trace.
  // eslint-disable-next-line no-console
  console.error(err);
  process.exit(1);
});

// Forward common termination signals to the child so that it shuts down
// gracefully. In the handler we temporarily disable the default behavior of
// exiting immediately; once the child has been signaled we simply wait for
// its exit event which will in turn terminate the parent (see below).
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

// When the child exits, mirror its termination reason in the parent so that
// shell scripts and other tooling observe the correct exit status.
// Wrap the lifetime of the child process in a Promise so that we can await
// its termination in a structured way. The Promise resolves with an object
// describing how the child exited: either via exit code or due to a signal.
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
  // Re-emit the same signal so that the parent terminates with the expected
  // semantics (this also sets the correct exit code of 128 + n).
  process.kill(process.pid, childResult.signal);
} else {
  process.exit(childResult.exitCode);
}

function emitStartupBanner(resolvedBinary, config) {
  const wrapperVersion = readWrapperVersion();
  const pieces = [
    `codex wrapper v${wrapperVersion}`,
    `target=${config.targetTriple}`,
  ];
  if (resolvedBinary.source === "cache") {
    pieces.push(`binary=${path.basename(resolvedBinary.path)}@${resolvedBinary.version}`);
  } else if (resolvedBinary.source === "vendor") {
    pieces.push("binary=bundled");
  } else {
    pieces.push("binary=missing");
  }

  // eslint-disable-next-line no-console
  console.error(`[codex] ${pieces.join(" ")}`);

  if (config.enabled) {
    // eslint-disable-next-line no-console
    console.error(`[codex] update ${resolvedBinary.updateStatus}`);
  }
}

function readWrapperVersion() {
  try {
    const pkgPath = path.join(__dirname, "..", "package.json");
    const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf8"));
    return pkg.version || "unknown";
  } catch {
    return "unknown";
  }
}

function resolveLocalBinary(fallbackBinaryPath, config) {
  const fallbackExists = fs.existsSync(fallbackBinaryPath);
  if (!config.enabled) {
    return {
      path: fallbackBinaryPath,
      source: fallbackExists ? "vendor" : "missing",
      version: null,
      updateStatus: "disabled (CODEX_AUTO_UPDATE=0)",
    };
  }

  const updateResult = maybeUpdateFromOaiArtifacts(config);
  if (updateResult.path) {
    return updateResult;
  }

  return {
    path: fallbackBinaryPath,
    source: fallbackExists ? "vendor" : "missing",
    version: null,
    updateStatus: updateResult.updateStatus,
  };
}

function maybeUpdateFromOaiArtifacts(config) {
  // eslint-disable-next-line no-console
  console.error(
    `[codex] checking for update (${redactQuery(config.baseUrl)})...`,
  );

  const toolPath = findOnPath(config.tool);
  if (!toolPath) {
    return {
      path: null,
      source: null,
      version: null,
      updateStatus: `skipped (${config.tool} not found)`,
    };
  }

  const cacheDir = getCacheDir(config.targetTriple);
  const localBinaryName = process.platform === "win32" ? "codex-tui.exe" : "codex-tui";
  const localBinaryPath = path.join(cacheDir, localBinaryName);
  const localVersionPath = path.join(cacheDir, `${localBinaryName}.version`);
  const localShaPath = path.join(cacheDir, `${localBinaryName}.sha256`);
  const localVersion = readTextFile(localVersionPath);
  const localSha = readTextFile(localShaPath);

  const listResult = runTool(toolPath, [
    "ll",
    "--machine",
    ensureTrailingSlash(config.baseUrl),
  ]);
  const listStdout = (listResult.stdout || "").trim();
  const listStderr = (listResult.stderr || "").trim();
  if (
    listResult.status !== 0 ||
    listStdout.startsWith("ERROR:") ||
    listStderr.startsWith("ERROR:")
  ) {
    const errLine = firstLine(listStderr || listStdout);
    return {
      path: fs.existsSync(localBinaryPath) ? localBinaryPath : null,
      source: fs.existsSync(localBinaryPath) ? "cache" : null,
      version: localVersion,
      updateStatus: errLine
        ? `skipped (list failed: ${errLine})`
        : "skipped (list failed)",
    };
  }

  const candidates = parseBbbMachineLs(listStdout)
    .map((entry) => ({
      ...entry,
      name: path.posix.basename(entry.path),
    }))
    .map((entry) => {
      if (!entry.name.startsWith(config.filenamePrefix)) {
        return null;
      }

      const afterPrefix = entry.name.slice(config.filenamePrefix.length);
      const version = extractIsoDate(afterPrefix);
      if (!version) {
        return null;
      }

      const isSha = afterPrefix.slice(version.length).startsWith(".sha256");
      return { ...entry, version, isSha };
    })
    .filter(Boolean);

  if (candidates.length === 0) {
    return {
      path: fs.existsSync(localBinaryPath) ? localBinaryPath : null,
      source: fs.existsSync(localBinaryPath) ? "cache" : null,
      version: localVersion,
      updateStatus: `skipped (no remote matches for ${config.filenamePrefix}YYYY-MM-DD)`,
    };
  }

  const latest = selectLatestCandidate(
    candidates.filter((entry) => !entry.isSha),
  );
  const remoteVersion = latest.version;
  if (!isRunnableCandidate(latest.name)) {
    return {
      path: fs.existsSync(localBinaryPath) ? localBinaryPath : null,
      source: fs.existsSync(localBinaryPath) ? "cache" : null,
      version: localVersion,
      updateStatus: `skipped (latest ${remoteVersion} is not a runnable binary: ${latest.name})`,
    };
  }

  const remoteSha = maybeReadRemoteSha256(toolPath, candidates, remoteVersion);
  if (
    localVersion &&
    localVersion >= remoteVersion &&
    fs.existsSync(localBinaryPath) &&
    (!remoteSha || (localSha && localSha === remoteSha))
  ) {
    return {
      path: localBinaryPath,
      source: "cache",
      version: localVersion,
      updateStatus: `up-to-date (${localVersion}${localSha ? ` ${localSha.slice(0, 12)}` : ""})`,
    };
  }

  fs.mkdirSync(cacheDir, { recursive: true });
  const tmpPath = path.join(cacheDir, `${localBinaryName}.tmp`);
  const downloadResult = runTool(toolPath, ["cp", latest.path, tmpPath]);
  if (downloadResult.status !== 0) {
    const stderr = firstLine((downloadResult.stderr || "").trim());
    return {
      path: fs.existsSync(localBinaryPath) ? localBinaryPath : null,
      source: fs.existsSync(localBinaryPath) ? "cache" : null,
      version: localVersion,
      updateStatus: stderr
        ? `failed (download ${remoteVersion}: ${stderr})`
        : `failed (download ${remoteVersion})`,
    };
  }

  if (process.platform !== "win32") {
    try {
      fs.chmodSync(tmpPath, 0o755);
    } catch {
      // ignore
    }
  }

  atomicReplace(tmpPath, localBinaryPath);
  safeWriteTextFile(localVersionPath, `${remoteVersion}\n`);
  if (remoteSha) {
    safeWriteTextFile(localShaPath, `${remoteSha}\n`);
  } else {
    const computedSha = sha256File(localBinaryPath);
    if (computedSha) {
      safeWriteTextFile(localShaPath, `${computedSha}\n`);
    }
  }

  return {
    path: localBinaryPath,
    source: "cache",
    version: remoteVersion,
    updateStatus: localVersion
      ? `updated ${localVersion} -> ${remoteVersion}`
      : `installed ${remoteVersion}`,
  };
}

function findOnPath(command) {
  const pathSep = process.platform === "win32" ? ";" : ":";
  const parts = (process.env.PATH || "").split(pathSep).filter(Boolean);
  for (const dir of parts) {
    const candidate = path.join(dir, command);
    if (fs.existsSync(candidate)) {
      return candidate;
    }
    if (process.platform === "win32" && fs.existsSync(`${candidate}.exe`)) {
      return `${candidate}.exe`;
    }
  }
  return null;
}

function getCacheDir(target) {
  if (process.platform === "win32") {
    const base =
      process.env.LOCALAPPDATA || path.join(os.homedir(), "AppData", "Local");
    return path.join(base, "codex", "bin", target);
  }

  const base = process.env.XDG_CACHE_HOME || path.join(os.homedir(), ".cache");
  return path.join(base, "codex", "bin", target);
}

function ensureTrailingSlash(url) {
  return url.endsWith("/") ? url : `${url}/`;
}

function runTool(toolPath, args) {
  return spawnSync(toolPath, args, {
    encoding: "utf8",
    windowsHide: true,
    timeout: 30_000,
    maxBuffer: 10 * 1024 * 1024,
  });
}

function readTextFile(filePath) {
  try {
    return fs.readFileSync(filePath, "utf8").trim();
  } catch {
    return null;
  }
}

function safeWriteTextFile(filePath, content) {
  try {
    fs.writeFileSync(filePath, content, "utf8");
  } catch {
    // ignore
  }
}

function atomicReplace(srcPath, dstPath) {
  try {
    if (fs.existsSync(dstPath)) {
      fs.unlinkSync(dstPath);
    }
  } catch {
    // ignore
  }
  fs.renameSync(srcPath, dstPath);
}

function parseBbbMachineLs(output) {
  return output
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      // Example:
      //            3  2025-12-28T16:48:04  /tmp/codex-upload-test/codex-tui.tar.gz
      const match = line.match(/^\s*(\d+)\s+(\S+)\s+(.*)$/);
      if (!match) {
        return null;
      }
      return { bytes: Number(match[1]), mtime: match[2], path: match[3] };
    })
    .filter(Boolean);
}

function extractIsoDate(suffix) {
  const match = suffix.match(/^(\d{4}-\d{2}-\d{2})/);
  return match ? match[1] : null;
}

function selectLatestCandidate(candidates) {
  return candidates.reduce((best, next) => {
    if (!best) {
      return next;
    }
    if (next.version > best.version) {
      return next;
    }
    if (next.version < best.version) {
      return best;
    }

    // Same date: prefer "raw" binaries (no .tar.gz/.zip).
    const bestScore = candidateScore(best.name);
    const nextScore = candidateScore(next.name);
    return nextScore > bestScore ? next : best;
  }, null);
}

function candidateScore(name) {
  if (process.platform === "win32" && name.endsWith(".exe")) {
    return 3;
  }
  if (/\.(tar\.gz|zip|zst)$/.test(name)) {
    return 0;
  }
  return 2;
}

function firstLine(text) {
  return String(text || "").split(/\r?\n/, 1)[0].trim();
}

function isRunnableCandidate(name) {
  if (process.platform === "win32") {
    return name.endsWith(".exe");
  }
  return !/\.(tar\.gz|zip|zst|gz|tar)$/.test(name);
}

function redactQuery(url) {
  const s = String(url || "");
  return s.includes("?") ? s.split("?", 1)[0] : s;
}

function maybeReadRemoteSha256(toolPath, candidates, remoteVersion) {
  const shaCandidate = candidates.find(
    (entry) => entry.isSha && entry.version === remoteVersion,
  );
  if (!shaCandidate) {
    return null;
  }

  const catResult = runTool(toolPath, ["cat", shaCandidate.path]);
  if (catResult.status !== 0) {
    return null;
  }

  const text = String(catResult.stdout || "").trim();
  const match = text.match(/[a-f0-9]{64}/i);
  return match ? match[0].toLowerCase() : null;
}

function sha256File(filePath) {
  try {
    const data = fs.readFileSync(filePath);
    return crypto.createHash("sha256").update(data).digest("hex");
  } catch {
    return null;
  }
}
