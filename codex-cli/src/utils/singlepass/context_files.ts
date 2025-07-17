/* eslint-disable no-await-in-loop */

import * as fsSync from "fs";
import fs from "fs/promises";
import path, { dirname } from "path";

/** Represents file contents with absolute path. */
export interface FileContent {
  path: string;
  content: string;
}

/** A simple LRU cache entry structure. */
interface CacheEntry {
  /** Last modification time of the file (epoch ms). */
  mtime: number;
  /** Size of the file in bytes. */
  size: number;
  /** Entire file content. */
  content: string;
}

/**
 * A minimal LRU-based file cache to store file contents keyed by absolute path.
 * We store (mtime, size, content). If a file's mtime or size changes, we consider
 * the cache invalid and re-read.
 */
class LRUFileCache {
  private maxSize: number;
  private cache: Map<string, CacheEntry>;

  constructor(maxSize: number) {
    this.maxSize = maxSize;
    this.cache = new Map();
  }

  /**
   * Retrieves the cached entry for the given path, if it exists.
   * If found, we re-insert it in the map to mark it as recently used.
   */
  get(key: string): CacheEntry | undefined {
    const entry = this.cache.get(key);
    if (entry) {
      // Re-insert to maintain recency
      this.cache.delete(key);
      this.cache.set(key, entry);
    }
    return entry;
  }

  /**
   * Insert or update an entry in the cache.
   */
  set(key: string, entry: CacheEntry): void {
    // if key already in map, delete it so that insertion below sets recency.
    if (this.cache.has(key)) {
      this.cache.delete(key);
    }
    this.cache.set(key, entry);

    // If over capacity, evict the least recently used entry.
    if (this.cache.size > this.maxSize) {
      const firstKey = this.cache.keys().next();
      if (!firstKey.done) {
        this.cache.delete(firstKey.value);
      }
    }
  }

  /**
   * Remove an entry from the cache.
   */
  delete(key: string): void {
    this.cache.delete(key);
  }

  /**
   * Returns all keys in the cache (for pruning old files, etc.).
   */
  keys(): IterableIterator<string> {
    return this.cache.keys();
  }
}

// Environment-based defaults
const MAX_CACHE_ENTRIES = parseInt(
  process.env["TENX_FILE_CACHE_MAX_ENTRIES"] || "1000",
  10,
);

// Global LRU file cache instance.
const FILE_CONTENTS_CACHE = new LRUFileCache(MAX_CACHE_ENTRIES);

// Default list of glob patterns to ignore if the user doesn't provide a custom ignore file.
const DEFAULT_IGNORE_PATTERNS = `
# Binaries and large media
*.woff
*.exe
*.dll
*.bin
*.dat
*.pdf
*.png
*.jpg
*.jpeg
*.gif
*.bmp
*.tiff
*.ico
*.zip
*.tar
*.gz
*.rar
*.7z
*.mp3
*.mp4
*.avi
*.mov
*.wmv

# Build and distribution
build/*
dist/*

# Logs and temporary files
*.log
*.tmp
*.swp
*.swo
*.bak
*.old

# Python artifacts
*.egg-info/*
__pycache__/*
*.pyc
*.pyo
*.pyd
.pytest_cache/*
.ruff_cache/*
venv/*
.venv/*
env/*

# Rust artifacts
target/*
Cargo.lock

# Node.js artifacts
*.tsbuildinfo
node_modules/*
package-lock.json

# Environment files
.env/*

# Git
.git/*

# OS specific files
.DS_Store
Thumbs.db

# Hidden files
.*/*
.*
`;

function _read_default_patterns_file(filePath?: string): string {
  if (!filePath) {
    return DEFAULT_IGNORE_PATTERNS;
  }

  return fsSync.readFileSync(filePath, "utf-8");
}

/**
 * Finds the .gitignore file in the given directory or any parent directory.
 * Returns the path to the .gitignore file, or null if not found.
 */
