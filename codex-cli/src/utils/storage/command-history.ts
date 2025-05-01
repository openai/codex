import { log } from "../logger/log.js";
import { existsSync } from "fs";
import fs from "fs/promises";
import os from "os";
import path from "path";

const HISTORY_FILE = path.join(os.homedir(), ".codex", "history.json");
const DEFAULT_HISTORY_SIZE = 10_000;

// Regex patterns for sensitive commands that should not be saved.
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
  sensitivePatterns: Array<string>; // Regex patterns.
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

export async function loadCommandHistory(): Promise<Array<HistoryEntry>> {
  try {
    if (!existsSync(HISTORY_FILE)) {
      return [];
    }

    const data = await fs.readFile(HISTORY_FILE, "utf-8");
    const history = JSON.parse(data) as Array<HistoryEntry>;
    return Array.isArray(history) ? history : [];
  } catch (error) {
    log(`error: failed to load command history: ${error}`);
    return [];
  }
}

export async function saveCommandHistory(
  history: Array<HistoryEntry>,
  config: HistoryConfig = DEFAULT_HISTORY_CONFIG,
): Promise<void> {
  try {
    // Create directory if it doesn't exist.
    const dir = path.dirname(HISTORY_FILE);
    await fs.mkdir(dir, { recursive: true });

    // Trim history to max size.
    const trimmedHistory = history.slice(-config.maxSize);

    await fs.writeFile(
      HISTORY_FILE,
      JSON.stringify(trimmedHistory, null, 2),
      "utf-8",
    );
  } catch (error) {
    log(`error: failed to save command history: ${error}`);
  }
}

export async function addToHistory(
  command: string,
  history: Array<HistoryEntry>,
  config: HistoryConfig = DEFAULT_HISTORY_CONFIG,
): Promise<Array<HistoryEntry>> {
  if (!config.saveHistory || command.trim() === "") {
    return history;
  }

  // Skip commands with sensitive information.
  if (commandHasSensitiveInfo(command, config.sensitivePatterns)) {
    return history;
  }

  // Check for duplicate (don't add if it's the same as the last command).
  const lastEntry = history[history.length - 1];
  if (lastEntry && lastEntry.command === command) {
    return history;
  }

  // Add new entry.
  const newHistory: Array<HistoryEntry> = [
    ...history,
    {
      command,
      timestamp: Date.now(),
    },
  ];
  await saveCommandHistory(newHistory, config);
  return newHistory;
}

function commandHasSensitiveInfo(
  command: string,
  additionalPatterns: Array<string> = [],
): boolean {
  // Check built-in patterns.
  for (const pattern of SENSITIVE_PATTERNS) {
    if (pattern.test(command)) {
      return true;
    }
  }

  // Check additional patterns from config.
  for (const patternStr of additionalPatterns) {
    try {
      const pattern = new RegExp(patternStr);
      if (pattern.test(command)) {
        return true;
      }
    } catch (error) {
      // Invalid regex pattern, skip it.
    }
  }

  return false;
}

export async function clearCommandHistory(): Promise<void> {
  try {
    if (existsSync(HISTORY_FILE)) {
      await fs.writeFile(HISTORY_FILE, JSON.stringify([]), "utf-8");
    }
  } catch (error) {
    log(`error: failed to clear command history: ${error}`);
  }
}
