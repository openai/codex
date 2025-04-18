import * as fsSync from "fs";
import path from "path";

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
export function loadIgnorePatternsAsRegExp(filePath?: string): Array<RegExp> {
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

/** Loads ignore patterns from a file (or a default list) and returns a string of SBPL deny rules. */
export function loadIgnorePatternsAsSBPLDenyRules(
  ignoreFilePath?: string,
): string {
  try {
    const raw = _read_default_patterns_file(ignoreFilePath);
    const lines = raw.split(/\r?\n/);
    const cleaned = lines
      .map((l: string) => l.trim())
      .filter((l: string) => l && !l.startsWith("#"));

    if (cleaned.length === 0) {
      return "";
    }

    const regexPatterns = cleaned.map((pattern) => {
      const escaped = pattern
        .replace(/[.+^${}()|[\]\\]/g, "\\$&") // Escape special regex characters
        .replace(/\*/g, ".*")
        .replace(/\?/g, ".");

      return `(regex #"(^|/)${escaped}$")`;
    });

    return ["(deny file-read* file-write*", ...regexPatterns, ")"].join("\n");
  } catch {
    return "";
  }
}

/** Checks if a given path is ignored by any of the compiled patterns. */
export function shouldIgnorePath(
  p: string,
  compiledIgnorePatterns: Array<RegExp>,
): boolean {
  const normalized = path.resolve(p);
  for (const regex of compiledIgnorePatterns) {
    if (regex.test(normalized)) {
      return true;
    }
  }
  return false;
}
