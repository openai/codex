export type WorkspaceRoot = {
  id: string;
  label: string;
  absPath: string;
  createdAt: string;
  order: number;
};

export type Workspace = {
  version: number;
  roots: WorkspaceRoot[];
};

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

export async function getTree(rootId: string, path: string): Promise<TreeEntry[]> {
  const qp = new URLSearchParams({ root: rootId, path });
  return await http(`/api/tree?${qp.toString()}`);
}

export async function getFileText(rootId: string, path: string): Promise<{ text: string }> {
  const qp = new URLSearchParams({ root: rootId, path });
  return await http(`/api/file?${qp.toString()}`);
}

export async function getBrowserHome(): Promise<{ home: string }> {
  return await http("/api/browser/home");
}

export async function browserList(path: string): Promise<BrowserListResponse> {
  const qp = new URLSearchParams({ path });
  return await http(`/api/browser/list?${qp.toString()}`);
}
