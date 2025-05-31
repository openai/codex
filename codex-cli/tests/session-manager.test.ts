import { describe, it, expect, beforeEach, afterAll, vi } from "vitest";
import fs from "fs";
import os from "os";
import path from "path";
import {
  listSessions,
  saveSession,
  loadSession,
  deleteSession,
  createSessionBackup,
} from "../src/utils/session-manager";
import type { TerminalChatSession } from "../src/utils/session";

describe("Session Manager", () => {
  // Test session data
  const testSession: TerminalChatSession = {
    id: "test-session-id",
    user: "tester",
    version: "0.0.test",
    model: "o4-mini",
    timestamp: new Date().toISOString(),
    instructions: "Test instructions",
    cwd: "/test/path", // This will now match the type after step 1
    firstPrompt: "Test prompt", // This will now match the type after step 1
  };

  // Path to the sessions file
  const sessionsDir = path.join(os.homedir(), ".codex");
  const sessionsFile = path.join(sessionsDir, "sessions.json");
  const backupFile = path.join(sessionsDir, "sessions.backup.json");

  // Setup and teardown
  beforeEach(() => {
    // Create directory if it doesn't exist
    fs.mkdirSync(path.dirname(sessionsFile), { recursive: true });

    // Start with empty sessions file
    fs.writeFileSync(sessionsFile, JSON.stringify([]), "utf-8");

    // Clear backup if it exists
    if (fs.existsSync(backupFile)) {
      fs.unlinkSync(backupFile);
    }
  });

  afterAll(() => {
    // Cleanup - reset to empty sessions
    if (fs.existsSync(sessionsFile)) {
      fs.writeFileSync(sessionsFile, JSON.stringify([]), "utf-8");
    }

    // Remove backup file
    if (fs.existsSync(backupFile)) {
      fs.unlinkSync(backupFile);
    }
  });

  // Tests
  it("should list no sessions if none saved", () => {
    const sessions = listSessions();
    expect(sessions.length).toBe(0);
  });

  it("should save a session", () => {
    saveSession(testSession);
    const sessions = listSessions();
    expect(sessions.length).toBe(1);
    expect(sessions[0]!.id).toBe(testSession.id); // Added !
    expect(sessions[0]!.user).toBe("tester"); // Added !
  });

  it("should create a backup when saving a session", () => {
    // Save initial session
    saveSession(testSession);

    // Verify main file exists
    expect(fs.existsSync(sessionsFile)).toBe(true);

    // Save another session to trigger backup creation
    const secondSession = { ...testSession, id: "second-id" };
    saveSession(secondSession);

    // Check that backup file exists
    expect(fs.existsSync(backupFile)).toBe(true);

    // Verify backup content
    const backupContent = fs.readFileSync(backupFile, "utf-8");
    const backupSessions = JSON.parse(backupContent);
    expect(backupSessions.length).toBe(1);
    expect(backupSessions[0].id).toBe(testSession.id); // No ! needed here as backupSessions is `any` from JSON.parse
  });

  it("should load an existing session", () => {
    saveSession(testSession);
    const loaded = loadSession(testSession.id);
    expect(loaded).not.toBeNull();
    expect(loaded?.user).toBe("tester"); // Using optional chaining here is fine
    expect(loaded?.model).toBe("o4-mini");
    expect(loaded?.instructions).toBe("Test instructions");
  });

  it("should return null when loading a non-existent session", () => {
    const loaded = loadSession("non-existent-id");
    expect(loaded).toBeNull();
  });

  it("should delete an existing session", () => {
    saveSession(testSession);
    const deleted = deleteSession(testSession.id);
    expect(deleted).toBe(true);
    const sessions = listSessions();
    expect(sessions.length).toBe(0);
  });

  it("should return false when deleting a non-existent session", () => {
    const deleted = deleteSession("non-existent-id");
    expect(deleted).toBe(false);
  });

  it("should update an existing session", () => {
    saveSession(testSession);
    const updatedSession = {
      ...testSession,
      model: "updated-model",
      timestamp: new Date().toISOString(),
    };
    saveSession(updatedSession);
    const sessions = listSessions();
    expect(sessions.length).toBe(1);
    expect(sessions[0]!.model).toBe("updated-model"); // Added !
  });

  it("should recover from corrupt session file", () => {
    // Save a valid session
    saveSession(testSession);

    // Force a backup to be created
    const secondSession = { ...testSession, id: "second-id" };
    saveSession(secondSession);

    // Corrupt the main file
    fs.writeFileSync(sessionsFile, "this is not valid JSON", "utf-8");

    // Should recover from backup
    const sessions = listSessions();
    expect(sessions.length).toBe(1); // Backup only had the first session
    expect(sessions[0]!.id).toBe(testSession.id); // Added !
  });

  it("should create a named backup with createSessionBackup", () => {
    saveSession(testSession);

    // Mock Date.now() to return a fixed timestamp
    const mockNow = 1234567890;
    const originalNow = Date.now;
    Date.now = vi.fn(() => mockNow);

    // Create backup
    const result = createSessionBackup();
    expect(result).toBe(true);

    // Check backup file exists
    const expectedBackupPath = path.join(
      sessionsDir,
      `sessions.backup.${mockNow}.json`,
    );
    expect(fs.existsSync(expectedBackupPath)).toBe(true);

    // Cleanup
    if (fs.existsSync(expectedBackupPath)) {
      fs.unlinkSync(expectedBackupPath);
    }
    Date.now = originalNow;
  });
});