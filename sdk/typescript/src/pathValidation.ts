import path from "node:path";
import fs from "node:fs/promises";

/**
 * Validates that a file path is safe to use and prevents path traversal attacks.
 *
 * Security checks performed:
 * - Prevents null byte injection
 * - Resolves to absolute path to prevent relative path tricks
 * - Ensures the file exists and is accessible
 * - Prevents access to sensitive system directories
 *
 * @param filePath - The file path to validate
 * @param allowedBasePath - Optional base path that the file must be within
 * @throws Error if the path is invalid or unsafe
 * @returns The resolved absolute path if valid
 */
export async function validateFilePath(
  filePath: string,
  allowedBasePath?: string,
): Promise<string> {
  // Check for null byte injection
  if (filePath.includes("\0")) {
    throw new Error("Invalid file path: contains null bytes");
  }

  // Check for empty path
  if (!filePath || filePath.trim() === "") {
    throw new Error("Invalid file path: path is empty");
  }

  // Resolve to absolute path to handle relative paths
  let resolvedPath = path.resolve(filePath);

  // Check if the file exists first before attempting to resolve symlinks
  try {
    await fs.access(resolvedPath);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      throw new Error(`Invalid file path: '${filePath}' does not exist`);
    }
    if ((error as NodeJS.ErrnoException).code === "EACCES") {
      throw new Error(`Invalid file path: '${filePath}' is not accessible`);
    }
    throw new Error(`Invalid file path: unable to access '${filePath}': ${(error as Error).message}`);
  }

  // Resolve symbolic links to their real paths
  try {
    resolvedPath = await fs.realpath(resolvedPath);
  } catch (error) {
    throw new Error(`Invalid file path: unable to resolve '${filePath}': ${(error as Error).message}`);
  }

  // If an allowed base path is provided, ensure the resolved path is within it
  if (allowedBasePath) {
    const resolvedBasePath = path.resolve(allowedBasePath);
    const relativePath = path.relative(resolvedBasePath, resolvedPath);

    // If the relative path starts with '..' or is absolute, it's outside the base path
    if (relativePath.startsWith("..") || path.isAbsolute(relativePath)) {
      throw new Error(
        `Invalid file path: '${filePath}' is outside the allowed directory '${allowedBasePath}'`,
      );
    }
  }

  // Block access to common sensitive directories on Unix-like systems
  const sensitiveDirectories = [
    "/etc/shadow",
    "/etc/passwd",
    "/etc/sudoers",
    "/root/.ssh",
    "/proc",
    "/sys",
  ];

  for (const sensitiveDir of sensitiveDirectories) {
    if (resolvedPath === sensitiveDir || resolvedPath.startsWith(sensitiveDir + path.sep)) {
      throw new Error(`Access denied: cannot access sensitive system path '${resolvedPath}'`);
    }
  }

  // Verify it's a file (not a directory)
  try {
    const stats = await fs.stat(resolvedPath);
    if (!stats.isFile()) {
      throw new Error(`Invalid file path: '${filePath}' is not a file`);
    }
  } catch (error) {
    throw new Error(`Invalid file path: unable to stat '${filePath}': ${(error as Error).message}`);
  }

  return resolvedPath;
}
