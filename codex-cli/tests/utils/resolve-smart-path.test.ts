import { describe, it, expect, beforeAll, afterAll } from "vitest";
import path from "path";
import fs from "fs";
import { resolveSmartPath } from "../../src/utils/resolve-smart-path";

describe("resolveSmartPath", () => {
  const tempDir = path.resolve(__dirname, "..", "temp-test-dir");

  beforeAll(() => {
    fs.mkdirSync(tempDir, { recursive: true });

    // Create auth route
    fs.mkdirSync(path.join(tempDir, "src/app/api/auth"), { recursive: true });
    fs.writeFileSync(
      path.join(tempDir, "src/app/api/auth/route.ts"),
      "// test file",
    );

    // Create user route
    fs.mkdirSync(path.join(tempDir, "src/app/api/user"), { recursive: true });
    fs.writeFileSync(
      path.join(tempDir, "src/app/api/user/route.ts"),
      "// not it",
    );
  });

  afterAll(() => {
    fs.rmSync(tempDir, { recursive: true, force: true });
  });

  it("resolves directly existing path", () => {
    const exactPath = path.join(tempDir, "src/app/api/auth/route.ts");
    const result = resolveSmartPath(exactPath);
    expect(result).toBe(exactPath);
  });

  it("falls back to best match when requested path is missing", () => {
    const result = resolveSmartPath("src/api/auth.ts", tempDir);
    expect(result.endsWith("src/app/api/auth/route.ts")).toBe(true);
  });

  it("throws when no match is found", () => {
    expect(() => resolveSmartPath("no/such/file.ts", tempDir)).toThrow();
  });
});