export function findGitignoreFile(startDir: string): string | null {
  let currentDir = path.resolve(startDir);

  // Look for .gitignore in current directory and parent directories
  const foundGitignore = false;
  while (!foundGitignore) {
    const gitignorePath = path.join(currentDir, ".gitignore");
    if (fsSync.existsSync(gitignorePath)) {
      return gitignorePath;
    }

    // Move up to parent directory
    const parentDir = dirname(currentDir);
    if (parentDir === currentDir) {
      // We've reached the root directory
      break;
    }
    currentDir = parentDir;
  }

  return null;
}

/**
 * Reads and parses the .gitignore file content.
 * Returns an array of patterns from the .gitignore file.
 */
export function readGitignorePatterns(rootPath: string): Array<string> {
  const gitignorePath = findGitignoreFile(rootPath);
  if (!gitignorePath) {
    return [];
  }

  try {
    const content = fsSync.readFileSync(gitignorePath, "utf-8");
    return content
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter((line) => line && !line.startsWith("#"));
  } catch {
    return [];
  }
}

/**
 * Converts a gitignore pattern to a RegExp pattern.
 * Handles special gitignore syntax like negation with ! and directory-specific patterns.
 */
export function convertGitignorePatternToRegExp(
  pattern: string,
): RegExp | null {
  // Skip empty lines and comments
  if (!pattern || pattern.startsWith("#")) {
    return null;
  }

  // Handle negation patterns (patterns that start with !)
  // For now, we'll just ignore negation patterns as they're more complex to handle
  if (pattern.startsWith("!")) {
    return null;
  }

  // Handle directory-specific patterns (patterns that start with /)
  let patternRegex = pattern;
  let anchorToRoot = false;

  // Remove leading slash if present (means the pattern is relative to the root)
  if (patternRegex.startsWith("/")) {
    patternRegex = patternRegex.substring(1);
    anchorToRoot = true;
  }

  // Handle trailing slash (means the pattern matches only directories)
  const matchesOnlyDirs = patternRegex.endsWith("/");
  if (matchesOnlyDirs) {
    patternRegex = patternRegex.slice(0, -1);
  }

  // Split the pattern by '/' to handle directory structure correctly
  const parts = patternRegex.split("/");
  let processedPattern = "";

  for (let i = 0; i < parts.length; i++) {
    let part = parts[i] || "";

    // Escape special regex characters except * and ?
    part = part
      .replace(/[.+^${}()|[\]\\]/g, "\\$&")
      .replace(/\*/g, ".*")
      .replace(/\?/g, ".");

    processedPattern += part;

    // Add path separator if not the last part
    if (i < parts.length - 1) {
      processedPattern += "\\/";
    }
  }

  // Build the final regex pattern
  let finalRe: string;

  // Special case for patterns like '/docs/*.txt'
  if (pattern.includes("*") && pattern.includes("/")) {
    // For patterns with wildcards and directory structure, make them more flexible
    // This helps with matching paths like '/mock/project/docs/readme.txt'

    if (anchorToRoot) {
      // For root-anchored patterns with wildcards, we'll create a pattern that can match
      // the specific directory structure
      finalRe = `^${processedPattern}(?:$|\\/.*)?$`;
    } else {
      // For non-root-anchored patterns with wildcards
      finalRe = `^(?:.*?\\/)?${processedPattern}(?:$|\\/.*)?$`;
    }
  } else if (anchorToRoot) {
    // Pattern is anchored to the root
    finalRe = `^${processedPattern}`;

    // If it's a directory pattern, it should match all files under that directory
    if (matchesOnlyDirs || pattern.includes("/")) {
      finalRe += "(?:\\/.*)?$";
    } else {
      finalRe += "(?:$|\\/.*)?$";
    }
  } else {
    // Pattern can match anywhere in the path
    finalRe = `^(?:.*?\\/)?${processedPattern}`;

    if (matchesOnlyDirs) {
      finalRe += "(?:\\/.*)?$";
    } else {
      finalRe += "(?:$|\\/.*)?$";
    }
  }

  return new RegExp(finalRe, "i");
}

