import * as vscode from "vscode";
import type { Personality } from "./generated/Personality";
import type { CollaborationModeMask } from "./generated/CollaborationModeMask";

export type BackendId = "codex" | "codez" | "opencode";

export type Session = {
  id: string;
  backendKey: string;
  backendId: BackendId;
  workspaceFolderUri: string;
  title: string;
  customTitle?: boolean;
  threadId: string;
  personality?: Personality | null;
  collaborationModePresetName?: CollaborationModeMask["name"] | null;
};

export class SessionStore {
  private readonly sessionsByBackendKey = new Map<string, Session[]>();
  private readonly sessionsById = new Map<string, Session>();
  private readonly sessionsByThreadKey = new Map<string, Session>();

  private threadKey(backendKey: string, threadId: string): string {
    return `${backendKey}::${threadId}`;
  }

  public list(backendKey: string): Session[] {
    return this.sessionsByBackendKey.get(backendKey) ?? [];
  }

  public listByWorkspaceFolderUri(workspaceFolderUri: string): Session[] {
    const out: Session[] = [];
    for (const sessions of this.sessionsByBackendKey.values()) {
      for (const s of sessions) {
        if (s.workspaceFolderUri === workspaceFolderUri) out.push(s);
      }
    }
    return out;
  }

  public listAll(): Session[] {
    const out: Session[] = [];
    for (const sessions of this.sessionsByBackendKey.values())
      out.push(...sessions);
    return out;
  }

  public reset(): void {
    this.sessionsByBackendKey.clear();
    this.sessionsById.clear();
    this.sessionsByThreadKey.clear();
  }

  public getById(sessionId: string): Session | null {
    return this.sessionsById.get(sessionId) ?? null;
  }

  public getByThreadId(backendKey: string, threadId: string): Session | null {
    return (
      this.sessionsByThreadKey.get(this.threadKey(backendKey, threadId)) ?? null
    );
  }

  public add(backendKey: string, session: Session): void {
    const list = this.sessionsByBackendKey.get(backendKey) ?? [];
    this.sessionsByBackendKey.set(backendKey, [...list, session]);
    this.sessionsById.set(session.id, session);
    this.sessionsByThreadKey.set(
      this.threadKey(backendKey, session.threadId),
      session,
    );
  }

  public rename(sessionId: string, title: string): Session | null {
    const session = this.sessionsById.get(sessionId) ?? null;
    if (!session) return null;
    session.title = title;
    session.customTitle = true;
    return session;
  }

  public async pick(backendKey: string): Promise<Session | null> {
    const sessions = this.list(backendKey);
    if (sessions.length === 0) return null;
    const picked = await vscode.window.showQuickPick(
      sessions.map((s) => ({
        label: s.title,
        description: s.threadId,
        session: s,
      })),
      { title: "Codex UI: Select a session" },
    );
    return picked?.session ?? null;
  }

  public remove(sessionId: string): Session | null {
    const session = this.sessionsById.get(sessionId);
    if (!session) return null;

    this.sessionsById.delete(sessionId);
    this.sessionsByThreadKey.delete(
      this.threadKey(session.backendKey, session.threadId),
    );

    const list = this.sessionsByBackendKey.get(session.backendKey) ?? [];
    const next = list.filter((s) => s.id !== sessionId);
    if (next.length === 0) this.sessionsByBackendKey.delete(session.backendKey);
    else this.sessionsByBackendKey.set(session.backendKey, next);

    return session;
  }
}
