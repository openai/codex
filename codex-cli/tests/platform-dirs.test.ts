import { getConfigDir, getDataDir, getCacheDir, getLogDir, getLegacyConfigDir } from "../src/utils/platform-dirs.js";
import { homedir } from "os";
import { join } from "path";
import { vi, test, expect, beforeEach, afterEach } from "vitest";

// Mock os.homedir
vi.mock("os", () => ({
  homedir: vi.fn(),
}));

// Mock process.platform
const originalPlatform = process.platform;
let mockPlatform: string;

Object.defineProperty(process, "platform", {
  get: () => mockPlatform,
});

// Mock process.env
const originalEnv = process.env;

beforeEach(() => {
  // Reset mocks
  vi.mocked(homedir).mockReturnValue("/home/testuser");
  mockPlatform = "linux";
  process.env = { ...originalEnv };
});

afterEach(() => {
  // Restore original values
  process.env = originalEnv;
});

test("getConfigDir returns XDG path on Linux", () => {
  mockPlatform = "linux";
  process.env.XDG_CONFIG_HOME = "/home/testuser/.config";
  
  expect(getConfigDir()).toBe("/home/testuser/.config/codex");
});

test("getConfigDir uses default XDG path on Linux when XDG_CONFIG_HOME is not set", () => {
  mockPlatform = "linux";
  delete process.env.XDG_CONFIG_HOME;
  
  expect(getConfigDir()).toBe("/home/testuser/.config/codex");
});

test("getConfigDir returns Application Support path on macOS", () => {
  mockPlatform = "darwin";
  
  expect(getConfigDir()).toBe("/home/testuser/Library/Application Support/Codex");
});

test("getConfigDir returns legacy path on other platforms", () => {
  mockPlatform = "win32";
  
  expect(getConfigDir()).toBe("/home/testuser/.codex");
});

test("getDataDir returns XDG path on Linux", () => {
  mockPlatform = "linux";
  process.env.XDG_DATA_HOME = "/home/testuser/.local/share";
  
  expect(getDataDir()).toBe("/home/testuser/.local/share/codex");
});

test("getDataDir uses default XDG path on Linux when XDG_DATA_HOME is not set", () => {
  mockPlatform = "linux";
  delete process.env.XDG_DATA_HOME;
  
  expect(getDataDir()).toBe("/home/testuser/.local/share/codex");
});

test("getCacheDir returns XDG path on Linux", () => {
  mockPlatform = "linux";
  process.env.XDG_CACHE_HOME = "/home/testuser/.cache";
  
  expect(getCacheDir()).toBe("/home/testuser/.cache/codex");
});

test("getCacheDir uses default XDG path on Linux when XDG_CACHE_HOME is not set", () => {
  mockPlatform = "linux";
  delete process.env.XDG_CACHE_HOME;
  
  expect(getCacheDir()).toBe("/home/testuser/.cache/codex");
});

test("getLogDir returns XDG path on Linux", () => {
  mockPlatform = "linux";
  process.env.XDG_STATE_HOME = "/home/testuser/.local/state";
  
  expect(getLogDir()).toBe("/home/testuser/.local/state/codex/logs");
});

test("getLogDir uses default XDG path on Linux when XDG_STATE_HOME is not set", () => {
  mockPlatform = "linux";
  delete process.env.XDG_STATE_HOME;
  
  expect(getLogDir()).toBe("/home/testuser/.local/state/codex/logs");
});

test("getLegacyConfigDir always returns ~/.codex", () => {
  expect(getLegacyConfigDir()).toBe("/home/testuser/.codex");
});
