/* eslint-disable no-await-in-loop */

import * as fsSync from "fs";
import fs from "fs/promises";
import path from "path";

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

/** Loads ignore patterns from a file (or a default list) and returns a list of RegExp patterns. */
export function loadIgnorePatterns(filePath?: string): Array<RegExp> {
  try {
    const raw = _read_default_patterns_file(filePath);
    const lines = raw.split(/\r?\n/);
    const cleaned = lines
      .map((l: string) => l.trim())
      .filter((l: string) => l && !l.startsWith("#"));

    // Convert each pattern to a RegExp with a leading '*/'.
    const regs = cleaned.map((pattern: string) => {
      const escaped = pattern
        .replace(/[.+^${}()|[\]\\]/g, "\\$&")
        .replace(/\*/g, ".*")
        .replace(/\?/g, ".");
      const finalRe = `^(?:(?:(?:.*/)?)(?:${escaped}))$`;
      return new RegExp(finalRe, "i");
    });
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
  const normalized = path.resolve(p);
  for (const regex of compiledPatterns) {
    if (regex.test(normalized)) {
      return true;
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
