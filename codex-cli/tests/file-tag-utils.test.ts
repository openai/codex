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
});

describe("collapseXmlBlocks", () => {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "codex-collapse-test-"));
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

  it("collapses XML block to @path format for valid file", () => {
    // Create a real file
    const fileName = "valid-file.txt";
    fs.writeFileSync(path.join(tmpDir, fileName), "file content");

    const input = `<${fileName}>\nHello, world!\n</${fileName}>`;
    const output = collapseXmlBlocks(input);
    expect(output).toBe(`@${fileName}`);
  });

  it("does not collapse XML block for unrelated xml block", () => {
    const xmlBlockName = "non-file-block";
    const input = `<${xmlBlockName}>\nContent here\n</${xmlBlockName}>`;
    const output = collapseXmlBlocks(input);
    // Should remain unchanged
    expect(output).toBe(input);
  });

  it("does not collapse XML block for a directory", () => {
    // Create a directory
    const dirName = "test-dir";
    fs.mkdirSync(path.join(tmpDir, dirName), { recursive: true });

    const input = `<${dirName}>\nThis is a directory\n</${dirName}>`;
    const output = collapseXmlBlocks(input);
    // Should remain unchanged
    expect(output).toBe(input);
  });

  it("collapses multiple valid file XML blocks in one string", () => {
    // Create real files
    const fileA = "a.txt";
    const fileB = "b.txt";
    fs.writeFileSync(path.join(tmpDir, fileA), "A content");
    fs.writeFileSync(path.join(tmpDir, fileB), "B content");

    const input = `<${fileA}>\nA content\n</${fileA}> and <${fileB}>\nB content\n</${fileB}>`;
    const output = collapseXmlBlocks(input);
    expect(output).toBe(`@${fileA} and @${fileB}`);
  });

  it("only collapses valid file paths in mixed content", () => {
    // Create a real file
    const validFile = "valid.txt";
    fs.writeFileSync(path.join(tmpDir, validFile), "valid content");
    const invalidFile = "invalid.txt";

    const input = `<${validFile}>\nvalid content\n</${validFile}> and <${invalidFile}>\ninvalid content\n</${invalidFile}>`;
    const output = collapseXmlBlocks(input);
    expect(output).toBe(
      `@${validFile} and <${invalidFile}>\ninvalid content\n</${invalidFile}>`,
    );
  });

  it("handles paths with subdirectories for valid files", () => {
    // Create a nested file
    const nestedDir = "nested/path";
    const nestedFile = "nested/path/file.txt";
    fs.mkdirSync(path.join(tmpDir, nestedDir), { recursive: true });
    fs.writeFileSync(path.join(tmpDir, nestedFile), "nested content");

    const relPath = "nested/path/file.txt";
    const input = `<${relPath}>\nContent here\n</${relPath}>`;
    const output = collapseXmlBlocks(input);
    const expectedPath = path.normalize(relPath);
    expect(output).toBe(`@${expectedPath}`);
  });

  it("handles XML blocks with special characters in path for valid files", () => {
    // Create a file with special characters
    const specialFileName = "weird-._~name.txt";
    fs.writeFileSync(path.join(tmpDir, specialFileName), "special chars");

    const input = `<${specialFileName}>\nspecial chars\n</${specialFileName}>`;
    const output = collapseXmlBlocks(input);
    expect(output).toBe(`@${specialFileName}`);
  });

  it("handles XML blocks with empty content for valid files", () => {
    // Create an empty file
    const emptyFileName = "empty.txt";
    fs.writeFileSync(path.join(tmpDir, emptyFileName), "");

    const input = `<${emptyFileName}>\n\n</${emptyFileName}>`;
    const output = collapseXmlBlocks(input);
    expect(output).toBe(`@${emptyFileName}`);
  });

  it("handles string with no XML blocks", () => {
    const input = "No tags here.";
    const output = collapseXmlBlocks(input);
    expect(output).toBe(input);
  });

  it("handles adjacent XML blocks for valid files", () => {
    // Create real files
    const adjFile1 = "adj1.txt";
    const adjFile2 = "adj2.txt";
    fs.writeFileSync(path.join(tmpDir, adjFile1), "adj1");
    fs.writeFileSync(path.join(tmpDir, adjFile2), "adj2");

    const input = `<${adjFile1}>\nadj1\n</${adjFile1}><${adjFile2}>\nadj2\n</${adjFile2}>`;
    const output = collapseXmlBlocks(input);
    expect(output).toBe(`@${adjFile1}@${adjFile2}`);
  });

  it("ignores malformed XML blocks", () => {
    const input = "<incomplete>content without closing tag";
    const output = collapseXmlBlocks(input);
    expect(output).toBe(input);
  });

  it("handles mixed content with valid file XML blocks and regular text", () => {
    // Create a real file
    const mixedFile = "mixed-file.txt";
    fs.writeFileSync(path.join(tmpDir, mixedFile), "file content");

    const input = `This is <${mixedFile}>\nfile content\n</${mixedFile}> and some more text.`;
    const output = collapseXmlBlocks(input);
    expect(output).toBe(`This is @${mixedFile} and some more text.`);
  });
});
