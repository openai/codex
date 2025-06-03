import { existsSync, readFileSync, writeFileSync, mkdirSync } from "fs";
import { homedir } from "os";
import { join } from "path";

export interface Snippet {
  label: string;
  code: string;
  created_at: string;
}

export interface SnippetStorageResult {
  success: boolean;
  message?: string;
  snippet?: Snippet;
  snippets?: Array<Snippet>;
}

/**
 * Get the path to the snippets storage directory
 */
function getSnippetsDir(): string {
  return join(homedir(), ".codex");
}

/**
 * Get the path to the snippets.json file
 */
function getSnippetsFilePath(): string {
  return join(getSnippetsDir(), "snippets.json");
}

/**
 * Ensure the snippets directory exists
 */
function ensureSnippetsDir(): void {
  const dir = getSnippetsDir();
  if (!existsSync(dir)) {
    mkdirSync(dir, { recursive: true });
  }
}

/**
 * Load all snippets from storage
 */
function loadSnippets(): Array<Snippet> {
  const filePath = getSnippetsFilePath();

  if (!existsSync(filePath)) {
    return [];
  }

  try {
    const content = readFileSync(filePath, "utf8");
    const snippets = JSON.parse(content);
    return Array.isArray(snippets) ? snippets : [];
  } catch (error) {
    // Failed to load snippets, return empty array
    return [];
  }
}

/**
 * Save snippets to storage
 */
function saveSnippets(snippets: Array<Snippet>): boolean {
  try {
    ensureSnippetsDir();
    const filePath = getSnippetsFilePath();
    const content = JSON.stringify(snippets, null, 2);
    writeFileSync(filePath, content, "utf8");
    return true;
  } catch (error) {
    // Failed to save snippets
    return false;
  }
}

/**
 * Add a new snippet
 */
export function addSnippet(label: string, code: string): SnippetStorageResult {
  if (!label || !code) {
    return {
      success: false,
      message: "Label and code are required",
    };
  }

  // Validate label format (no spaces, special characters)
  if (!/^[a-zA-Z0-9_-]+$/.test(label)) {
    return {
      success: false,
      message:
        "Label must contain only letters, numbers, underscores, and hyphens",
    };
  }

  const snippets = loadSnippets();

  // Check if label already exists
  const existingIndex = snippets.findIndex((s) => s.label === label);

  const newSnippet: Snippet = {
    label,
    code,
    created_at: new Date().toISOString(),
  };

  if (existingIndex >= 0) {
    // Update existing snippet
    snippets[existingIndex] = newSnippet;
  } else {
    // Add new snippet
    snippets.push(newSnippet);
  }

  const saved = saveSnippets(snippets);

  if (saved) {
    return {
      success: true,
      message:
        existingIndex >= 0
          ? `Updated snippet "${label}"`
          : `Added snippet "${label}"`,
      snippet: newSnippet,
    };
  } else {
    return {
      success: false,
      message: "Failed to save snippet",
    };
  }
}

/**
 * Get a specific snippet by label
 */
export function getSnippet(label: string): SnippetStorageResult {
  if (!label) {
    return {
      success: false,
      message: "Label is required",
    };
  }

  const snippets = loadSnippets();
  const snippet = snippets.find((s) => s.label === label);

  if (snippet) {
    return {
      success: true,
      snippet,
    };
  } else {
    return {
      success: false,
      message: `Snippet "${label}" not found`,
    };
  }
}

/**
 * List all snippets
 */
export function listSnippets(): SnippetStorageResult {
  const snippets = loadSnippets();

  return {
    success: true,
    snippets,
  };
}

/**
 * Remove a snippet by label
 */
export function removeSnippet(label: string): SnippetStorageResult {
  if (!label) {
    return {
      success: false,
      message: "Label is required",
    };
  }

  const snippets = loadSnippets();
  const index = snippets.findIndex((s) => s.label === label);

  if (index < 0) {
    return {
      success: false,
      message: `Snippet "${label}" not found`,
    };
  }

  const removedSnippet = snippets.splice(index, 1)[0];
  const saved = saveSnippets(snippets);

  if (saved) {
    return {
      success: true,
      message: `Removed snippet "${label}"`,
      snippet: removedSnippet,
    };
  } else {
    return {
      success: false,
      message: "Failed to save changes",
    };
  }
}
