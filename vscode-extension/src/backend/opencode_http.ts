import * as path from "node:path";
import type { SkillMetadata } from "../generated/v2/SkillMetadata";
import type { SkillScope } from "../generated/v2/SkillScope";
import type { SkillsListEntry } from "../generated/v2/SkillsListEntry";
import type { Model } from "../generated/v2/Model";
import type { FuzzyFileSearchResponse } from "../generated/FuzzyFileSearchResponse";

type Json = Record<string, unknown>;

export type OpencodeHttpOptions = {
  baseUrl: URL;
  directory: string;
};

export type OpencodeSessionInfo = {
  id: string;
  title: string;
  directory: string;
  time?: { created?: number; updated?: number };
};

export type OpencodeMessageWithParts = {
  info: { id: string; role: "user" | "assistant" } & Record<string, unknown>;
  parts: Array<{ type: string } & Record<string, unknown>>;
};

export type OpencodeFileDiff = {
  file: string;
  before: string;
  after: string;
  additions: number;
  deletions: number;
};

export type OpencodeProviderListResponse = {
  all: Array<
    {
      id: string;
      name: string;
      models: Array<{ id: string; name: string } & Record<string, unknown>>;
    } & Record<string, unknown>
  >;
  default: Record<string, string>;
  connected: string[];
};

export type OpencodeSkillInfo = {
  name: string;
  description: string;
  location: string;
};

export type OpencodeProviderAuthMethod = {
  type: "oauth" | "api";
  label: string;
};
export type OpencodeProviderAuthMethodsResponse = Record<
  string,
  OpencodeProviderAuthMethod[]
>;
export type OpencodeProviderAuthorization = {
  url: string;
  method: "auto" | "code";
  instructions: string;
};

export type OpencodeEvent =
  | { type: string; properties: any }
  | { payload: { type: string; properties: any }; directory?: string };

export class OpencodeHttpClient {
  public constructor(private readonly opts: OpencodeHttpOptions) {}

  public async getConfig(): Promise<Record<string, unknown>> {
    const res = await this.getJson(`/config`, { directory: this.opts.directory });
    if (typeof res !== "object" || res === null) {
      throw new Error("Unexpected /config response (not an object)");
    }
    return res as Record<string, unknown>;
  }

  public async getHealth(): Promise<{ healthy: true; version: string }> {
    const res = await this.getJson(`/global/health`);
    if (typeof res !== "object" || res === null) {
      throw new Error("Unexpected /global/health response (not an object)");
    }
    const anyRes = res as Record<string, unknown>;
    if (anyRes["healthy"] !== true) {
      throw new Error("Unexpected /global/health response (healthy != true)");
    }
    const version = anyRes["version"];
    if (typeof version !== "string" || !version.trim()) {
      throw new Error("Unexpected /global/health response (missing version)");
    }
    return { healthy: true, version };
  }

  public async listSessions(): Promise<OpencodeSessionInfo[]> {
    const res = await this.getJson(`/session`, {
      directory: this.opts.directory,
    });
    if (!Array.isArray(res)) throw new Error("Unexpected /session response");
    return res as any;
  }

  public async createSession(): Promise<OpencodeSessionInfo> {
    const res = await this.postJson(
      `/session`,
      {},
      { directory: this.opts.directory },
    );
    return res as any;
  }

  public async getSession(sessionID: string): Promise<OpencodeSessionInfo> {
    const res = await this.getJson(
      `/session/${encodeURIComponent(sessionID)}`,
      { directory: this.opts.directory },
    );
    return res as any;
  }

  public async listMessages(
    sessionID: string,
    limit?: number,
  ): Promise<OpencodeMessageWithParts[]> {
    const query: Record<string, string> = { directory: this.opts.directory };
    if (typeof limit === "number" && Number.isFinite(limit))
      query["limit"] = String(limit);
    const res = await this.getJson(
      `/session/${encodeURIComponent(sessionID)}/message`,
      query,
    );
    if (!Array.isArray(res))
      throw new Error("Unexpected /session/:id/message response");
    return res as any;
  }

  public async prompt(
    sessionID: string,
    args: {
      parts: Array<Record<string, unknown>>;
      model?: { providerID: string; modelID: string };
    },
  ): Promise<OpencodeMessageWithParts> {
    const body: Record<string, unknown> = {
      parts: args.parts,
    };
    if (args.model) body["model"] = args.model;
    const res = await this.postJson(
      `/session/${encodeURIComponent(sessionID)}/message`,
      body,
      { directory: this.opts.directory },
    );
    return res as any;
  }

  public async abort(sessionID: string): Promise<void> {
    await this.postJson(
      `/session/${encodeURIComponent(sessionID)}/abort`,
      {},
      { directory: this.opts.directory },
    );
  }

  public async summarize(
    sessionID: string,
    model: { providerID: string; modelID: string; auto?: boolean },
  ): Promise<void> {
    await this.postJson(
      `/session/${encodeURIComponent(sessionID)}/summarize`,
      {
        providerID: model.providerID,
        modelID: model.modelID,
        auto: Boolean(model.auto),
      },
      { directory: this.opts.directory },
    );
  }

