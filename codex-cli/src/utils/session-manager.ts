import fs from "fs";
import path from "path";
import os from "os";
import type { TerminalChatSession } from "./session";

// Constants for session storage
const sessionsDir = path.join(os.homedir(), ".codex");
const sessionsFile = path.join(sessionsDir, "sessions.json");
const BACKUP_SESSION_FILE = path.join(sessionsDir, "sessions.backup.json");

/**
 * Ensure the sessions directory and file exist.
 * Creates the directory and empty sessions file if they don't exist.
 */
function ensureSessionsFileExists(): void {
  if (!fs.existsSync(sessionsDir)) {
    fs.mkdirSync(sessionsDir, { recursive: true });
  }

  if (!fs.existsSync(sessionsFile)) {
    fs.writeFileSync(sessionsFile, JSON.stringify([]), "utf-8");
  }
}

/**
 * Read sessions from the sessions file
 * @returns Array of TerminalChatSession objects
 */
function readSessions(): TerminalChatSession[] {
  ensureSessionsFileExists();

  try {
    const raw = fs.readFileSync(sessionsFile, "utf-8");
    const sessions = JSON.parse(raw);
    return Array.isArray(sessions) ? sessions : [];
  } catch (error) {
    // If there's an error reading the main file, try to recover from backup
    if (fs.existsSync(BACKUP_SESSION_FILE)) {
      try {
        const backupRaw = fs.readFileSync(BACKUP_SESSION_FILE, "utf-8");
        const backupSessions = JSON.parse(backupRaw);
        return Array.isArray(backupSessions) ? backupSessions : [];
      } catch {
        // If backup also fails, return empty array
        return [];
      }
    }
    return [];
  }
}

/**
 * Write sessions to the sessions file with backup
 * @param sessions Array of sessions to write
 */
function writeSessions(sessions: TerminalChatSession[]): void {
  ensureSessionsFileExists();

  // First create a backup of the current file if it exists and has content
  if (fs.existsSync(sessionsFile)) {
    try {
      const currentContent = fs.readFileSync(sessionsFile, "utf-8");
      if (currentContent.trim()) {
        fs.writeFileSync(BACKUP_SESSION_FILE, currentContent, "utf-8");
      }
    } catch {
      // Ignore backup creation errors
    }
  }

  // Now write the new content
  try {
    fs.writeFileSync(sessionsFile, JSON.stringify(sessions, null, 2), "utf-8");
  } catch (error) {
    console.error("Error writing sessions file:", error);
  }
}

/**
 * List all available sessions
 * @returns Array of TerminalChatSession objects
 */
export function listSessions(): TerminalChatSession[] {
  return readSessions();
}

/**
 * Save a session, replacing an existing one with the same ID or adding a new one
 * @param newSession The session to save
 */
export function saveSession(newSession: TerminalChatSession): void {
  const sessions = readSessions();

  // Replace an existing session if its id matches; otherwise, add new
  const index = sessions.findIndex((session) => session.id === newSession.id);
  if (index >= 0) {
    sessions[index] = newSession;
  } else {
    sessions.push(newSession);
  }

  writeSessions(sessions);
}

/**
 * Load a session by ID
 * @param sessionId The ID of the session to load
 * @returns The session if found, null otherwise
 */
export function loadSession(sessionId: string): TerminalChatSession | null {
  const sessions = readSessions();
  return sessions.find((s) => s.id === sessionId) || null;
}

/**
 * Delete a session by ID
 * @param sessionId The ID of the session to delete
 * @returns true if the session was deleted, false if it wasn't found
 */
export function deleteSession(sessionId: string): boolean {
  let sessions = readSessions();
  const initialLength = sessions.length;
  sessions = sessions.filter((s) => s.id !== sessionId);

  // Only write if something was actually removed
  if (sessions.length < initialLength) {
    writeSessions(sessions);
    return true;
  }

  return false;
}

/**
 * Create a session backup
 * Useful for emergency recovery situations
 */
export function createSessionBackup(): boolean {
  try {
    const sessions = readSessions();
    const backupPath = path.join(
      sessionsDir,
      `sessions.backup.${Date.now()}.json`,
    );
    fs.writeFileSync(backupPath, JSON.stringify(sessions, null, 2), "utf-8");
    return true;
  } catch {
    return false;
  }
}
