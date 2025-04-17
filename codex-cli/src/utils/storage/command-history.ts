import { existsSync } from "fs";
import fs from "fs/promises";
import os from "os";
import path from "path";

const HISTORY_FILE = path.join(os.homedir(), ".codex", "history.json");
const DEFAULT_HISTORY_SIZE = 1000;

// Regex patterns for sensitive commands that should not be saved
const SENSITIVE_PATTERNS = [
  /\b[A-Za-z0-9-_]{20,}\b/, // API keys and tokens
  /\bpassword\b/i,
  /\bsecret\b/i,
  /\btoken\b/i,
  /\bkey\b/i,
];

export interface HistoryConfig {
  maxSize: number;
  saveHistory: boolean;
  sensitivePatterns: Array<string>; // Array of regex patterns as strings
}

export interface HistoryEntry {
  command: string;
  timestamp: number;
}

export const DEFAULT_HISTORY_CONFIG: HistoryConfig = {
  maxSize: DEFAULT_HISTORY_SIZE,
  saveHistory: true,
  sensitivePatterns: [],
};

/**
 * Loads command history from the history file
 */
export async function loadCommandHistory(): Promise<Array<HistoryEntry>> {
  try {
    if (!existsSync(HISTORY_FILE)) {
      return [];
    }

    const data = await fs.readFile(HISTORY_FILE, "utf-8");
    const history = JSON.parse(data) as Array<HistoryEntry>;
    return Array.isArray(history) ? history : [];
  } catch (error) {
    // Use error logger but for production would use a proper logging system
    // eslint-disable-next-line no-console
    console.error("Failed to load command history:", error);
    return [];
  }
}

/**
 * Saves command history to the history file
 */
export async function saveCommandHistory(
  history: Array<HistoryEntry>,
  config: HistoryConfig = DEFAULT_HISTORY_CONFIG,
): Promise<void> {
  try {
    // Create directory if it doesn't exist
    const dir = path.dirname(HISTORY_FILE);
    await fs.mkdir(dir, { recursive: true });

    // Trim history to max size
    const trimmedHistory = history.slice(-config.maxSize);

    await fs.writeFile(
      HISTORY_FILE,
      JSON.stringify(trimmedHistory, null, 2),
      "utf-8",
    );
  } catch (error) {
    // eslint-disable-next-line no-console
    console.error("Failed to save command history:", error);
  }
}

/**
 * Adds a command to history if it's not sensitive
 */
export async function addToHistory(
  command: string,
  history: Array<HistoryEntry>,
  config: HistoryConfig = DEFAULT_HISTORY_CONFIG,
): Promise<Array<HistoryEntry>> {
  if (!config.saveHistory || command.trim() === "") {
    return history;
  }

  // Check if command contains sensitive information
  if (isSensitiveCommand(command, config.sensitivePatterns)) {
    return history;
  }

  // Check for duplicate (don't add if it's the same as the last command)
  const lastEntry = history[history.length - 1];
  if (lastEntry && lastEntry.command === command) {
    return history;
  }

  // Add new entry
  const newEntry: HistoryEntry = {
    command,
    timestamp: Date.now(),
  };

  const newHistory = [...history, newEntry];

  // Save to file
  await saveCommandHistory(newHistory, config);

  return newHistory;
}

/**
 * Checks if a command contains sensitive information
 */
function isSensitiveCommand(
  command: string,
  additionalPatterns: Array<string> = [],
): boolean {
  // Check built-in patterns
  for (const pattern of SENSITIVE_PATTERNS) {
    if (pattern.test(command)) {
      return true;
    }
  }

  // Check additional patterns from config
  for (const patternStr of additionalPatterns) {
    try {
      const pattern = new RegExp(patternStr);
      if (pattern.test(command)) {
        return true;
      }
    } catch (error) {
      // Invalid regex pattern, skip it
    }
  }

  return false;
}

/**
 * Clears the command history
 */
export async function clearCommandHistory(): Promise<void> {
  try {
    if (existsSync(HISTORY_FILE)) {
      await fs.writeFile(HISTORY_FILE, JSON.stringify([]), "utf-8");
    }
  } catch (error) {
    // eslint-disable-next-line no-console
    console.error("Failed to clear command history:", error);
  }
}
