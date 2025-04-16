/*
 * Vitest setup helper that patches `fs.mkdtempSync` so it works inside
 * `worker_threads` on macOS where creating new directories inside the default
 * temporary folder fails with `EPERM`. We fall back to creating the temporary
 * directory inside the current working directory which is writable in the
 * sandboxed test environment.
 *
 * The file name is prefixed with `00-` so it executes before the rest of the
 * test suite, ensuring the patched implementation is in place for subsequent
 * tests.
 */

import fs from "fs";
import path from "path";

const originalMkdtempSync = fs.mkdtempSync.bind(fs);

// Override with a resilient variant.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
(fs as any).mkdtempSync = (prefix: string, ...args: Array<any>) => {
  try {
    return originalMkdtempSync(prefix, ...args);
  } catch (err: any) {
    if (err?.code !== "EPERM") {
      throw err;
    }

    // Fallback path inside the repository that is always writable.
    const safePrefix = prefix.replace(/.*[\\/]/, ""); // strip any tmp dir path
    const fallbackDir = path.join(
      process.cwd(),
      `${safePrefix}${Date.now().toString(36)}${Math.random()
        .toString(36)
        .slice(2, 8)}`,
    );
    fs.mkdirSync(fallbackDir, { recursive: true });
    return fallbackDir;
  }
};

// Dummy test so Vitest recognizes the file as a proper test module.
import { it } from "vitest";

it("fs patch applied", () => {
  /* no‑op */
});
