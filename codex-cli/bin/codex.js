#!/usr/bin/env node
// Unified entry point for the Codex CLI.
/*
 * Behavior
 * =========
 *   1. By default we import the JavaScript implementation located in
 *      dist/cli.js.
 *
 *   2. Developers can opt-in to a pre-compiled Rust binary by setting the
 *      environment variable CODEX_RUST to a truthy value (`1`, `true`, etc.).
 *      When that variable is present we resolve the correct binary for the
 *      current platform / architecture and execute it via child_process.
 *
 *      If the CODEX_RUST=1 is specified and there is no native binary for the
 *      current platform / architecture, an error is thrown.
 */

import fs from "fs";
import path from "path";
import { fileURLToPath, pathToFileURL } from "url";

// Determine whether the user explicitly wants the Rust CLI.

// __dirname equivalent in ESM
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// For the @native release of the Node module, the `use-native` file is added,
// indicating we should default to the native binary. For other releases,
// setting CODEX_RUST=1 will opt-in to the native binary, if included.
const wantsNative = fs.existsSync(path.join(__dirname, "use-native")) ||
  (process.env.CODEX_RUST != null
    ? ["1", "true", "yes"].includes(process.env.CODEX_RUST.toLowerCase())
    : false);

// Try native binary if requested.
if (wantsNative && process.platform !== 'win32') {
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
    default:
      break;
  }

  if (!targetTriple) {
    throw new Error(`Unsupported platform: ${platform} (${arch})`);
  }

  const binaryPath = path.join(__dirname, "..", "bin", `codex-${targetTriple}`);

  // Use an asynchronous spawn instead of spawnSync so that Node is able to
  // respond to signals (e.g. Ctrl-C / SIGINT) while the native binary is
  // executing. This allows us to forward those signals to the child process
  // and guarantees that when either the child terminates or the parent
  // receives a fatal signal, both processes exit in a predictable manner.
  const { spawn } = await import("child_process");

  const child = spawn(binaryPath, process.argv.slice(2), {
    stdio: "inherit",
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
} else {
  // Fallback: execute the original JavaScript CLI.

  // Resolve the path to the compiled CLI bundle
  const cliPath = path.resolve(__dirname, "../dist/cli.js");
  const cliUrl = pathToFileURL(cliPath).href;

  // Load and execute the CLI
  try {
    await import(cliUrl);
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error(err);
    process.exit(1);
  }
}