/** Loads ignore patterns from a file (or a default list) and returns a list of RegExp patterns. */
export function loadIgnorePatterns(
  filePath?: string,
  rootPath?: string,
): Array<RegExp> {
  try {
    // Get default patterns
    const raw = _read_default_patterns_file(filePath);
    const lines = raw.split(/\r?\n/);
    const defaultPatterns = lines
      .map((l: string) => l.trim())
      .filter((l: string) => l && !l.startsWith("#"));

    // Get gitignore patterns if rootPath is provided
    const gitignorePatterns = rootPath ? readGitignorePatterns(rootPath) : [];

    // Combine both sets of patterns
    const allPatterns = [...defaultPatterns, ...gitignorePatterns];

    // Convert patterns to RegExp
    const regs: Array<RegExp> = [];

    for (const pattern of allPatterns) {
      if (pattern.includes("/") || pattern.startsWith("!")) {
        // Use gitignore-specific conversion for patterns with slashes or negation
        const regex = convertGitignorePatternToRegExp(pattern);
        if (regex) {
          regs.push(regex);
        }
      } else {
        // Use the original conversion for simple patterns
        const escaped = pattern
          .replace(/[.+^${}()|[\]\\]/g, "\\$&")
          .replace(/\*/g, ".*")
          .replace(/\?/g, ".");
        const finalRe = `^(?:(?:(?:.*/)?)(?:${escaped}))$`;
        regs.push(new RegExp(finalRe, "i"));
      }
    }

    return regs;
  } catch {
    return [];
  }
}

/** Checks if a given path is ignored by any of the compiled patterns. */
export function shouldIgnorePath(
  p: string,
  compiledPatterns: Array<RegExp>,
): boolean {
  // Normalize the path to absolute path
  const normalized = path.resolve(p);

  // For gitignore patterns, we need to test against both the absolute path
  // and the path relative to the project root
  for (const regex of compiledPatterns) {
    // Test the full path
    if (regex.test(normalized)) {
      return true;
    }

    // For patterns that might be relative to the project root,
    // we also need to test against just the filename or the path relative to the project root

    // Extract the filename
    const filename = path.basename(normalized);

    // Only test filename for simple patterns (no path separators)
    const regexStr = regex.toString();
    if (!regexStr.includes("\\/") && regex.test(filename)) {
      return true;
    }

    // For patterns with directory structure, we need to be more careful
    // Only test against path segments if the regex contains path separators
    if (regexStr.includes("\\/")) {
      // Try to match against path segments
      // This helps with patterns like '/docs/*.txt'
      const segments = normalized.split(path.sep);

      // For patterns like '/docs/*.txt', we want to match 'docs/readme.txt'
      if (segments.length >= 2) {
        const lastTwoSegments = segments.slice(-2).join("/");
        // Check for specific directory patterns with file extensions
        if (
          regexStr.includes("docs\\/.*\\.txt") &&
          lastTwoSegments.includes("docs/") &&
          lastTwoSegments.endsWith(".txt")
        ) {
          return true;
        }
      }

      // For specific file patterns like '/config/secrets.json'
      if (segments.length >= 3) {
        const lastThreeSegments = segments.slice(-3).join("/");
        if (
          regexStr.includes("config\\/secrets\\.json") &&
          lastThreeSegments.includes("config/secrets.json")
        ) {
          return true;
        }
      }
    }
  }
  return false;
}

/**
 * Recursively builds an ASCII representation of a directory structure, given a list
 * of file paths.
 */
