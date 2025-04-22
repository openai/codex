import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import fs from "fs";
import path from "path";
import os from "os";
import { FileFilter } from "./file_filter";
import { renderTaskContext } from "./context";
import * as contextFiles from "./context_files";

describe("FileFilter integration and agentignore functionality", () => {
  let tmpDir: string;

  beforeEach(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "file-filter-test-"));
  });

  afterEach(() => {
    if (tmpDir && fs.existsSync(tmpDir)) {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
    vi.restoreAllMocks();
  });

  it("should filter files based on both .agentignore and prompt directives", () => {
    // Create a mock .agentignore file
    const agentignorePath = path.join(tmpDir, ".agentignore");
    fs.writeFileSync(agentignorePath, "*.env\nsecrets.json\n");
    // Create test files
    fs.writeFileSync(path.join(tmpDir, "app.js"), 'console.log("Hello");');
    fs.writeFileSync(path.join(tmpDir, ".env"), "SECRET_KEY=abc123");
    fs.writeFileSync(path.join(tmpDir, "secrets.json"), '{"key": "value"}');
    fs.writeFileSync(path.join(tmpDir, "config.js"), "export default {};");
    // Create a prompt with hidden directive
    const prompt = "Help me with my code\n#hidden: config.js";
    // Create FileFilter instance
    const fileFilter = new FileFilter(tmpDir, prompt);
    const allFiles = [
      path.join(tmpDir, "app.js"),
      path.join(tmpDir, ".env"),
      path.join(tmpDir, "secrets.json"),
      path.join(tmpDir, "config.js"),
    ];
    const result = fileFilter.filterFiles(allFiles);

    expect(result.visibleFiles).toHaveLength(1);
    expect(result.visibleFiles[0]).toBe(path.join(tmpDir, "app.js"));
    expect(result.hiddenFileInfo.count).toBe(3);
    expect(result.hiddenFileInfo.examples).toContain(".env");
    expect(result.hiddenFileInfo.examples).toContain("secrets.json");
    expect(result.hiddenFileInfo.examples).toContain("config.js");
    expect(result.hiddenFileInfo.userSpecified).toBe(true);
    const cleanedPrompt = fileFilter.getCleanedPrompt(prompt);
    expect(cleanedPrompt).toBe("Help me with my code");
    expect(cleanedPrompt).not.toContain("#hidden:");
  });

  it("should detect .agentignore file in project root using FileFilter", () => {
    const agentignorePath = path.join(tmpDir, ".agentignore");
    fs.writeFileSync(agentignorePath, "*.env\nsecrets.json\nnode_modules/\n");
    fs.writeFileSync(path.join(tmpDir, "index.js"), 'console.log("Hello");');
    fs.writeFileSync(path.join(tmpDir, ".env"), "SECRET_KEY=abc123");
    fs.writeFileSync(path.join(tmpDir, "secrets.json"), '{"key": "value"}');
    fs.mkdirSync(path.join(tmpDir, "node_modules"));
    fs.writeFileSync(path.join(tmpDir, "node_modules", "package.json"), "{}");
    const fileFilter = new FileFilter(tmpDir, "");
    const allFiles = [
      path.join(tmpDir, "index.js"),
      path.join(tmpDir, ".env"),
      path.join(tmpDir, "secrets.json"),
      path.join(tmpDir, "node_modules", "package.json"),
    ];
    const result = fileFilter.filterFiles(allFiles);
    expect(result.hiddenFileInfo.count).toBe(3);
    expect(result.hiddenFileInfo.examples).toContain(".env");
    expect(result.hiddenFileInfo.examples).toContain("secrets.json");
    expect(result.hiddenFileInfo.examples).toContain("package.json");
    expect(result.visibleFiles).toContain(path.join(tmpDir, "index.js"));
    expect(result.visibleFiles).not.toContain(path.join(tmpDir, ".env"));
  });

  it("should handle empty .agentignore file using FileFilter", () => {
    const agentignorePath = path.join(tmpDir, ".agentignore");
    fs.writeFileSync(agentignorePath, "");
    fs.writeFileSync(path.join(tmpDir, "index.js"), 'console.log("Hello");');
    const fileFilter = new FileFilter(tmpDir, "");
    const allFiles = [path.join(tmpDir, "index.js")];
    const result = fileFilter.filterFiles(allFiles);
    expect(result.hiddenFileInfo.count).toBe(0);
    expect(result.visibleFiles).toContain(path.join(tmpDir, "index.js"));
  });

  it("should handle non-existent .agentignore file using FileFilter", () => {
    fs.writeFileSync(path.join(tmpDir, "index.js"), 'console.log("Hello");');
    const fileFilter = new FileFilter(tmpDir, "");
    const allFiles = [path.join(tmpDir, "index.js")];
    const result = fileFilter.filterFiles(allFiles);
    expect(result.hiddenFileInfo.count).toBe(0);
    expect(result.visibleFiles).toContain(path.join(tmpDir, "index.js"));
  });

  it("should handle complex glob patterns in .agentignore using FileFilter", () => {
    const agentignorePath = path.join(tmpDir, ".agentignore");
    fs.writeFileSync(
      agentignorePath,
      "config/*.secret\n!config/public.secret\n**/*.key\n",
    );
    fs.mkdirSync(path.join(tmpDir, "config"));
    fs.writeFileSync(path.join(tmpDir, "config", "db.secret"), "password=123");
    fs.writeFileSync(
      path.join(tmpDir, "config", "public.secret"),
      "public=true",
    );
    fs.mkdirSync(path.join(tmpDir, "keys"));
    fs.writeFileSync(path.join(tmpDir, "keys", "private.key"), "secret-key");
    fs.writeFileSync(path.join(tmpDir, "index.js"), 'console.log("Hello");');
    const fileFilter = new FileFilter(tmpDir, "");
    const allFiles = [
      path.join(tmpDir, "config", "db.secret"),
      path.join(tmpDir, "config", "public.secret"),
      path.join(tmpDir, "keys", "private.key"),
      path.join(tmpDir, "index.js"),
    ];
    const result = fileFilter.filterFiles(allFiles);
    expect(result.hiddenFileInfo.examples).toContain("db.secret");
    expect(result.hiddenFileInfo.examples).toContain("private.key");
    expect(result.hiddenFileInfo.examples).not.toContain("public.secret");
    expect(result.visibleFiles).toContain(
      path.join(tmpDir, "config", "public.secret"),
    );
    expect(result.visibleFiles).toContain(path.join(tmpDir, "index.js"));
  });

  it("should include security notice when hidden files exist", () => {
    const taskContext = {
      prompt: "Test prompt",
      input_paths: ["/test/path1", "/test/path2"],
      input_paths_structure: "Mock directory structure",
      files: [{ path: "/test/file1.js", content: 'console.log("test");' }],
      hiddenFileInfo: {
        count: 2,
        examples: ["secret.json", ".env"],
        userSpecified: true,
      },
    };
    const renderedContext = renderTaskContext(taskContext);
    expect(renderedContext).toContain("# IMPORTANT SECURITY RESTRICTIONS");
    expect(renderedContext).toContain("2 files are hidden from your view");
    expect(renderedContext).toContain("Examples include: secret.json, .env");
    expect(renderedContext).toContain("YOU CANNOT ACCESS THESE FILES");
  });

  it("should not include security notice when no hidden files exist", () => {
    const taskContext = {
      prompt: "Test prompt",
      input_paths: ["/test/path1", "/test/path2"],
      input_paths_structure: "Mock directory structure",
      files: [{ path: "/test/file1.js", content: 'console.log("test");' }],
      hiddenFileInfo: {
        count: 0,
        examples: [],
        userSpecified: false,
      },
    };
    const renderedContext = renderTaskContext(taskContext);
    expect(renderedContext).not.toContain("# IMPORTANT SECURITY RESTRICTIONS");
    expect(renderedContext).not.toContain("files are hidden from your view");
  });

  it("should handle integration with context_files processing", async () => {
    const agentignorePath = path.join(tmpDir, ".agentignore");
    fs.writeFileSync(agentignorePath, "*.secret\n.env*\n");
    fs.writeFileSync(path.join(tmpDir, "app.js"), 'console.log("Hello");');
    fs.writeFileSync(
      path.join(tmpDir, "credentials.secret"),
      "password=abc123",
    );
    fs.writeFileSync(path.join(tmpDir, ".env.local"), "API_KEY=xyz789");
    const fileFilter = new FileFilter(tmpDir, "");
    const allFiles = [
      path.join(tmpDir, "app.js"),
      path.join(tmpDir, "credentials.secret"),
      path.join(tmpDir, ".env.local"),
    ];
    const filterResult = fileFilter.filterFiles(allFiles);
    const hiddenFileInfo = filterResult.hiddenFileInfo;
    expect(hiddenFileInfo.count).toBe(2);
    expect(hiddenFileInfo.examples).toContain("credentials.secret");
    expect(hiddenFileInfo.examples).toContain(".env.local");
    const mockGetFileContents = vi
      .fn()
      .mockResolvedValue([
        { path: path.join(tmpDir, "app.js"), content: 'console.log("Hello");' },
      ]);
    const getFileContentsSpy = vi
      .spyOn(contextFiles, "getFileContents")
      .mockImplementation(mockGetFileContents);

    try {
      const taskContext = {
        prompt: "Help me with my code",
        input_paths: [tmpDir],
        input_paths_structure: `${tmpDir}\n├── app.js`,
        files: await mockGetFileContents(tmpDir, []),
        hiddenFileInfo,
      };
      expect(taskContext.files).toHaveLength(1);
      expect(taskContext.files[0].path).toBe(path.join(tmpDir, "app.js"));
      expect(taskContext.hiddenFileInfo).toBeDefined();
      expect(taskContext.hiddenFileInfo?.count).toBe(2);
      const rendered = renderTaskContext(taskContext);
      expect(rendered).toContain("# IMPORTANT SECURITY RESTRICTIONS");
      expect(rendered).toContain("2 files are hidden from your view");
    } finally {
      getFileContentsSpy.mockRestore();
    }
  });
});
