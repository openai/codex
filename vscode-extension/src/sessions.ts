import * as vscode from "vscode";

export type Session = {
  id: string;
  backendKey: string;
  workspaceFolderUri: string;
  title: string;
  threadId: string;
};

export class SessionStore {
  private readonly sessionsByBackendKey = new Map<string, Session[]>();
  private readonly sessionsById = new Map<string, Session>();
  private readonly sessionsByThreadId = new Map<string, Session>();

  public list(backendKey: string): Session[] {
    return this.sessionsByBackendKey.get(backendKey) ?? [];
  }

  public listAll(): Session[] {
    const out: Session[] = [];
    for (const sessions of this.sessionsByBackendKey.values())
      out.push(...sessions);
    return out;
  }

  public getById(sessionId: string): Session | null {
    return this.sessionsById.get(sessionId) ?? null;
  }

  public getByThreadId(threadId: string): Session | null {
    return this.sessionsByThreadId.get(threadId) ?? null;
  }

  public add(backendKey: string, session: Session): void {
    const list = this.sessionsByBackendKey.get(backendKey) ?? [];
    this.sessionsByBackendKey.set(backendKey, [...list, session]);
    this.sessionsById.set(session.id, session);
    this.sessionsByThreadId.set(session.threadId, session);
  }

  public rename(sessionId: string, title: string): Session | null {
    const session = this.sessionsById.get(sessionId) ?? null;
    if (!session) return null;
    session.title = title;
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
    this.sessionsByThreadId.delete(session.threadId);

    const list = this.sessionsByBackendKey.get(session.backendKey) ?? [];
    const next = list.filter((s) => s.id !== sessionId);
    if (next.length === 0) this.sessionsByBackendKey.delete(session.backendKey);
    else this.sessionsByBackendKey.set(session.backendKey, next);

    return session;
  }
}
