import assert from "node:assert/strict";
import test from "node:test";

import { sanitizeMacosMallocDiagnosticEnv } from "../bin/sanitize-macos-malloc-env.js";

test("removes macOS malloc diagnostic env vars on darwin", () => {
  const env = {
    MallocStackLogging: "0",
    MallocStackLoggingDirectory: "/tmp/stack-logs",
    MallocLogFile: "/tmp/malloc.log",
    MallocNanoZone: "0",
    PATH: "/usr/bin",
  };

  sanitizeMacosMallocDiagnosticEnv(env, "darwin");

  assert.deepEqual(env, {
    MallocNanoZone: "0",
    PATH: "/usr/bin",
  });
});

test("leaves env unchanged off darwin", () => {
  const env = {
    MallocStackLogging: "0",
    PATH: "/usr/bin",
  };

  sanitizeMacosMallocDiagnosticEnv(env, "linux");

  assert.deepEqual(env, {
    MallocStackLogging: "0",
    PATH: "/usr/bin",
  });
});
