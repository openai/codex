import {
  analyzeProject,
  formatFileSize,
  formatProjectStats,
} from "../project-status";
import { writeFile, mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it, expect, beforeEach, afterEach } from "vitest";

describe("project-stats", () => {
  let tempDir: string;

  beforeEach(async () => {
    // 创建临时测试目录
    tempDir = await mkdtemp(join(tmpdir(), "codex-test-"));
  });

  afterEach(async () => {
    // 清理临时目录
    await rm(tempDir, { recursive: true, force: true });
  });

  it("should analyze a simple project structure", async () => {
    // 创建测试文件
    await writeFile(
      join(tempDir, "index.js"),
      'console.log("hello");\n// Comment\n',
    );
    await writeFile(
      join(tempDir, "style.css"),
      "body { margin: 0; }\n",
    );
    await writeFile(
      join(tempDir, "README.md"),
      "# Test Project\n\nDescription here.\n",
    );

    // 创建子目录
    const srcDir = join(tempDir, "src");
    await mkdir(srcDir);
    await writeFile(
      join(srcDir, "app.ts"),
      "const x: number = 42;\nexport default x;\n",
    );

    const stats = await analyzeProject(tempDir);

    expect(stats.totalFiles).toBe(4);
    expect(stats.totalLines).toBe(6); // 只计算代码文件的行数

    // 使用可选链和非空断言来处理可能为 undefined 的情况
    const jsExt = stats.filesByExtension[".js"];
    expect(jsExt).toBeDefined();
    expect(jsExt!.extension).toBe(".js");
    expect(jsExt!.count).toBe(1);
    expect(jsExt!.totalLines).toBe(2);

    const tsExt = stats.filesByExtension[".ts"];
    expect(tsExt).toBeDefined();
    expect(tsExt!.extension).toBe(".ts");
    expect(tsExt!.count).toBe(1);
    expect(tsExt!.totalLines).toBe(2);

    const cssExt = stats.filesByExtension[".css"];
    expect(cssExt).toBeDefined();
    expect(cssExt!.extension).toBe(".css");
    expect(cssExt!.count).toBe(1);
    expect(cssExt!.totalLines).toBe(1);
  });

  it("should ignore node_modules and .git directories", async () => {
    // 创建应该被忽略的目录和文件
    await mkdir(join(tempDir, "node_modules"));
    await writeFile(
      join(tempDir, "node_modules", "package.js"),
      "module.exports = {};",
    );

    await mkdir(join(tempDir, ".git"));
    await writeFile(join(tempDir, ".git", "config"), "[core]");

    // 创建应该被包含的文件
    await writeFile(join(tempDir, "index.js"), 'console.log("test");');

    const stats = await analyzeProject(tempDir);

    expect(stats.totalFiles).toBe(1);

    const jsExt = stats.filesByExtension[".js"];
    expect(jsExt).toBeDefined();
    expect(jsExt!.count).toBe(1);
  });

  it("should format file sizes correctly", () => {
    expect(formatFileSize(512)).toBe("512.0 B");
    expect(formatFileSize(1024)).toBe("1.0 KB");
    expect(formatFileSize(1536)).toBe("1.5 KB");
    expect(formatFileSize(1048576)).toBe("1.0 MB");
    expect(formatFileSize(1073741824)).toBe("1.0 GB");
  });

  it("should track recently modified files", async () => {
    await writeFile(join(tempDir, "old.js"), "old file");

    // 等待一毫秒确保时间戳不同
    await new Promise((resolve) => setTimeout(resolve, 10));

    await writeFile(join(tempDir, "new.js"), "new file");

    const stats = await analyzeProject(tempDir);

    expect(stats.recentFiles).toHaveLength(2);
    expect(stats.recentFiles.length).toBeGreaterThan(0);

    // 安全地访问数组元素
    const firstFile = stats.recentFiles[0];
    const secondFile = stats.recentFiles[1];

    expect(firstFile).toBeDefined();
    expect(secondFile).toBeDefined();
    expect(firstFile!.path).toBe("new.js"); // 最新的文件在前
    expect(secondFile!.path).toBe("old.js");
  });

  it("should generate formatted output", async () => {
    await writeFile(join(tempDir, "test.js"), 'console.log("test");\n');

    const stats = await analyzeProject(tempDir);
    const output = formatProjectStats(stats);

    expect(output).toContain("📊 Project Statistics");
    expect(output).toContain("📁 Total Files: 1");
    expect(output).toContain("📝 Total Lines of Code: 1");
    expect(output).toContain(".js");
    expect(output).toContain("🕒 Recently Modified Files:");
  });

  it("should handle empty directories", async () => {
    const stats = await analyzeProject(tempDir);

    expect(stats.totalFiles).toBe(0);
    expect(stats.totalLines).toBe(0);
    expect(Object.keys(stats.filesByExtension)).toHaveLength(0);
    expect(stats.recentFiles).toHaveLength(0);
  });

  it("should handle file extensions properly", async () => {
    // 创建没有扩展名的文件
    await writeFile(join(tempDir, "Dockerfile"), "FROM node:18");

    // 创建有扩展名的文件
    await writeFile(join(tempDir, "test.json"), '{"test": true}');

    const stats = await analyzeProject(tempDir);

    expect(stats.totalFiles).toBe(2);

    // 检查无扩展名文件
    const noExt = stats.filesByExtension["no-extension"];
    expect(noExt).toBeDefined();
    expect(noExt!.count).toBe(1);

    // 检查 JSON 文件
    const jsonExt = stats.filesByExtension[".json"];
    expect(jsonExt).toBeDefined();
    expect(jsonExt!.count).toBe(1);
  });

  it("should calculate project size correctly", async () => {
    const content = "test content";
    await writeFile(join(tempDir, "test.txt"), content);

    const stats = await analyzeProject(tempDir);

    expect(stats.projectSize).toBeGreaterThan(0);
    expect(stats.projectSize).toBe(content.length);
  });

  it("should sort recent files by modification time", async () => {
    // 创建多个文件，确保时间戳不同
    await writeFile(join(tempDir, "file1.txt"), "content1");
    await new Promise((resolve) => setTimeout(resolve, 10));

    await writeFile(join(tempDir, "file2.txt"), "content2");
    await new Promise((resolve) => setTimeout(resolve, 10));

    await writeFile(join(tempDir, "file3.txt"), "content3");

    const stats = await analyzeProject(tempDir);

    expect(stats.recentFiles).toHaveLength(3);

    // 验证排序（最新的在前）
    const files = stats.recentFiles;
    expect(files[0]!.path).toBe("file3.txt");
    expect(files[1]!.path).toBe("file2.txt");
    expect(files[2]!.path).toBe("file1.txt");

    // 验证时间戳递减
    expect(files[0]!.lastModified.getTime()).toBeGreaterThan(files[1]!.lastModified.getTime());
    expect(files[1]!.lastModified.getTime()).toBeGreaterThan(files[2]!.lastModified.getTime());
  });
});