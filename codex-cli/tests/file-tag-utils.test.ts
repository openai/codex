import { describe, it, expect, beforeAll, afterAll } from "vitest";
import fs from "fs";
import path from "path";
import os from "os";
import {
  expandFileTags,
  collapseXmlBlocks,
} from "../src/utils/file-tag-utils.js";

/**
 * Unit-tests for file tag utility functions:
 * - expandFileTags(): Replaces tokens like `@relative/path` with XML blocks containing file contents
 * - collapseXmlBlocks(): Reverses the expansion, converting XML blocks back to @path format
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

describe("collapseXmlBlocks", () => {
  it("collapses a single XML block to @path format", () => {
    const input = "<hello.txt>\nHello, world!\n</hello.txt>";
    const output = collapseXmlBlocks(input);
    expect(output).toBe("@hello.txt");
  });

  it("collapses multiple XML blocks in one string", () => {
    const input =
      "<a.txt>\nA content\n</a.txt> and <b.txt>\nB content\n</b.txt>";
    const output = collapseXmlBlocks(input);
    expect(output).toBe("@a.txt and @b.txt");
  });

  it("handles paths with subdirectories", () => {
    const input = "<path/to/file.txt>\nContent here\n</path/to/file.txt>";
    const output = collapseXmlBlocks(input);
    const expectedPath = path.normalize("path/to/file.txt");
    expect(output).toBe(`@${expectedPath}`);
  });

  it("handles XML blocks with special characters in path", () => {
    const input = "<weird-._~name.txt>\nspecial chars\n</weird-._~name.txt>";
    const output = collapseXmlBlocks(input);
    expect(output).toBe("@weird-._~name.txt");
  });

  it("handles XML blocks with empty content", () => {
    const input = "<empty.txt>\n\n</empty.txt>";
    const output = collapseXmlBlocks(input);
    expect(output).toBe("@empty.txt");
  });

  it("handles string with no XML blocks", () => {
    const input = "No tags here.";
    const output = collapseXmlBlocks(input);
    expect(output).toBe(input);
  });

  it("handles adjacent XML blocks", () => {
    const input = "<adj1.txt>\nadj1\n</adj1.txt><adj2.txt>\nadj2\n</adj2.txt>";
    const output = collapseXmlBlocks(input);
    expect(output).toBe("@adj1.txt@adj2.txt");
  });

  it("ignores malformed XML blocks", () => {
    const input = "<incomplete>content without closing tag";
    const output = collapseXmlBlocks(input);
    expect(output).toBe(input);
  });

  it("handles mixed content with XML blocks and regular text", () => {
    const input =
      "This is <file.txt>\nfile content\n</file.txt> and some more text.";
    const output = collapseXmlBlocks(input);
    expect(output).toBe("This is @file.txt and some more text.");
  });
});
