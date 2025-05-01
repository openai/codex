import fs from "fs";
import os from "os";
import path from "path";

/**
 * Represents a file system suggestion with path and directory information
 */
export interface FileSystemSuggestion {
  /** The full path of the suggestion */
  path: string;
  /** Whether the suggestion is a directory */
  isDirectory: boolean;
}

/**
 * Gets file system suggestions based on a path prefix
 * @param pathPrefix The path prefix to search for
 * @returns Array of file system suggestions
 */
export function getFileSystemSuggestions(
  pathPrefix: string,
): Array<FileSystemSuggestion> {
  if (!pathPrefix) {
    return [];
  }

  try {
    const sep = path.sep;
    const hasTilde = pathPrefix === "~" || pathPrefix.startsWith("~" + sep);
    const expanded = hasTilde
      ? path.join(os.homedir(), pathPrefix.slice(1))
      : pathPrefix;

    const normalized = path.normalize(expanded);
    const isDir = pathPrefix.endsWith(path.sep);
    const base = path.basename(normalized);

    const dir =
      normalized === "." && !pathPrefix.startsWith("." + sep) && !hasTilde
        ? process.cwd()
        : path.dirname(normalized);

    const readDir = isDir ? path.join(dir, base) : dir;

    return fs
      .readdirSync(readDir)
      .filter((item) => isDir || item.startsWith(base))
      .map((item) => {
        const fullPath = path.join(readDir, item);
        const isDirectory = fs.statSync(fullPath).isDirectory();
        return {
          path: isDirectory ? path.join(fullPath, sep) : fullPath,
          isDirectory,
        };
      });
  } catch {
    return [];
  }
}
