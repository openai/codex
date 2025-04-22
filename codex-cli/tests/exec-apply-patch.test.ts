import { execApplyPatch } from "../src/utils/agent/exec.js";
import fs from "fs";
import os from "os";
import path from "path";
import { test, expect } from "vitest";

/**
 * This test verifies that `execApplyPatch()` is able to add a new file whose
 * parent directory does not yet exist. Prior to the fix, the call would throw
 * because `fs.writeFileSync()` could not create intermediate directories. The
 * test creates an isolated temporary directory to avoid polluting the project
 * workspace.
 */
test("execApplyPatch creates missing directories when adding a file", () => {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "apply-patch-test-"));

  // Ensure we start from a clean slate.
  const nestedFileRel = path.join("foo", "bar", "baz.txt");
  const nestedFileAbs = path.join(tmpDir, nestedFileRel);
  expect(fs.existsSync(nestedFileAbs)).toBe(false);

  const patch = `*** Begin Patch\n*** Add File: ${nestedFileRel}\n+hello new world\n*** End Patch`;

  // Run execApplyPatch() with cwd switched to tmpDir so that the relative
  // path in the patch is resolved inside the temporary location.
  const prevCwd = process.cwd();
  try {
    process.chdir(tmpDir);

    const result = execApplyPatch(patch);
    expect(result.exitCode).toBe(0);
    expect(result.stderr).toBe("");
  } finally {
    process.chdir(prevCwd);
  }

  // The file (and its parent directories) should have been created with the
  // expected contents.
  const fileContents = fs.readFileSync(nestedFileAbs, "utf8");
  expect(fileContents).toBe("hello new world");

  // Cleanup to keep tmpdir tidy.
  fs.rmSync(tmpDir, { recursive: true, force: true });
});
