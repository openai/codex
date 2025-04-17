import { homedir } from "os";
import { join } from "path";
import { existsSync, mkdirSync } from "fs";

/**
 * Get the appropriate configuration directory based on the platform.
 * - Linux: Uses XDG Base Directory Specification ($XDG_CONFIG_HOME/codex or ~/.config/codex)
 * - macOS: Uses Apple's File System Programming Guide (~/Library/Application Support/Codex)
 * - Other platforms: Falls back to ~/.codex
 */
export function getConfigDir(): string {
  const platform = process.platform;
  const home = homedir();

  // Linux: Follow XDG Base Directory Specification
  if (platform === "linux") {
    const xdgConfigHome = process.env.XDG_CONFIG_HOME || join(home, ".config");
    return join(xdgConfigHome, "codex");
  }

  // macOS: Follow Apple's File System Programming Guide
  if (platform === "darwin") {
    return join(home, "Library", "Application Support", "Codex");
  }

  // Default fallback for other platforms (Windows, etc.)
  return join(home, ".codex");
}

/**
 * Get the appropriate data directory based on the platform.
 * - Linux: Uses XDG Base Directory Specification ($XDG_DATA_HOME/codex or ~/.local/share/codex)
 * - macOS: Uses Apple's File System Programming Guide (~/Library/Application Support/Codex)
 * - Other platforms: Falls back to ~/.codex
 */
export function getDataDir(): string {
  const platform = process.platform;
  const home = homedir();

  // Linux: Follow XDG Base Directory Specification
  if (platform === "linux") {
    const xdgDataHome =
      process.env.XDG_DATA_HOME || join(home, ".local", "share");
    return join(xdgDataHome, "codex");
  }

  // macOS: Uses the same directory as config for Application Support
  if (platform === "darwin") {
    return join(home, "Library", "Application Support", "Codex");
  }

  // Default fallback for other platforms (Windows, etc.)
  return join(home, ".codex");
}

/**
 * Get the appropriate cache directory based on the platform.
 * - Linux: Uses XDG Base Directory Specification ($XDG_CACHE_HOME/codex or ~/.cache/codex)
 * - macOS: Uses Apple's File System Programming Guide (~/Library/Caches/Codex)
 * - Other platforms: Falls back to ~/.codex/cache
 */
export function getCacheDir(): string {
  const platform = process.platform;
  const home = homedir();

  // Linux: Follow XDG Base Directory Specification
  if (platform === "linux") {
    const xdgCacheHome = process.env.XDG_CACHE_HOME || join(home, ".cache");
    return join(xdgCacheHome, "codex");
  }

  // macOS: Follow Apple's File System Programming Guide
  if (platform === "darwin") {
    return join(home, "Library", "Caches", "Codex");
  }

  // Default fallback for other platforms (Windows, etc.)
  return join(home, ".codex", "cache");
}

/**
 * Get the appropriate log directory based on the platform.
 * - Linux: Uses XDG Base Directory Specification ($XDG_STATE_HOME/codex/logs or ~/.local/state/codex/logs)
 * - macOS: Uses Apple's File System Programming Guide (~/Library/Logs/Codex)
 * - Other platforms: Falls back to ~/.codex/logs
 */
export function getLogDir(): string {
  const platform = process.platform;
  const home = homedir();

  // Linux: Follow XDG Base Directory Specification
  if (platform === "linux") {
    const xdgStateHome =
      process.env.XDG_STATE_HOME || join(home, ".local", "state");
    return join(xdgStateHome, "codex", "logs");
  }

  // macOS: Follow Apple's File System Programming Guide
  if (platform === "darwin") {
    return join(home, "Library", "Logs", "Codex");
  }

  // Default fallback for other platforms (Windows, etc.)
  return join(home, ".codex", "logs");
}

/**
 * Get the legacy config directory (~/.codex) for backward compatibility
 */
export function getLegacyConfigDir(): string {
  return join(homedir(), ".codex");
}

/**
 * Ensures that a directory exists, creating it if necessary
 */
export function ensureDirectoryExists(dir: string): void {
  if (!existsSync(dir)) {
    mkdirSync(dir, { recursive: true });
  }
}

/**
 * Checks if the legacy config directory exists
 */
export function legacyConfigDirExists(): boolean {
  return existsSync(getLegacyConfigDir());
}
