import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { join } from "node:path";
import os from "node:os";
import type { UpdateOptions } from "../src/utils/check-updates";
import { getLatestVersion } from "fast-npm-meta";
import { getUserAgent } from "package-manager-detector";
import {
  checkForUpdates,
  renderUpdateCommand,
} from "../src/utils/check-updates";
import { detectInstallerByPath } from "../src/utils/package-manager-detector";
import { CLI_VERSION } from "../src/version";

// In-memory FS mock
let memfs: Record<string, string> = {};
vi.mock("node:fs/promises", async (importOriginal) => {
  return {
    ...(await importOriginal()),
    readFile: async (path: string) => {
      if (!(path in memfs)) {
        const err: any = new Error(
          `ENOENT: no such file or directory, open '${path}'`,
        );
        err.code = "ENOENT";
        throw err;
      }
      return memfs[path];
    },
    writeFile: async (path: string, data: string) => {
      memfs[path] = data;
    },
    rm: async (path: string) => {
      delete memfs[path];
    },
  };
});

// Mock package name & CLI version
const MOCK_PKG = "my-pkg";
vi.mock("../src/version", () => ({ CLI_VERSION: "1.0.0" }));
vi.mock("../package.json", () => ({ name: MOCK_PKG }));
vi.mock("../src/utils/package-manager-detector", async (importOriginal) => {
  return {
    ...(await importOriginal()),
    detectInstallerByPath: vi.fn(),
  };
});

// Mock external services
vi.mock("fast-npm-meta", () => ({ getLatestVersion: vi.fn() }));
vi.mock("package-manager-detector", () => ({ getUserAgent: vi.fn() }));

describe("renderUpdateCommand()", () => {
  it.each([
    [{ manager: "npm", packageName: MOCK_PKG }, `npm install -g ${MOCK_PKG}`],
    [{ manager: "pnpm", packageName: MOCK_PKG }, `pnpm add -g ${MOCK_PKG}`],
    [{ manager: "bun", packageName: MOCK_PKG }, `bun add -g ${MOCK_PKG}`],
    [{ manager: "yarn", packageName: MOCK_PKG }, `yarn global add ${MOCK_PKG}`],
    [
      { manager: "deno", packageName: MOCK_PKG },
      `deno install -g npm:${MOCK_PKG}`,
    ],
  ])("%s → command", async (options, cmd) => {
    expect(renderUpdateCommand(options as UpdateOptions)).toBe(cmd);
  });
});

describe("checkForUpdates()", () => {
  // Use a stable directory under the OS temp
  const TMP = join(os.tmpdir(), "update-test-memfs");
  const STATE_PATH = join(TMP, "update-check.json");

  beforeEach(async () => {
    memfs = {};
    // Mock CONFIG_DIR to our TMP
    vi.doMock("../src/utils/config", () => ({ CONFIG_DIR: TMP }));

    // Freeze time so the 24h logic is deterministic
    vi.useFakeTimers().setSystemTime(new Date("2025-01-01T00:00:00Z"));
    vi.resetAllMocks();
  });

  afterEach(async () => {
    vi.useRealTimers();
  });

  it("uses global installer when detected, ignoring local agent", async () => {
    // seed old timestamp
    const old = new Date("2000-01-01T00:00:00Z").toUTCString();
    memfs[STATE_PATH] = JSON.stringify({ lastUpdateCheck: old });

    // simulate registry says update available
    vi.mocked(getLatestVersion).mockResolvedValue({ version: "2.0.0" } as any);
    // local agent would be npm, but global detection wins
    vi.mocked(getUserAgent).mockReturnValue("npm");
    vi.mocked(detectInstallerByPath).mockReturnValue(Promise.resolve("pnpm"));

    const logSpy = vi.spyOn(console, "log").mockImplementation(() => {});

    await checkForUpdates();

    // should render using `pnpm` (global) rather than `npm`
    expect(logSpy).toHaveBeenCalledOnce();
    const output = logSpy.mock.calls.at(0)?.at(0);
    expect(output).toContain("pnpm add -g"); // global branch used
    // state updated
    const newState = JSON.parse(memfs[STATE_PATH]!);
    expect(newState.lastUpdateCheck).toBe(new Date().toUTCString());
  });

  it("skips when lastUpdateCheck is still fresh (<frequency)", async () => {
    // seed a timestamp 12h ago
    const recent = new Date(Date.now() - 1000 * 60 * 60 * 12).toUTCString();
    memfs[STATE_PATH] = JSON.stringify({ lastUpdateCheck: recent });

    const versionSpy = vi.mocked(getLatestVersion);
    const logSpy = vi.spyOn(console, "log").mockImplementation(() => {});

    await checkForUpdates();

    expect(versionSpy).not.toHaveBeenCalled();
    expect(logSpy).not.toHaveBeenCalled();
  });

  it("does not print when up-to-date", async () => {
    vi.mocked(getLatestVersion).mockResolvedValue({
      version: CLI_VERSION,
    } as any);
    vi.mocked(getUserAgent).mockReturnValue("npm");
    vi.mocked(detectInstallerByPath).mockResolvedValue(undefined);

    const logSpy = vi.spyOn(console, "log").mockImplementation(() => {});

    await checkForUpdates();

    expect(logSpy).not.toHaveBeenCalled();
    // but state still written
    const state = JSON.parse(memfs[STATE_PATH]!);
    expect(state.lastUpdateCheck).toBe(new Date().toUTCString());
  });

  it("does not print when no manager detected at all", async () => {
    vi.mocked(getLatestVersion).mockResolvedValue({ version: "2.0.0" } as any);
    vi.mocked(detectInstallerByPath).mockResolvedValue(undefined);
    vi.mocked(getUserAgent).mockReturnValue(null);

    const logSpy = vi.spyOn(console, "log").mockImplementation(() => {});

    await checkForUpdates();

    expect(logSpy).not.toHaveBeenCalled();
    // state still written
    const state = JSON.parse(memfs[STATE_PATH]!);
    expect(state.lastUpdateCheck).toBe(new Date().toUTCString());
  });

  it("renders a box when a newer version exists and no global installer", async () => {
    // old timestamp
    const old = new Date("2000-01-01T00:00:00Z").toUTCString();
    memfs[STATE_PATH] = JSON.stringify({ lastUpdateCheck: old });

    vi.mocked(getLatestVersion).mockResolvedValue({ version: "2.0.0" } as any);
    vi.mocked(detectInstallerByPath).mockResolvedValue(undefined);
    vi.mocked(getUserAgent).mockReturnValue("bun");

    const logSpy = vi.spyOn(console, "log").mockImplementation(() => {});

    await checkForUpdates();

    expect(logSpy).toHaveBeenCalledOnce();
    const output = logSpy.mock.calls[0]![0] as string;
    expect(output).toContain("bun add -g");
    expect(output).to.matchSnapshot();
    // state updated
    const state = JSON.parse(memfs[STATE_PATH]!);
    expect(state.lastUpdateCheck).toBe(new Date().toUTCString());
  });
});
