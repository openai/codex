// Run with CODEX_TEST_BINARY=/absolute/path/to/codex node --test codex-cli/tests/exit-status.test.mjs.
import assert from "node:assert/strict";
import { execFileSync, spawn, spawnSync } from "node:child_process";
import { once } from "node:events";
import {
  copyFile,
  mkdir,
  mkdtemp,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import { constants, tmpdir } from "node:os";
import path from "node:path";
import { createInterface } from "node:readline";
import test from "node:test";

test(
  "the npm wrapper preserves numeric exits and child signals",
  {
    skip: process.platform === "win32",
  },
  async () => {
    assert.ok(
      process.env.CODEX_TEST_BINARY,
      "Set CODEX_TEST_BINARY to a built Codex binary",
    );
    const directory = await mkdtemp(path.join(tmpdir(), "codex-npm-exit-"));
    let child;
    let killTimer;
    try {
      const arch = process.arch === "arm64" ? "aarch64" : "x86_64";
      const target = `${arch}-${process.platform === "darwin" ? "apple-darwin" : "unknown-linux-musl"}`;
      const bin = path.join(directory, "vendor", target, "bin");
      const home = path.join(directory, "home");
      await mkdir(bin, { recursive: true });
      await mkdir(home);
      await mkdir(path.join(directory, "bin"));
      await writeFile(
        path.join(directory, "package.json"),
        '{"type":"module"}',
      );
      const wrapper = path.join(directory, "bin", "codex.js");
      await copyFile(new URL("../bin/codex.js", import.meta.url), wrapper);
      await symlink(
        path.resolve(process.env.CODEX_TEST_BINARY),
        path.join(bin, "codex"),
      );
      const env = { ...process.env, CODEX_HOME: home, CODEX_SQLITE_HOME: home };

      // A normal CLI argument error must retain its numeric exit status.
      const invalid = spawnSync(
        process.execPath,
        [wrapper, "--invalid-codex-option"],
        { env, timeout: 30_000, killSignal: "SIGKILL" },
      );
      assert.equal(invalid.error, undefined);
      assert.deepEqual([invalid.status, invalid.signal], [2, null]);

      for (const signal of ["SIGHUP", "SIGXFSZ"]) {
        // Wait for the real migration prompt before terminating a process.
        child = spawn(process.execPath, [wrapper, "migrate-nfs-db"], {
          env,
          detached: true,
        });
        const processGroup = child.pid;
        killTimer = setTimeout(() => {
          try {
            process.kill(-processGroup, "SIGKILL");
          } catch (error) {
            if (error.code !== "ESRCH") throw error;
          }
        }, 30_000);
        const exited = once(child, "exit");
        const lines = createInterface({ input: child.stderr });
        let prompted = false;
        for await (const line of lines) {
          if (line.endsWith("[y/N]:")) {
            prompted = true;
            break;
          }
        }
        assert.ok(prompted, "migration must reach its confirmation prompt");
        const processes = execFileSync("ps", ["-axo", "pid=,ppid="], {
          encoding: "utf8",
        });
        const children = processes
          .trim()
          .split("\n")
          .map((line) => line.trim().split(/\s+/).map(Number))
          .filter(([, parent]) => parent === child.pid);
        const usesExecve = typeof process.execve === "function";
        assert.equal(
          children.length,
          usesExecve ? 0 : 1,
          "execve must replace the wrapper; spawn must create one native child",
        );
        if (usesExecve || signal === "SIGHUP") {
          // Signal the native process directly, or let the wrapper forward it.
          child.kill(signal);
          assert.deepEqual(await exited, [null, signal]);
        } else {
          // A native child can die from a signal that Node ignores by default.
          process.kill(children[0][0], signal);
          assert.deepEqual(await exited, [
            128 + constants.signals[signal],
            null,
          ]);
        }
        clearTimeout(killTimer);
      }
    } finally {
      clearTimeout(killTimer);
      if (child && child.exitCode === null && child.signalCode === null) {
        process.kill(-child.pid, "SIGKILL");
        await once(child, "exit");
      }
      await rm(directory, { recursive: true, force: true });
    }
  },
);
