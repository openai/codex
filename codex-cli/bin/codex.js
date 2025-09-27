#!/usr/bin/env node
// Unified entry point for the Codex CLI.
// -------------------------------------------------------
// This script acts as a universal wrapper that initializes
// and runs the native Codex binary, adjusting PATHs,
// detecting platform/architecture, and forwarding signals.
// -------------------------------------------------------

import { existsSync } from "fs";        // Checks if files/directories exist
import path from "path";                // Cross-platform path utilities
import { fileURLToPath } from "url";    // Converts URL to local path (needed in ES modules)

// -------------------------------------------------------
// In ESM (ECMAScript Modules) __dirname and __filename
// do not exist. They are recreated here manually.
// -------------------------------------------------------
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// -------------------------------------------------------
// Detects the current operating system and CPU architecture.
// process.platform → "linux", "darwin", "win32", etc.
// process.arch     → "x64", "arm64", etc.
// -------------------------------------------------------
const { platform, arch } = process;

let targetTriple = null; // target triple = OS/CPU combo to locate the binary
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
  case "darwin": // macOS
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
  case "win32": // Windows
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

// -------------------------------------------------------
// If no supported target triple was found → throw error.
// -------------------------------------------------------
if (!targetTriple) {
  throw new Error(`Unsupported platform: ${platform} (${arch})`);
}

// -------------------------------------------------------
// Build paths to locate the native Codex binary:
// vendorRoot  → base "vendor" directory
// archRoot    → subfolder for platform/architecture
// binaryPath  → full path to executable
// -------------------------------------------------------
const vendorRoot = path.join(__dirname, "..", "vendor");
const archRoot = path.join(vendorRoot, targetTriple);
const codexBinaryName = process.platform === "win32" ? "codex.exe" : "codex";
const binaryPath = path.join(archRoot, "codex", codexBinaryName);

// -------------------------------------------------------
// Use spawn (async) instead of spawnSync so that:
// - Parent process still responds to OS signals (Ctrl+C).
// - Signals can be forwarded to the child process.
// - Both processes terminate predictably and consistently.
// -------------------------------------------------------
const { spawn } = await import("child_process");

/**
 * getUpdatedPath(newDirs)
 * -------------------------------------------------------
 * Returns an updated PATH environment variable string.
 *
 * Logical:
 *   - Splits current PATH into a list.
 *   - Removes empty entries.
 *   - Prepends new directories (higher priority).
 *
 * Electronic:
 *   - The OS resolves executables by scanning PATH entries
 *     in order. Modifying PATH changes how the kernel’s
 *     program loader finds binaries.
 */
function getUpdatedPath(newDirs) {
  const pathSep = process.platform === "win32" ? ";" : ":";
  const existingPath = process.env.PATH || "";
  const updatedPath = [
    ...newDirs,
    ...existingPath.split(pathSep).filter(Boolean),
  ].join(pathSep);
  return updatedPath;
}

// -------------------------------------------------------
// Check if "path" directory exists inside archRoot.
// If yes, add it to the PATH update list.
// -------------------------------------------------------
const additionalDirs = [];
const pathDir = path.join(archRoot, "path");
if (existsSync(pathDir)) {
  additionalDirs.push(pathDir);
}
const updatedPath = getUpdatedPath(additionalDirs);

// -------------------------------------------------------
// Spawn the child process that runs the native Codex binary.
// - stdio "inherit": child shares stdout/stderr with parent.
// - env: copies parent’s env and injects updated PATH.
// - CODEX_MANAGED_BY_NPM: flag signaling managed execution.
// -------------------------------------------------------
const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  env: { ...process.env, PATH: updatedPath, CODEX_MANAGED_BY_NPM: "1" },
});

// -------------------------------------------------------
// Handle spawn errors (usually binary missing or not executable).
// -------------------------------------------------------
child.on("error", (err) => {
  // Print error (stack trace) and exit with non-zero code
  console.error(err);
  process.exit(1);
});

/**
 * forwardSignal(signal)
 * -------------------------------------------------------
 * Forwards termination signals from parent → child process.
 *
 * Logical:
 *   - Prevents child process from becoming a zombie.
 *   - Ensures symmetry: if parent receives SIGTERM, child does too.
 *
 * Electronic:
 *   - Signals (SIGINT, SIGTERM, etc.) are OS-level interrupts.
 *   - Here we intercept them in the parent and re-send to child PID.
 */
const forwardSignal = (signal) => {
  if (child.killed) {
    return;
  }
  try {
    child.kill(signal); // re-send signal to child process
  } catch {
    /* ignore errors if child has already exited */
  }
};

// -------------------------------------------------------
// Forward common signals (Ctrl+C, kill, hangup) to the child.
// -------------------------------------------------------
["SIGINT", "SIGTERM", "SIGHUP"].forEach((sig) => {
  process.on(sig, () => forwardSignal(sig));
});

// -------------------------------------------------------
// Wait for child process termination inside a Promise.
// Captures exit reason as either:
//   - exitCode (normal termination).
//   - signal (terminated by signal).
// -------------------------------------------------------
const childResult = await new Promise((resolve) => {
  child.on("exit", (code, signal) => {
    if (signal) {
      resolve({ type: "signal", signal });
    } else {
      resolve({ type: "code", exitCode: code ?? 1 });
    }
  });
});

// -------------------------------------------------------
// Mirror the child’s termination reason in the parent.
// - If child exited by signal → re-emit same signal to parent.
//   (ensures correct shell semantics, exit code = 128 + n).
// - Otherwise → exit with child’s exit code.
// -------------------------------------------------------
if (childResult.type === "signal") {
  process.kill(process.pid, childResult.signal);
} else {
  process.exit(childResult.exitCode);
}
