import { describe, it, expect, beforeEach, afterEach } from "vitest";
import fs from "fs";
import path from "path";
import os from "os";
import {
  getSafeFileContents,
  loadIgnorePatterns,
  getMergedIgnorePatterns,
} from "./context_files";

describe("context_files core functions", () => {
  let tmpDir: string;

  beforeEach(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "context-files-test-"));
  });

  afterEach(() => {
    if (tmpDir && fs.existsSync(tmpDir)) {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  it("loadIgnorePatterns returns regexes and ignores comments/empty lines", () => {
    const patterns = loadIgnorePatterns();
    expect(Array.isArray(patterns)).toBe(true);
    expect(patterns.length).toBeGreaterThan(0);
    patterns.forEach((re) => {
      expect(re).toBeInstanceOf(RegExp);
    });
  });

  it("getMergedIgnorePatterns combines default and .agentignore patterns", () => {
    fs.writeFileSync(path.join(tmpDir, ".agentignore"), "*.secret\nfoo.txt\n");
    const merged = getMergedIgnorePatterns(tmpDir);
    expect(Array.isArray(merged)).toBe(true);
    expect(merged.some((re) => re.test("bar.log"))).toBe(true);
    expect(merged.some((re) => re.test("foo.secret"))).toBe(true);
    expect(merged.some((re) => re.test("foo.txt"))).toBe(true);
    expect(merged.some((re) => re.test("randomfile.abc"))).toBe(false);
  });

  it("getSafeFileContents respects ignore patterns and .agentignore", async () => {
    fs.writeFileSync(path.join(tmpDir, "a.js"), "console.log(1);");
    fs.writeFileSync(path.join(tmpDir, "b.log"), "should be ignored");
    fs.writeFileSync(
      path.join(tmpDir, "c.secret"),
      "should be ignored by agentignore",
    );
    fs.writeFileSync(path.join(tmpDir, ".agentignore"), "*.secret\n");
    const files = await getSafeFileContents(tmpDir);
    const fileNames = files.map((f) => path.basename(f.path));
    expect(fileNames).toContain("a.js");
    expect(fileNames).not.toContain("b.log");
    expect(fileNames).not.toContain("c.secret");
  });

  it("ignores all files in a directory pattern", async () => {
    fs.mkdirSync(path.join(tmpDir, "logs"));
    fs.writeFileSync(
      path.join(tmpDir, "logs", "error.log"),
      "should be ignored",
    );
    fs.writeFileSync(
      path.join(tmpDir, "logs", "keep.log"),
      "should be ignored",
    );
    fs.writeFileSync(path.join(tmpDir, "main.js"), "should be included");
    fs.writeFileSync(path.join(tmpDir, ".agentignore"), "logs/\n");
    const files = await getSafeFileContents(tmpDir);
    const fileNames = files.map((f) => path.relative(tmpDir, f.path));
    expect(fileNames).toContain("main.js");
    expect(fileNames).not.toContain(path.join("logs", "error.log"));
    expect(fileNames).not.toContain(path.join("logs", "keep.log"));
  });

  it("nested directories are matched by patterns", async () => {
    fs.mkdirSync(path.join(tmpDir, "src"));
    fs.mkdirSync(path.join(tmpDir, "src", "secrets"));
    fs.writeFileSync(
      path.join(tmpDir, "src", "secrets", "hidden.txt"),
      "should be ignored",
    );
    fs.writeFileSync(path.join(tmpDir, "src", "main.js"), "should be included");
    fs.writeFileSync(path.join(tmpDir, ".agentignore"), "src/secrets/\n");
    const files = await getSafeFileContents(tmpDir);
    const fileNames = files.map((f) => path.relative(tmpDir, f.path));
    expect(fileNames).toContain(path.join("src", "main.js"));
    expect(fileNames).not.toContain(path.join("src", "secrets", "hidden.txt"));
  });

  it("pattern precedence: ignore then un-ignore", async () => {
    fs.writeFileSync(path.join(tmpDir, "foo.txt"), "should NOT be ignored");
    fs.writeFileSync(path.join(tmpDir, ".agentignore"), "foo.txt\n!foo.txt\n");
    const files = await getSafeFileContents(tmpDir);
    const fileNames = files.map((f) => path.basename(f.path));
    expect(fileNames).toContain("foo.txt");
  });

  it("malformed patterns do not crash and are ignored", async () => {
    fs.writeFileSync(path.join(tmpDir, "main.js"), "should be included");
    fs.writeFileSync(path.join(tmpDir, ".agentignore"), "[[badpattern\n");
    const files = await getSafeFileContents(tmpDir);
    const fileNames = files.map((f) => path.basename(f.path));
    expect(fileNames).toContain("main.js");
  });

  it("pattern with spaces is handled correctly", async () => {
    fs.writeFileSync(
      path.join(tmpDir, "file with space.txt"),
      "should be ignored",
    );
    fs.writeFileSync(
      path.join(tmpDir, ".agentignore"),
      "file with space.txt\n",
    );
    const files = await getSafeFileContents(tmpDir);
    const fileNames = files.map((f) => path.basename(f.path));
    expect(fileNames).not.toContain("file with space.txt");
  });
});
