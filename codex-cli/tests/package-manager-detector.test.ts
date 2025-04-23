import { describe, it, expect, beforeEach, vi, afterEach } from "vitest";
import which from "which";
import { detectInstallerByPath } from "../src/utils/package-manager-detector";
import { execFileSync } from "node:child_process";

vi.mock("which", () => ({
  default: { sync: vi.fn() },
}));
vi.mock("node:child_process", () => ({ execFileSync: vi.fn() }));

describe("detectInstallerByPath()", () => {
  const originalArgv = process.argv;
  const fakeBinDirs = {
    // `npm prefix -g` returns the global “prefix” (we’ll add `/bin` when detecting)
    npm: "/usr/local",
    pnpm: "/home/user/.local/share/pnpm/bin",
    bun: "/Users/test/.bun/bin",
  } as const;

  beforeEach(() => {
    vi.resetAllMocks();
    // Pretend each manager binary is on PATH:
    vi.mocked(which.sync).mockImplementation(() => "/fake/path");

    vi.mocked(execFileSync).mockImplementation(
      (
        cmd: string,
        _args: ReadonlyArray<string> = [],
        _options: unknown,
      ): string => {
        return fakeBinDirs[cmd as keyof typeof fakeBinDirs];
      },
    );
  });

  afterEach(() => {
    // Restore the real argv so tests don’t leak
    process.argv = originalArgv;
  });

  it.each(Object.entries(fakeBinDirs))(
    "detects %s when invoked from its global-bin",
    async (manager, binDir) => {
      // Simulate the shim living under that binDir
      process.argv =
        manager === "npm"
          ? [process.argv[0]!, `${binDir}/bin/my-cli`]
          : [process.argv[0]!, `${binDir}/my-cli`];
      const detected = await detectInstallerByPath();
      expect(detected).toBe(manager);
    },
  );

  it("returns undefined if argv[1] is missing", async () => {
    process.argv = [process.argv[0]!];
    expect(await detectInstallerByPath()).toBeUndefined();
    expect(execFileSync).not.toHaveBeenCalled();
  });

  it("returns undefined if shim isn't in any manager's bin", async () => {
    // stub execFileSync to some other dirs
    vi.mocked(execFileSync).mockImplementation(() => "/some/other/dir");
    process.argv = [process.argv[0]!, "/home/user/.node_modules/.bin/my-cli"];
    expect(await detectInstallerByPath()).toBeUndefined();
  });
});
