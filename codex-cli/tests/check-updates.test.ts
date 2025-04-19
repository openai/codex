import { describe, it, expect, vi } from "vitest";
import {
  checkForUpdates,
  checkOutdated,
  getNPMCommandPath,
} from "../src/utils/check-updates.js";
import { execFile } from "node:child_process";
import { join } from "node:path";
import { CONFIG_DIR } from "src/utils/config.js";
import { beforeEach } from "node:test";

vi.mock("which", () => ({
  default: vi.fn(() => "/usr/local/bin/npm"),
}));

vi.mock("child_process", () => ({
  execFile: vi.fn((_cmd, _args, _opts, callback) => {
    const stdout = JSON.stringify({
      "@openai/codex": {
        current: "1.0.0",
        latest: "2.0.0",
      },
    });
    callback?.(null, stdout, "");
    return {} as any;
  }),
}));

let memfs: Record<string, string> = {};

vi.mock("node:fs/promises", async (importOriginal) => ({
  ...(await importOriginal()),
  readFile: async (path: string) => {
    if (memfs[path] === undefined) {
      throw new Error("ENOENT");
    }
    return memfs[path];
  },
}));

beforeEach(() => {
  memfs = {}; // reset in‑memory store
});

describe("Check for updates", () => {
  it("should return the path to npm", async () => {
    const npmPath = await getNPMCommandPath();
    expect(npmPath).toBeDefined();
  });

  it("should return undefined if npm is not found", async () => {
    vi.mocked(await import("which")).default.mockImplementationOnce(() => {
      throw new Error("not found");
    });

    const npmPath = await getNPMCommandPath();
    expect(npmPath).toBeUndefined();
  });

  it("should return the return value when package is outdated", async () => {
    const npmPath = await getNPMCommandPath();

    const info = await checkOutdated(npmPath!);
    expect(info).toStrictEqual({
      currentVersion: "1.0.0",
      latestVersion: "2.0.0",
    });
  });

  it("should return undefined when package is not outdated", async () => {
    const npmPath = await getNPMCommandPath();
    vi.mocked(execFile).mockImplementationOnce(
      (_cmd, _args, _opts, callback) => {
        // Simulate the case where the package is not outdated, returning an empty object
        const stdout = JSON.stringify({});
        callback?.(null, stdout, "");
        return {} as any;
      },
    );

    const info = await checkOutdated(npmPath!);
    expect(info).toBeUndefined();
  });

  it("should outputs the update message when package is outdated", async () => {
    const codexStatePath = join(CONFIG_DIR, "update-check.json");
    // Use a fixed early date far in the past to ensure it's always at least 1 day before now
    memfs[codexStatePath] = JSON.stringify({
      lastUpdateCheck: new Date("2000-01-01T00:00:00Z").toUTCString(),
    });
    await checkForUpdates();
    // Spy on console.log to capture output
    const logSpy = vi.spyOn(console, "log").mockImplementation(() => {});
    await checkForUpdates();
    expect(logSpy).toHaveBeenCalled();
    // The last call should be the boxen message
    const lastCallArg = logSpy.mock.calls.at(-1)?.[0];
    expect(lastCallArg).toMatchSnapshot();
  });

  it("should not output the update message when package is not outdated", async () => {
    const codexStatePath = join(CONFIG_DIR, "update-check.json");
    memfs[codexStatePath] = JSON.stringify({
      lastUpdateCheck: new Date().toUTCString(),
    });
    await checkForUpdates();
    // Spy on console.log to capture output
    const logSpy = vi.spyOn(console, "log").mockImplementation(() => {});
    await checkForUpdates();
    expect(logSpy).not.toHaveBeenCalled();
  });
});
