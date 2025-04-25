import { randomUUID } from "crypto";
import { createSessionStore } from "./json-store";

// Session interface
export interface Session {
  id: string;
  name: string;
  target?: string;
  created_at: string;
  updated_at: string;
}

// Command history interface
export interface CommandHistory {
  id: string;
  session_id: string;
  command: string;
  output: string;
  exit_code: number;
  timestamp: string;
}

// Create stores
const sessionStore = createSessionStore<Session>();
const historyStore = createSessionStore<CommandHistory>();

/**
 * Create a new security session
 */
export function createSession(
  name: string,
  target?: string
): Session {
  const id = randomUUID();
  const now = new Date().toISOString();
  
  const session: Session = {
    id,
    name,
    target,
    created_at: now,
    updated_at: now
  };
  
  sessionStore.save(session);
  
  return session;
}

/**
 * Get a session by ID
 */
export function getSession(id: string): Session | null {
  return sessionStore.get(id);
}

/**
 * Get a session by name
 */
export function getSessionByName(name: string): Session | null {
  const sessions = sessionStore.getAll();
  return sessions.find(session => session.name === name) || null;
}

/**
 * List all sessions
 */
export function listSessions(): Session[] {
  const sessions = sessionStore.getAll();
  return sessions.sort((a, b) => 
    new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime()
  );
}

/**
 * Update session last updated time
 */
export function updateSessionTimestamp(id: string): void {
  const session = sessionStore.get(id);
  
  if (session) {
    session.updated_at = new Date().toISOString();
    sessionStore.save(session);
  }
}

/**
 * Record a command in the session history
 */
export function recordCommand(
  sessionId: string,
  command: string,
  output: string,
  exitCode: number
): void {
  const id = randomUUID();
  const timestamp = new Date().toISOString();
  
  const commandHistory: CommandHistory = {
    id,
    session_id: sessionId,
    command,
    output,
    exit_code: exitCode,
    timestamp
  };
  
  historyStore.save(commandHistory);
  
  // Update session timestamp
  updateSessionTimestamp(sessionId);
}

/**
 * Get command history for a session
 */
export function getCommandHistory(sessionId: string): CommandHistory[] {
  const allHistory = historyStore.getAll();
  
  return allHistory
    .filter(cmd => cmd.session_id === sessionId)
    .sort((a, b) => 
      new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime()
    );
} 