  public async revert(sessionID: string, messageID: string): Promise<void> {
    await this.postJson(
      `/session/${encodeURIComponent(sessionID)}/revert`,
      { messageID },
      { directory: this.opts.directory },
    );
  }

  public async unrevert(sessionID: string): Promise<void> {
    await this.postJson(
      `/session/${encodeURIComponent(sessionID)}/unrevert`,
      {},
      { directory: this.opts.directory },
    );
  }

  public async listSkills(cwdFsPath: string): Promise<SkillsListEntry[]> {
    const res = await this.getJson(`/skill`, { directory: cwdFsPath });
    const skills = Array.isArray(res)
      ? (res as any as OpencodeSkillInfo[])
      : [];
    const mapped: SkillMetadata[] = skills.map((s) => {
      const scope: SkillScope = inferSkillScope(cwdFsPath, s.location);
      return {
        name: s.name,
        description: s.description,
        path: s.location,
        scope,
        enabled: true,
      };
    });
    return [{ cwd: cwdFsPath, skills: mapped, errors: [] }];
  }

  public async listModels(): Promise<Model[]> {
    const res = await this.listProviders();
    return this.modelsFromProviders(res);
  }

  public async listProviders(): Promise<OpencodeProviderListResponse> {
    return (await this.getJson(`/provider`, {
      directory: this.opts.directory,
    })) as OpencodeProviderListResponse;
  }

  public modelsFromProviders(res: OpencodeProviderListResponse): Model[] {
    const providers = Array.isArray(res?.all) ? res.all : [];
    const defaultByProvider =
      typeof res?.default === "object" && res.default !== null ? res.default : {};
    const out: Model[] = [];
    for (const p of providers) {
      const providerID = String(p.id ?? "");
      const providerName = String(p.name ?? providerID);
      const rawModels = (p as any).models as unknown;
      const modelEntries: Array<{ id: string; name: string }> = [];
      if (Array.isArray(rawModels)) {
        for (const m of rawModels as any[]) {
          const modelID = String(m?.id ?? "");
          const modelName = String(m?.name ?? modelID);
          if (!modelID) continue;
          modelEntries.push({ id: modelID, name: modelName });
        }
      } else if (typeof rawModels === "object" && rawModels !== null) {
        for (const [modelID, meta] of Object.entries(
          rawModels as Record<string, unknown>,
        )) {
          const modelName =
            typeof (meta as any)?.name === "string" &&
            String((meta as any).name).trim()
              ? String((meta as any).name).trim()
              : modelID;
          if (!modelID) continue;
          modelEntries.push({ id: modelID, name: modelName });
        }
      }

      const defaultModelID =
        typeof (defaultByProvider as any)[providerID] === "string"
          ? String((defaultByProvider as any)[providerID])
          : null;

      for (const m of modelEntries) {
        if (!providerID || !m.id) continue;
        const key = `${providerID}:${m.id}`;
        out.push({
          id: key,
          // For opencode, the model selection UI only carries a single string.
          // Encode `providerID:modelID` so we can recover both on send.
          model: key,
          displayName: `${providerName} / ${m.name}`,
          description: "",
          supportedReasoningEfforts: [],
          defaultReasoningEffort: "none",
          isDefault: defaultModelID ? defaultModelID === m.id : false,
        });
      }
    }
    return out;
  }

  public async listProviderAuthMethods(): Promise<OpencodeProviderAuthMethodsResponse> {
    return (await this.getJson(`/provider/auth`, {
      directory: this.opts.directory,
    })) as OpencodeProviderAuthMethodsResponse;
  }

  public async providerOauthAuthorize(args: {
    providerID: string;
    method: number;
  }): Promise<OpencodeProviderAuthorization | null> {
    const res = await this.postJson(
      `/provider/${encodeURIComponent(args.providerID)}/oauth/authorize`,
      { method: args.method },
      { directory: this.opts.directory },
    );
    if (!res) return null;
    return res as any;
  }

  public async providerOauthCallback(args: {
    providerID: string;
    method: number;
    code?: string;
  }): Promise<void> {
    await this.postJson(
      `/provider/${encodeURIComponent(args.providerID)}/oauth/callback`,
      { method: args.method, ...(args.code ? { code: args.code } : {}) },
      { directory: this.opts.directory },
    );
  }

  public async setProviderApiKey(args: {
    providerID: string;
    apiKey: string;
  }): Promise<void> {
    await this.putJson(
      `/auth/${encodeURIComponent(args.providerID)}`,
      { type: "api", key: args.apiKey },
      { directory: this.opts.directory },
    );
  }

  public async fuzzyFileSearch(args: {
    query: string;
    roots: string[];
    cancellationToken: string;
  }): Promise<FuzzyFileSearchResponse> {
    const q = String(args.query ?? "");
    const root = args.roots[0] ?? this.opts.directory;
    const res = await this.getJson(`/find/file`, {
      directory: root,
      query: q,
      limit: "50",
    });
    if (!Array.isArray(res)) {
      return { files: [] };
    }
    const files = (res as any[]).map((p) => {
      const filePath = String(p);
      return {
        root,
        path: filePath,
        file_name: path.basename(filePath),
        score: 0,
        indices: null,
      };
    });
    return { files };
  }