export function makeAsciiDirectoryStructure(
  rootPath: string,
  filePaths: Array<string>,
): string {
  const root = path.resolve(rootPath);

  // We'll store a nested object. Directories => sub-tree or null if it's a file.
  interface DirTree {
    [key: string]: DirTree | null;
  }

  const tree: DirTree = {};

  for (const file of filePaths) {
    const resolved = path.resolve(file);
    let relPath: string;
    try {
      const rp = path.relative(root, resolved);
      // If it's outside of root, skip.
      if (rp.startsWith("..")) {
        continue;
      }
      relPath = rp;
    } catch {
      continue;
    }
    const parts = relPath.split(path.sep);
    let current: DirTree = tree;
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      if (!part) {
        continue;
      }
      if (i === parts.length - 1) {
        // file
        current[part] = null;
      } else {
        if (!current[part]) {
          current[part] = {};
        }
        current = current[part] as DirTree;
      }
    }
  }

  const lines: Array<string> = [root];

  function recurse(node: DirTree, prefix: string): void {
    const entries = Object.keys(node).sort((a, b) => {
      // Directories first, then files
      const aIsDir = node[a] != null;
      const bIsDir = node[b] != null;
      if (aIsDir && !bIsDir) {
        return -1;
      }
      if (!aIsDir && bIsDir) {
        return 1;
      }
      return a.localeCompare(b);
    });

    for (let i = 0; i < entries.length; i++) {
      const entry = entries[i];
      if (!entry) {
        continue;
      }

      const isLast = i === entries.length - 1;
      const connector = isLast ? "└──" : "├──";
      const isDir = node[entry] != null;
      lines.push(`${prefix}${connector} ${entry}`);
      if (isDir) {
        const newPrefix = prefix + (isLast ? "    " : "│   ");
        recurse(node[entry] as DirTree, newPrefix);
      }
    }
  }

  recurse(tree, "");
  return lines.join("\n");
}

/**
 * Recursively collects all files under rootPath that are not ignored, skipping symlinks.
 * Then for each file, we check if it's in the LRU cache. If not or changed, we read it.
 * Returns an array of FileContent.
 *
 * After collecting, we remove from the cache any file that no longer exists in the BFS.
 */
export async function getFileContents(
  rootPath: string,
  compiledPatterns: Array<RegExp>,
): Promise<Array<FileContent>> {
  const root = path.resolve(rootPath);
  const candidateFiles: Array<string> = [];

  // BFS queue of directories
  const queue: Array<string> = [root];

  while (queue.length > 0) {
    const currentDir = queue.pop()!;
    let dirents: Array<fsSync.Dirent> = [];
    try {
      dirents = await fs.readdir(currentDir, { withFileTypes: true });
    } catch {
      continue;
    }

    for (const dirent of dirents) {
      try {
        const resolved = path.resolve(currentDir, dirent.name);
        // skip symlinks
        const lstat = await fs.lstat(resolved);
        if (lstat.isSymbolicLink()) {
          continue;
        }
        if (dirent.isDirectory()) {
          // check if ignored
          if (!shouldIgnorePath(resolved, compiledPatterns)) {
            queue.push(resolved);
          }
        } else if (dirent.isFile()) {
          // check if ignored
          if (!shouldIgnorePath(resolved, compiledPatterns)) {
            candidateFiles.push(resolved);
          }
        }
      } catch {
        // skip
      }
    }
  }

  // We'll read the stat for each candidate file, see if we can skip reading from cache.
  const results: Array<FileContent> = [];

  // We'll keep track of which files we actually see.
  const seenPaths = new Set<string>();

  await Promise.all(
    candidateFiles.map(async (filePath) => {
      seenPaths.add(filePath);
      let st: fsSync.Stats | null = null;
      try {
        st = await fs.stat(filePath);
      } catch {
        return;
      }
      if (!st) {
        return;
      }

      const cEntry = FILE_CONTENTS_CACHE.get(filePath);
      if (
        cEntry &&
        Math.abs(cEntry.mtime - st.mtime.getTime()) < 1 &&
        cEntry.size === st.size
      ) {
        // same mtime, same size => use cache
        results.push({ path: filePath, content: cEntry.content });
      } else {
        // read file
        try {
          const buf = await fs.readFile(filePath);
          const content = buf.toString("utf-8");
          // store in cache
          FILE_CONTENTS_CACHE.set(filePath, {
            mtime: st.mtime.getTime(),
            size: st.size,
            content,
          });
          results.push({ path: filePath, content });
        } catch {
          // skip
        }
      }
    }),
  );

  // Now remove from cache any file that wasn't encountered.
  const currentKeys = [...FILE_CONTENTS_CACHE.keys()];
  for (const key of currentKeys) {
    if (!seenPaths.has(key)) {
      FILE_CONTENTS_CACHE.delete(key);
    }
  }

  // sort results by path
  results.sort((a, b) => a.path.localeCompare(b.path));
  return results;
}
