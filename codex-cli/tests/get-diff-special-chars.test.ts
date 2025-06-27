import { mkdtempSync, writeFileSync, rmSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { execSync } from "child_process";
import { describe, it, expect } from "vitest";

import { getGitDiff } from "../src/utils/get-diff.js";

describe("getGitDiff", () => {
  it("handles untracked files with special characters", () => {
    const repoDir = mkdtempSync(join(tmpdir(), "git-diff-test-"));
    const prevCwd = process.cwd();
    try {
      process.chdir(repoDir);
      execSync("git init", { stdio: "ignore" });

      const fileName = "a$b.txt";
      writeFileSync(join(repoDir, fileName), "hello\n");

      const { isGitRepo, diff } = getGitDiff();
      expect(isGitRepo).toBe(true);
      expect(diff).toContain(fileName);
    } finally {
      process.chdir(prevCwd);
      rmSync(repoDir, { recursive: true, force: true });
    }
  });
});
