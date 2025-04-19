import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { isGhInstalled, gh } from "../src/utils/github/gh-cli.js";
import { exec } from "../src/utils/agent/exec.js";

vi.mock("../src/utils/agent/exec.js", () => ({
  exec: vi.fn(),
}));

describe("GitHub CLI utilities", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  describe("isGhInstalled", () => {
    it("returns true when gh is installed", async () => {
      vi.mocked(exec).mockResolvedValueOnce({
        stdout: "gh version 2.30.0 (2023-01-01)\nhttps://github.com/cli/cli",
        stderr: "",
        exitCode: 0,
      });

      const result = await isGhInstalled();
      expect(result).toBe(true);
    });

    it("returns false when gh is not installed", async () => {
      vi.mocked(exec).mockResolvedValueOnce({
        stdout: "",
        stderr: "command not found: gh",
        exitCode: 1,
      });

      const result = await isGhInstalled();
      expect(result).toBe(false);
    });

    it("handles exceptions gracefully", async () => {
      vi.mocked(exec).mockRejectedValueOnce(new Error("Unexpected error"));

      const result = await isGhInstalled();
      expect(result).toBe(false);
    });
  });

  describe("gh", () => {
    it("executes gh commands correctly", async () => {
      vi.mocked(exec).mockResolvedValueOnce({
        stdout: "PR #1",
        stderr: "",
        exitCode: 0,
      });

      const result = await gh(["pr", "view", "1"]);
      
      expect(exec).toHaveBeenCalledWith(
        expect.objectContaining({
          cmd: ["gh", "pr", "view", "1"],
        }),
        expect.anything()
      );
      
      expect(result).toEqual({
        stdout: "PR #1",
        stderr: "",
        exitCode: 0,
      });
    });

    it("passes workdir and timeout options correctly", async () => {
      vi.mocked(exec).mockResolvedValueOnce({
        stdout: "Issue created",
        stderr: "",
        exitCode: 0,
      });

      await gh(["issue", "create"], { workdir: "/tmp", timeoutInMillis: 10000 });
      
      expect(exec).toHaveBeenCalledWith(
        expect.objectContaining({
          cmd: ["gh", "issue", "create"],
          workdir: "/tmp",
          timeoutInMillis: 10000,
        }),
        expect.anything()
      );
    });
  });
});