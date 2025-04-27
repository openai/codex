import { describe, it, expect, beforeAll, afterAll } from "vitest";
import fs from "fs";
import path from "path";
import os from "os";
import { expandFileTags } from "../src/utils/expand-file-tags.js";

/**
 * Unit-tests for the expandFileTags() helper. The helper replaces tokens like
 * `@relative/path` with an XML block that inlines the file contents so that it
 * can be sent to the LLM. The tests exercise positive and negative cases.
 */

describe("expandFileTags", () => {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "codex-test-"));
  const originalCwd = process.cwd();

  beforeAll(() => {
    // Run the test from within the temporary directory so that the helper
    // generates relative paths that are predictable and isolated.
    process.chdir(tmpDir);
  });

  afterAll(() => {
    process.chdir(originalCwd);
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it("replaces @file token with XML wrapped contents", async () => {
    const filename = "hello.txt";
    const fileContent = "Hello, world!";
    fs.writeFileSync(path.join(tmpDir, filename), fileContent);

    const input = `Please read @${filename}`;
    const output = await expandFileTags(input);

    expect(output).toContain(`<${filename}>`);
    expect(output).toContain(fileContent);
    expect(output).toContain(`</${filename}>`);
  });

  it("leaves token unchanged when file does not exist", async () => {
    const input = "This refers to @nonexistent.file";
    const output = await expandFileTags(input);
    expect(output).toEqual(input);
  });

  it("handles multiple @file tokens in one string", async () => {
    const fileA = "a.txt";
    const fileB = "b.txt";
    fs.writeFileSync(path.join(tmpDir, fileA), "A content");
    fs.writeFileSync(path.join(tmpDir, fileB), "B content");
    const input = `@${fileA} and @${fileB}`;
    const output = await expandFileTags(input);
    expect(output).toContain("A content");
    expect(output).toContain("B content");
    expect(output).toContain(`<${fileA}>`);
    expect(output).toContain(`<${fileB}>`);
  });

  it("does not replace @dir if it's a directory", async () => {
    const dirName = "somedir";
    fs.mkdirSync(path.join(tmpDir, dirName));
    const input = `Check @${dirName}`;
    const output = await expandFileTags(input);
    expect(output).toContain(`@${dirName}`);
  });

  it("handles @file with special characters in name", async () => {
    const fileName = "weird-._~name.txt";
    fs.writeFileSync(path.join(tmpDir, fileName), "special chars");
    const input = `@${fileName}`;
    const output = await expandFileTags(input);
    expect(output).toContain("special chars");
    expect(output).toContain(`<${fileName}>`);
  });

  it("handles repeated @file tokens", async () => {
    const fileName = "repeat.txt";
    fs.writeFileSync(path.join(tmpDir, fileName), "repeat content");
    const input = `@${fileName} @${fileName}`;
    const output = await expandFileTags(input);
    // Both tags should be replaced
    expect(output.match(new RegExp(`<${fileName}>`, "g"))?.length).toBe(2);
  });

  it("handles empty file", async () => {
    const fileName = "empty.txt";
    fs.writeFileSync(path.join(tmpDir, fileName), "");
    const input = `@${fileName}`;
    const output = await expandFileTags(input);
    expect(output).toContain(`<${fileName}>\n\n</${fileName}>`);
  });

  it("handles string with no @file tokens", async () => {
    const input = "No tags here.";
    const output = await expandFileTags(input);
    expect(output).toBe(input);
  });

  it("handles @ at end of string with no path", async () => {
    const input = "Ends with @";
    const output = await expandFileTags(input);
    expect(output).toBe(input);
  });

  it("handles adjacent tokens", async () => {
    const file1 = "adj1.txt";
    const file2 = "adj2.txt";
    fs.writeFileSync(path.join(tmpDir, file1), "adj1");
    fs.writeFileSync(path.join(tmpDir, file2), "adj2");
    const input = `@${file1}@${file2}`;
    const output = await expandFileTags(input);
    expect(output).toContain("adj1");
    expect(output).toContain("adj2");
  });
});
