export type WorkspaceRoot = {
  id: string;
  label: string;
  absPath: string;
  cliCommand: "codex" | "codex-mine";
  createdAt: string;
  order: number;
};

export type WorkspaceSettings = {
  cliCommand: "codex" | "codex-mine";
};

export type Workspace = {
  version: number;
  roots: WorkspaceRoot[];
  settings?: WorkspaceSettings | null;
};

export type ChatSession = {
  id: string;
  title: string;
  threadId: string | null;
};

export type ChatState = {
  sessions: ChatSession[];
  activeSessionId: string | null;
};

export type ChatMessage =
  | { id: string; role: "user"; text: string }
  | { id: string; role: "assistant"; text: string }
  | { id: string; role: "meta"; text: string };

export type TreeEntry = {
  name: string;
  kind: "dir" | "file" | "symlink" | "other";
  path: string; // root内の POSIX 形式 (例: "/src/main.ts")
  size?: number;
  mtimeMs?: number;
};

export type BrowserListResponse = {
  path: string; // absolute
  entries: Array<{ name: string; absPath: string }>;
};

async function http<T>(input: RequestInfo, init?: RequestInit): Promise<T> {
  const res = await fetch(input, init);
  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw new Error(`HTTP ${res.status}: ${text || res.statusText}`);
  }
  return (await res.json()) as T;
}

export async function getWorkspace(): Promise<Workspace> {
  return await http("/api/workspace");
}

export async function patchWorkspaceSettings(settings: Partial<WorkspaceSettings>) {
  return await http("/api/workspace/settings", {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(settings),
  });
}

export async function addRoot(args: { absPath: string; label?: string }) {
  return await http("/api/workspace/roots", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(args),
  });
}

export async function removeRoot(rootId: string) {
  const res = await fetch(`/api/workspace/roots/${encodeURIComponent(rootId)}`, {
    method: "DELETE",
  });
  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw new Error(`HTTP ${res.status}: ${text || res.statusText}`);
  }
}

export async function renameRoot(rootId: string, label: string) {
  return await http(`/api/workspace/roots/${encodeURIComponent(rootId)}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ label }),
  });
}

export async function setRootCliCommand(
  rootId: string,
  cliCommand: WorkspaceRoot["cliCommand"],
) {
  return await http(`/api/workspace/roots/${encodeURIComponent(rootId)}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ cliCommand }),
  });
}

export async function getTree(rootId: string, path: string): Promise<TreeEntry[]> {
  const qp = new URLSearchParams({ root: rootId, path });
  return await http(`/api/tree?${qp.toString()}`);
}

export async function getFileText(rootId: string, path: string): Promise<{ text: string }> {
  const qp = new URLSearchParams({ root: rootId, path });
  return await http(`/api/file?${qp.toString()}`);
}

export async function getChatState(rootId: string): Promise<ChatState> {
  const qp = new URLSearchParams({ root: rootId });
  return await http(`/api/chat/state?${qp.toString()}`);
}

export async function createChatSession(rootId: string, title?: string) {
  return await http("/api/chat/session", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ rootId, title }),
  });
}

export async function setChatActiveSession(rootId: string, activeSessionId: string | null) {
  return await http("/api/chat/state", {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ rootId, activeSessionId }),
  });
}

export async function updateChatSession(args: {
  rootId: string;
  sessionId: string;
  title?: string;
  threadId?: string | null;
}) {
  return await http("/api/chat/session", {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(args),
  });
}

export async function getChatMessages(rootId: string, sessionId: string): Promise<{ messages: ChatMessage[] }> {
  const qp = new URLSearchParams({ root: rootId, session: sessionId });
  return await http(`/api/chat/messages?${qp.toString()}`);
}

export async function putChatMessages(args: {
  rootId: string;
  sessionId: string;
  messages: ChatMessage[];
}) {
  return await http("/api/chat/messages", {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(args),
  });
}

export async function getBrowserHome(): Promise<{ home: string }> {
  return await http("/api/browser/home");
}

export async function browserList(path: string): Promise<BrowserListResponse> {
  const qp = new URLSearchParams({ path });
  return await http(`/api/browser/list?${qp.toString()}`);
}