  public formatFileDiffs(diffs: OpencodeFileDiff[]): string {
    const lines: string[] = [];
    for (const d of diffs) {
      lines.push(`diff -- opencode`);
      lines.push(`file: ${d.file}`);
      lines.push(`additions: ${d.additions} deletions: ${d.deletions}`);
      lines.push("");
      lines.push("----- BEFORE -----");
      lines.push(d.before ?? "");
      lines.push("----- AFTER ------");
      lines.push(d.after ?? "");
      lines.push("");
    }
    return lines.join("\n").trimEnd();
  }

  public async connectEventStream(
    onEvent: (event: OpencodeEvent) => void,
    onError: (err: Error) => void,
  ): Promise<AbortController> {
    const controller = new AbortController();
    const url = this.buildUrl(`/event`, { directory: this.opts.directory });
    void (async () => {
      try {
        const res = await fetch(url, {
          method: "GET",
          headers: {
            accept: "text/event-stream",
          },
          signal: controller.signal,
        });
        if (!res.ok)
          throw new Error(
            `SSE connect failed: ${res.status} ${res.statusText}`,
          );
        if (!res.body) throw new Error("SSE response has no body");
        const reader = res.body.getReader();
        const decoder = new TextDecoder();
        let buf = "";
        for (;;) {
          const { value, done } = await reader.read();
          if (done) break;
          buf += decoder.decode(value, { stream: true });
          for (;;) {
            const idx = buf.indexOf("\n\n");
            if (idx === -1) break;
            const raw = buf.slice(0, idx);
            buf = buf.slice(idx + 2);
            const dataLines = raw
              .split("\n")
              .map((l) => l.trimEnd())
              .filter((l) => l.startsWith("data:"))
              .map((l) => l.slice("data:".length).trimStart());
            if (dataLines.length === 0) continue;
            const payload = dataLines.join("\n");
            try {
              const parsed = JSON.parse(payload) as OpencodeEvent;
              onEvent(parsed);
            } catch (e) {
              onError(new Error(`Failed to parse SSE event: ${String(e)}`));
            }
          }
        }
      } catch (err) {
        if ((err as any)?.name === "AbortError") return;
        onError(err instanceof Error ? err : new Error(String(err)));
      }
    })();
    return controller;
  }

  private buildUrl(pathname: string, query?: Record<string, string>): string {
    const url = new URL(pathname, this.opts.baseUrl);
    if (query) {
      for (const [k, v] of Object.entries(query)) {
        url.searchParams.set(k, v);
      }
    }
    return url.toString();
  }

  private async getJson(
    pathname: string,
    query?: Record<string, string>,
  ): Promise<unknown> {
    const url = this.buildUrl(pathname, query);
    let res: Response;
    try {
      res = await fetch(url, { method: "GET" });
    } catch (err) {
      const e = new Error(`GET ${pathname} fetch failed: url=${url}`);
      (e as any).cause = err;
      throw e;
    }
    if (!res.ok)
      throw new Error(
        `GET ${pathname} failed: ${res.status} ${res.statusText}`,
      );
    return await res.json();
  }

  private async postJson(
    pathname: string,
    body: unknown,
    query?: Record<string, string>,
  ): Promise<unknown> {
    const url = this.buildUrl(pathname, query);
    let res: Response;
    try {
      res = await fetch(url, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body ?? {}),
      });
    } catch (err) {
      const e = new Error(`POST ${pathname} fetch failed: url=${url}`);
      (e as any).cause = err;
      throw e;
    }
    if (!res.ok) {
      const text = await res.text().catch(() => "");
      throw new Error(
        `POST ${pathname} failed: ${res.status} ${res.statusText}${text ? `; body=${text}` : ""}`,
      );
    }
    if (res.status === 204) return null;
    const ct = res.headers.get("content-type") ?? "";
    if (!ct.includes("application/json")) return await res.text();
    return await res.json();
  }

  private async putJson(
    pathname: string,
    body: unknown,
    query?: Record<string, string>,
  ): Promise<unknown> {
    const url = this.buildUrl(pathname, query);
    let res: Response;
    try {
      res = await fetch(url, {
        method: "PUT",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body ?? {}),
      });
    } catch (err) {
      const e = new Error(`PUT ${pathname} fetch failed: url=${url}`);
      (e as any).cause = err;
      throw e;
    }
    if (!res.ok) {
      const text = await res.text().catch(() => "");
      throw new Error(
        `PUT ${pathname} failed: ${res.status} ${res.statusText}${text ? `; body=${text}` : ""}`,
      );
    }
    if (res.status === 204) return null;
    const ct = res.headers.get("content-type") ?? "";
    if (!ct.includes("application/json")) return await res.text();
    return await res.json();
  }
}

function inferSkillScope(cwdFsPath: string, location: string): SkillScope {
  const loc = String(location ?? "");
  if (!loc) return "user";
  const rel = path.relative(cwdFsPath, loc);
  if (rel && !rel.startsWith("..") && !path.isAbsolute(rel)) return "repo";
  return "user";
}
