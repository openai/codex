import * as child_process from "node:child_process";
import { EventEmitter } from "node:events";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { PassThrough } from "node:stream";

import { describe, expect, it } from "@jest/globals";

jest.mock("node:child_process", () => {
  const actual = jest.requireActual<typeof import("node:child_process")>("node:child_process");
  return { ...actual, spawn: jest.fn() };
});

const _actualChildProcess =
  jest.requireActual<typeof import("node:child_process")>("node:child_process");
const spawnMock = child_process.spawn as jest.MockedFunction<typeof _actualChildProcess.spawn>;

class FakeChildProcess extends EventEmitter {
  stdin = new PassThrough();
  stdout = new PassThrough();
  stderr = new PassThrough();
  killed = false;

  kill(): boolean {
    this.killed = true;
    return true;
  }
}

function createEarlyExitChild(exitCode = 2): FakeChildProcess {
  const child = new FakeChildProcess();
  setImmediate(() => {
    child.stderr.write("boom");
    child.emit("exit", exitCode, null);
    setImmediate(() => {
      child.stdout.end();
      child.stderr.end();
    });
  });
  return child;
}

const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

describe("CodexExec", () => {
  it("rejects when exit happens before stdout closes", async () => {
    const { CodexExec } = await import("../src/exec");
    const child = createEarlyExitChild();
    spawnMock.mockReturnValue(child as unknown as child_process.ChildProcess);

    const exec = new CodexExec("codex");
    const runPromise = (async () => {
      for await (const _ of exec.run({ input: "hi" })) {
        // no-op
      }
    })().then(
      () => ({ status: "resolved" as const }),
      (error) => ({ status: "rejected" as const, error }),
    );

    const result = await Promise.race([
      runPromise,
      delay(500).then(() => ({ status: "timeout" as const })),
    ]);

    expect(result.status).toBe("rejected");
    if (result.status === "rejected") {
      expect(result.error).toBeInstanceOf(Error);
      expect(result.error.message).toMatch(/Codex Exec exited/);
    }
  });

  it("places resume args before image args", async () => {
    const { CodexExec } = await import("../src/exec");
    spawnMock.mockClear();
    const child = new FakeChildProcess();
    spawnMock.mockReturnValue(child as unknown as child_process.ChildProcess);

    setImmediate(() => {
      child.stdout.end();
      child.stderr.end();
      child.emit("exit", 0, null);
    });

    const exec = new CodexExec("codex");
    for await (const _ of exec.run({ input: "hi", images: ["img.png"], threadId: "thread-id" })) {
      // no-op
    }

    const commandArgs = spawnMock.mock.calls[0]?.[1] as string[] | undefined;
    expect(commandArgs).toBeDefined();
    const resumeIndex = commandArgs!.indexOf("resume");
    const imageIndex = commandArgs!.indexOf("--image");
    expect(resumeIndex).toBeGreaterThan(-1);
    expect(imageIndex).toBeGreaterThan(-1);
    expect(resumeIndex).toBeLessThan(imageIndex);
  });

  it("allows overriding the env passed to the Codex CLI", async () => {
    const { CodexExec } = await import("../src/exec");
    spawnMock.mockClear();
    const child = new FakeChildProcess();
    spawnMock.mockReturnValue(child as unknown as child_process.ChildProcess);

    setImmediate(() => {
      child.stdout.end();
      child.stderr.end();
      child.emit("exit", 0, null);
    });

    process.env.CODEX_ENV_SHOULD_NOT_LEAK = "leak";

    try {
      const exec = new CodexExec("codex", {
        CODEX_HOME: "/tmp/codex-home",
        CUSTOM_ENV: "custom",
      });

      for await (const _ of exec.run({
        input: "custom env",
        apiKey: "test",
        baseUrl: "https://example.test",
      })) {
        // no-op
      }

      const commandArgs = spawnMock.mock.calls[0]?.[1] as string[] | undefined;
      expect(commandArgs).toBeDefined();
      const spawnOptions = spawnMock.mock.calls[0]?.[2] as child_process.SpawnOptions | undefined;
      const spawnEnv = spawnOptions?.env as Record<string, string> | undefined;
      expect(spawnEnv).toBeDefined();
      if (!spawnEnv || !commandArgs) {
        throw new Error("Spawn args missing");
      }

      expect(spawnEnv.CODEX_HOME).toBe("/tmp/codex-home");
      expect(spawnEnv.CUSTOM_ENV).toBe("custom");
      expect(spawnEnv.CODEX_ENV_SHOULD_NOT_LEAK).toBeUndefined();
      expect(spawnEnv.CODEX_API_KEY).toBe("test");
      expect(spawnEnv.CODEX_INTERNAL_ORIGINATOR_OVERRIDE).toBeDefined();
      expect(commandArgs).toContain("--config");
      expect(commandArgs).toContain(`openai_base_url=${JSON.stringify("https://example.test")}`);
    } finally {
      delete process.env.CODEX_ENV_SHOULD_NOT_LEAK;
    }
  });

  it("resolves the package-layout binary and PATH directory", async () => {
    const { resolveNativePackage } = await import("../src/exec");
    const vendorRoot = mkdtempSync(path.join(tmpdir(), "codex-sdk-vendor-"));
    const packageRoot = path.join(vendorRoot, "x86_64-unknown-linux-musl");
    const binDir = path.join(packageRoot, "bin");
    const pathDir = path.join(packageRoot, "codex-path");
    mkdirSync(binDir, { recursive: true });
    mkdirSync(pathDir, { recursive: true });
    writeFileSync(path.join(packageRoot, "codex-package.json"), "{}");
    writeFileSync(path.join(binDir, "codex"), "");

    expect(resolveNativePackage(vendorRoot, "x86_64-unknown-linux-musl", "codex")).toEqual({
      executablePath: path.join(binDir, "codex"),
      pathDirs: [pathDir],
    });
  });

  it("falls back to the legacy binary layout", async () => {
    const { resolveNativePackage } = await import("../src/exec");
    const vendorRoot = mkdtempSync(path.join(tmpdir(), "codex-sdk-vendor-"));
    const packageRoot = path.join(vendorRoot, "x86_64-unknown-linux-musl");
    const binDir = path.join(packageRoot, "codex");
    const pathDir = path.join(packageRoot, "path");
    mkdirSync(binDir, { recursive: true });
    mkdirSync(pathDir, { recursive: true });
    writeFileSync(path.join(binDir, "codex"), "");

    expect(resolveNativePackage(vendorRoot, "x86_64-unknown-linux-musl", "codex")).toEqual({
      executablePath: path.join(binDir, "codex"),
      pathDirs: [pathDir],
    });
  });

  it("prepends package PATH entries without duplicating them", async () => {
    const { prependPathDirs } = await import("../src/exec");
    const pathDir = path.join(tmpdir(), "codex-path");
    const env = { PATH: `/usr/bin${path.delimiter}${pathDir}` };

    prependPathDirs(env, [pathDir]);

    expect(env).toEqual({ PATH: `${pathDir}${path.delimiter}/usr/bin` });
  });

  it("preserves the Windows Path key when prepending package PATH entries", async () => {
    const { prependPathDirs } = await import("../src/exec");
    const pathDir = path.join(tmpdir(), "codex-path");
    const env = { PATH: "/usr/bin", Path: `C\\Windows${path.delimiter}${pathDir}` };

    prependPathDirs(env, [pathDir], "win32");

    expect(env).toEqual({ Path: `${pathDir}${path.delimiter}C\\Windows` });
  });
});
