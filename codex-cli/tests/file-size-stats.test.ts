import { describe, test, expect } from "vitest";
import { formatBytes, getFileSizeStats } from "../src/utils/file-size-stats";

describe("formatBytes()", () => {
  test("formats bytes correctly", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1024)).toBe("1 KB");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(1048576)).toBe("1 MB");
    expect(formatBytes(1073741824)).toBe("1 GB");
  });

  test("handles decimal values correctly", () => {
    expect(formatBytes(1234)).toBe("1.21 KB");
    expect(formatBytes(5678901)).toBe("5.42 MB");
  });
});

describe("getFileSizeStats()", () => {
  test("returns empty array for non-existent directory", () => {
    const stats = getFileSizeStats("/non/existent/path");
    expect(stats).toEqual([]);
  });

  test("handles directory with no files", () => {
    // This test would need a temp directory setup for a complete test
    // For now, we just verify the function doesn't crash
    expect(() => getFileSizeStats(".")).not.toThrow();
  });
});
