/* eslint-disable no-restricted-globals */

// NOTE: This file intentionally has no imports/exports so that TypeScript emits a plain browser script
// (tsconfig uses CommonJS modules for the extension host, but webview scripts must not rely on require()).

declare function acquireVsCodeApi(): {
  postMessage(msg: unknown): void;
  getState(): unknown;
  setState(state: unknown): void;
};

declare const markdownit:
  | undefined
  | ((opts?: unknown) => {
      render(md: string): string;
      renderer: { rules: Record<string, any> };
    });

type Session = { id: string; title: string };
type ModelState = {
  model: string | null;
  provider: string | null;
  reasoning: string | null;
};

type ChatBlock =
  | { id: string; type: "user"; text: string }
  | { id: string; type: "assistant"; text: string }
  | { id: string; type: "divider"; text: string }
  | { id: string; type: "note"; text: string }
  | { id: string; type: "info"; title: string; text: string }
  | { id: string; type: "webSearch"; query: string; status: string }
  | {
      id: string;
      type: "reasoning";
      summaryParts: string[];
      rawParts: string[];
      status: string;
    }
  | {
      id: string;
      type: "command";
      title: string;
      status: string;
      command: string;
      actionsText?: string | null;
      cwd: string | null;
      exitCode: number | null;
      durationMs: number | null;
      terminalStdin: string[];
      output: string;
    }
  | {
      id: string;
      type: "fileChange";
      title: string;
      status: string;
      files: string[];
      detail: string;
      hasDiff: boolean;
      diffs?: Array<{ path: string; diff: string }>;
    }
  | {
      id: string;
      type: "mcp";
      title: string;
      status: string;
      server: string;
      tool: string;
      detail: string;
    }
  | { id: string; type: "plan"; title: string; text: string }
  | { id: string; type: "error"; title: string; text: string }
  | { id: string; type: "system"; title: string; text: string };

type ChatViewState = {
  globalBlocks?: ChatBlock[];
  sessions: Session[];
  activeSession: Session | null;
  blocks: ChatBlock[];
  latestDiff: string | null;
  sending: boolean;
  statusText?: string | null;
  modelState?: ModelState | null;
  models?: Array<{
    id: string;
    model: string;
    displayName: string;
    description: string;
    supportedReasoningEfforts: Array<{ reasoningEffort: string; description: string }>;
    defaultReasoningEffort: string;
    isDefault: boolean;
  }> | null;
  approvals: Array<{
    requestKey: string;
    title: string;
    detail: string;
    canAcceptForSession: boolean;
  }>;
  customPrompts?: Array<{
    name: string;
    description: string | null;
    argumentHint: string | null;
    source: string;
  }>;
};

type SuggestItem = {
  insert: string;
  label: string;
  detail?: string;
  kind: "slash" | "at" | "file";
};

function main(): void {
  let vscode: ReturnType<typeof acquireVsCodeApi>;
  try {
    vscode = acquireVsCodeApi();
  } catch (err) {
    const st = document.getElementById("statusText");
    if (st) {
      st.textContent = "Webview error: acquireVsCodeApi() failed";
      (st as HTMLElement).style.display = "";
    }
    throw err;
  }

  const mustGet = <T extends HTMLElement = HTMLElement>(id: string): T => {
    const e = document.getElementById(id);
    if (!e) throw new Error(`Webview DOM element missing: #${id}`);
    return e as T;
  };

  const titleEl = mustGet("title");
  const statusTextEl = mustGet("statusText");
  const logEl = mustGet("log");
  const approvalsEl = mustGet("approvals");
  const inputEl = mustGet<HTMLTextAreaElement>("input");
  const sendBtn = mustGet<HTMLButtonElement>("send");
  const diffBtn = mustGet<HTMLButtonElement>("diff");
  const newBtn = mustGet<HTMLButtonElement>("new");
  const statusBtn = mustGet<HTMLButtonElement>("status");
  const tabsEl = mustGet("tabs");
  const modelBarEl = mustGet("modelBar");
  const modelSelect = document.createElement("select");
  modelSelect.className = "modelSelect model";
  const reasoningSelect = document.createElement("select");
  reasoningSelect.className = "modelSelect effort";
  modelBarEl.appendChild(modelSelect);
  modelBarEl.appendChild(reasoningSelect);

  const populateSelect = (
    el: HTMLSelectElement,
    options: string[],
    value: string | null | undefined,
  ): void => {
    const v = (value && value.trim()) || "default";
    const opts = options.includes(v) ? options : [v, ...options];
    el.innerHTML = "";
    for (const opt of opts) {
      const o = document.createElement("option");
      o.value = opt;
      o.textContent = opt === "default" ? "default (CLI config)" : opt;
      if (opt === v) o.selected = true;
      el.appendChild(o);
    }
  };

  const sendModelState = (): void => {
    vscode.postMessage({
      type: "setModel",
      model: modelSelect.value === "default" ? null : modelSelect.value,
      reasoning:
        reasoningSelect.value === "default" ? null : reasoningSelect.value,
    });
  };

  modelSelect.addEventListener("change", sendModelState);
  reasoningSelect.addEventListener("change", sendModelState);
  const suggestEl = mustGet("suggest");

  // Chat auto-scroll:
  // - While the user is near the bottom, new content keeps the log pinned to the bottom.
  // - Once the user scrolls up, stop forcing scroll (free mode) until they scroll back near bottom.
  let stickLogToBottom = true;
  const isLogNearBottom = (): boolean => {
    const slackPx = 40;
    return (
      logEl.scrollHeight - logEl.scrollTop - logEl.clientHeight <= slackPx
    );
  };

  const MAX_INPUT_HEIGHT_PX = 200;
  const MIN_INPUT_HEIGHT_PX = 30;
  function autosizeInput(): void {
    inputEl.style.height = "auto";
    const nextHeight = Math.min(MAX_INPUT_HEIGHT_PX, inputEl.scrollHeight);
    inputEl.style.height = `${Math.max(MIN_INPUT_HEIGHT_PX, nextHeight)}px`;
    inputEl.style.overflowY =
      inputEl.scrollHeight > MAX_INPUT_HEIGHT_PX ? "auto" : "hidden";
  }
  logEl.addEventListener("scroll", () => {
    stickLogToBottom = isLogNearBottom();
  });


  const getSessionDisplayTitle = (
    sess: Session,
    idx: number,
  ): { label: string; tooltip: string } => {
    const title = String(sess.title || "").trim() || "Untitled";
    return { label: `${title} #${idx + 1}`, tooltip: title };
  };

  if (typeof markdownit !== "function") {
    throw new Error("markdown-it is not loaded");
  }
  const md = markdownit({ html: false, linkify: true, breaks: true });
  const defaultLinkOpen =
    md.renderer.rules["link_open"] ||
    ((tokens: any, idx: number, options: any, _env: any, self: any) =>
      self.renderToken(tokens, idx, options));
  md.renderer.rules["link_open"] = function (
    tokens: any,
    idx: number,
    options: any,
    env: any,
    self: any,
  ) {
    const token = tokens[idx];
    if (token && typeof token.attrSet === "function") {
      token.attrSet("target", "_blank");
      token.attrSet("rel", "noreferrer noopener");
    }
    return defaultLinkOpen(tokens, idx, options, env, self);
  };

  let receivedState = false;
  function showWebviewError(err: unknown): void {
    const anyErr = err as { message?: unknown; stack?: unknown } | null;
    const msg = String(anyErr && anyErr.message ? anyErr.message : err);
    statusTextEl.textContent = "Webview error: " + msg;
    statusTextEl.style.display = "";
    try {
      vscode.postMessage({
        type: "webviewError",
        message: msg,
        stack: anyErr && anyErr.stack ? String(anyErr.stack) : null,
      });
    } catch {
      // ignore
    }
  }

  window.addEventListener("error", (e) =>
    showWebviewError((e as ErrorEvent).error || (e as ErrorEvent).message),
  );
  window.addEventListener("unhandledrejection", (e) =>
    showWebviewError((e as PromiseRejectionEvent).reason),
  );

  let state: ChatViewState = {
    sessions: [],
    activeSession: null,
    blocks: [],
    latestDiff: null,
    sending: false,
    statusText: null,
    modelState: null,
    approvals: [],
    customPrompts: [],
  };

  let domSessionId: string | null = null;
  const blockElByKey = new Map<string, HTMLElement>();

  const inputHistory: string[] = [];
  let historyIndex: number | null = null;
  let draftBeforeHistory = "";
  let isComposing = false;

  let detailsState =
    (((vscode.getState() as { detailsState?: unknown } | undefined) || {})
      .detailsState as Record<string, boolean> | undefined) || {};

  function saveDetailsState(key: string, open: boolean): void {
    detailsState[key] = open;
    vscode.setState({ detailsState });
  }

  const baseSlashSuggestions: SuggestItem[] = [];
  const uiSlashSuggestions: SuggestItem[] = [
    {
      insert: "/new ",
      label: "/new",
      detail: "New session",
      kind: "slash",
    },
    {
      insert: "/diff ",
      label: "/diff",
      detail: "Open Latest Diff",
      kind: "slash",
    },
    {
      insert: "/rename ",
      label: "/rename",
      detail: "Rename session",
      kind: "slash",
    },
    { insert: "/help ", label: "/help", detail: "Show help", kind: "slash" },
  ];

  function buildSlashSuggestions(): SuggestItem[] {
    const reserved = new Set(
      [...baseSlashSuggestions, ...uiSlashSuggestions].map((s) =>
        s.label.replace(/^\//, ""),
      ),
    );
    const custom = (state.customPrompts ?? [])
      .map((p) => {
        const name = String(p.name || "").trim();
        if (!name || reserved.has(name)) return null;
        const hint = p.argumentHint ? " " + p.argumentHint : "";
        const detail = p.description || p.argumentHint || "Custom prompt";
        return {
          insert: "/prompts:" + name + hint + " ",
          label: "/prompts:" + name,
          detail,
          kind: "slash",
        } as SuggestItem;
      })
      .filter(Boolean) as SuggestItem[];
    return [...baseSlashSuggestions, ...custom, ...uiSlashSuggestions];
  }

  const atSuggestions: SuggestItem[] = [
    {
      insert: "@selection ",
      label: "@selection",
      detail: "Insert selection reference",
      kind: "at",
    },
  ];

  let suggestItems: SuggestItem[] = [];
  let suggestIndex = 0;
  let fileIndex: string[] | null = null;
  let fileIndexForSessionId: string | null = null;
  let fileIndexRequested = false;
  let activeReplace: null | {
    from: number;
    to: number;
    inserted: string;
  } = null;

  function isOpen(key: string, defaultOpen: boolean): boolean {
    const v = detailsState[key];
    if (v === undefined) return !!defaultOpen;
    return !!v;
  }

  function el(tag: string, className?: string): HTMLElement {
    const e = document.createElement(tag);
    if (className) e.className = className;
    return e;
  }

  function truncateCommand(cmd: string, max: number): string {
    const c = cmd.trim().replace(/\s+/g, " ");
    if (c.length <= max) return c;
    return c.slice(0, Math.max(0, max - 1)) + "…";
  }

  function truncateOneLine(text: string, max: number): string {
    const c = text.trim().replace(/\s+/g, " ");
    if (c.length <= max) return c;
    return c.slice(0, Math.max(0, max - 1)) + "…";
  }

  function looksOpaqueToken(s: string): boolean {
    const t = s.trim();
    if (t.length < 40) return false;
    if (t.includes(" ")) return false;
    if (t.includes("\n")) return false;
    // Likely base64 or similar token.
    if (!/^[A-Za-z0-9+/=]+$/.test(t)) return false;
    return true;
  }

  function normalizeStatusKey(status: string): string {
    const s = status.trim();
    if (!s) return "";
    if (s === "in_progress" || s === "in-progress") return "inProgress";
    if (s === "canceled") return "cancelled";
    return s;
  }

  function stripShellWrapper(cmd: string): string {
    const t = cmd.trim();
    // Common wrapper produced by the tool runner:
    //   /bin/zsh -lc cd /path && <actual>
    //   /bin/bash -lc "cd /path && <actual>"
    const m1 = t.match(/^\/bin\/(zsh|bash)\s+-lc\s+cd\s+.+?\s+&&\s+([\s\S]+)$/);
    if (m1) return String(m1[2] || "").trim();
    const m2 = t.match(
      /^\/bin\/(zsh|bash)\s+-lc\s+["']cd\s+.+?\s+&&\s+([\s\S]+?)["']$/,
    );
    if (m2) return String(m2[2] || "").trim();
    return cmd;
  }

  function ensureDetails(
    key: string,
    className: string,
    openDefault: boolean,
    summaryText: string,
    onToggleKey: string,
  ): HTMLDetailsElement {
    const existing = blockElByKey.get(key);
    if (existing && existing.tagName.toLowerCase() === "details") {
      const det = existing as HTMLDetailsElement;
      det.className = className;
      const sum = det.querySelector(":scope > summary");
      if (sum) {
        const txt = sum.querySelector(
          ':scope > span[data-k="summaryText"]',
        ) as HTMLSpanElement | null;
        if (txt) txt.textContent = summaryText;
        else sum.textContent = summaryText;
      }
      return det;
    }

    const det = document.createElement("details");
    det.className = className;
    det.open = isOpen(onToggleKey, openDefault);
    det.addEventListener("toggle", () =>
      saveDetailsState(onToggleKey, det.open),
    );
    const sum = document.createElement("summary");
    const txt = document.createElement("span");
    txt.dataset.k = "summaryText";
    txt.textContent = summaryText;
    sum.appendChild(txt);
    const icon = document.createElement("span");
    icon.dataset.k = "statusIcon";
    icon.className = "statusIcon";
    icon.style.display = "none";
    sum.appendChild(icon);
    det.appendChild(sum);
    blockElByKey.set(key, det);
    logEl.appendChild(det);
    return det;
  }

  function setStatusIcon(
    det: HTMLDetailsElement,
    status: string | null | undefined,
  ): void {
    const sum = det.querySelector(":scope > summary");
    if (!sum) return;
    const icon = sum.querySelector(
      ':scope > span[data-k="statusIcon"]',
    ) as HTMLSpanElement | null;
    if (!icon) return;
    const s = String(status || "").trim();
    if (!s) {
      icon.style.display = "none";
      icon.textContent = "";
      icon.title = "";
      icon.className = "statusIcon";
      return;
    }

    const key = normalizeStatusKey(s);

    icon.style.display = "";
    icon.title = s;
    icon.className = "statusIcon status-" + key;
  }

  function ensureCardStatusIcon(el: HTMLElement): HTMLSpanElement {
    const existing = el.querySelector(
      ':scope > span[data-k="statusIcon"]',
    ) as HTMLSpanElement | null;
    if (existing) return existing;
    const sp = document.createElement("span");
    sp.dataset.k = "statusIcon";
    sp.className = "statusIcon";
    sp.style.display = "none";
    el.appendChild(sp);
    return sp;
  }

  function setCardStatusIcon(
    el: HTMLElement,
    status: string | null | undefined,
  ): void {
    const icon = ensureCardStatusIcon(el);
    const s = String(status || "").trim();
    if (!s) {
      icon.style.display = "none";
      icon.title = "";
      icon.className = "statusIcon";
      return;
    }
    const key = normalizeStatusKey(s);
    icon.style.display = "";
    icon.title = s;
    icon.className = "statusIcon status-" + key;
  }

  function ensureDiv(key: string, className: string): HTMLDivElement {
    const existing = blockElByKey.get(key);
    if (existing && existing.tagName.toLowerCase() === "div") {
      const div = existing as HTMLDivElement;
      div.className = className;
      return div;
    }
    const div = document.createElement("div");
    div.className = className;
    blockElByKey.set(key, div);
    logEl.appendChild(div);
    return div;
  }

  function ensurePre(parent: HTMLElement, key: string): HTMLPreElement {
    const pre = parent.querySelector(
      `pre[data-k="${key}"]`,
    ) as HTMLPreElement | null;
    if (pre) return pre;
    const p = document.createElement("pre");
    p.dataset.k = key;
    parent.appendChild(p);
    return p;
  }

  function ensureMd(parent: HTMLElement, key: string): HTMLDivElement {
    const div = parent.querySelector(
      `div.md[data-k="${key}"]`,
    ) as HTMLDivElement | null;
    if (div) return div;
    const d = document.createElement("div");
    d.className = "md";
    d.dataset.k = key;
    parent.appendChild(d);
    return d;
  }

  function ensureFileList(parent: HTMLElement, key: string): HTMLDivElement {
    const div = parent.querySelector(
      `div.fileList[data-k="${key}"]`,
    ) as HTMLDivElement | null;
    if (div) return div;
    const d = document.createElement("div");
    d.className = "fileList";
    d.dataset.k = key;
    parent.appendChild(d);
    return d;
  }

  function ensureNestedDetails(
    parent: HTMLElement,
    key: string,
    className: string,
    openDefault: boolean,
    summaryText: string,
    onToggleKey: string,
  ): HTMLDetailsElement {
    const existing = blockElByKey.get(key);
    if (existing && existing.tagName.toLowerCase() === "details") {
      const det = existing as HTMLDetailsElement;
      det.className = className;
      const sum = det.querySelector(":scope > summary");
      if (sum) sum.textContent = summaryText;
      if (det.parentElement !== parent) parent.appendChild(det);
      return det;
    }

    const det = document.createElement("details");
    det.className = className;
    det.open = isOpen(onToggleKey, openDefault);
    det.addEventListener("toggle", () =>
      saveDetailsState(onToggleKey, det.open),
    );
    const sum = document.createElement("summary");
    sum.textContent = summaryText;
    det.appendChild(sum);
    blockElByKey.set(key, det);
    parent.appendChild(det);
    return det;
  }

  function removeBlockEl(key: string): void {
    const el = blockElByKey.get(key);
    if (!el) return;
    if (el.parentElement) el.parentElement.removeChild(el);
    blockElByKey.delete(key);
    delete detailsState[key];
  }

  function renderMarkdownInto(el: HTMLElement, text: string): void {
    if (el.dataset.src === text) return;
    el.dataset.src = text;
    el.innerHTML = md.render(text);
  }

  function ensureMeta(parent: HTMLElement, key: string): HTMLDivElement {
    const meta = parent.querySelector(
      `div.meta[data-k="${key}"]`,
    ) as HTMLDivElement | null;
    if (meta) return meta;
    const m = document.createElement("div");
    m.className = "meta";
    m.dataset.k = key;
    parent.appendChild(m);
    return m;
  }

  function setSuggestVisible(visible: boolean): void {
    suggestEl.style.display = visible ? "block" : "none";
  }

  function renderSuggest(): void {
    suggestEl.innerHTML = "";
    if (suggestItems.length === 0) {
      setSuggestVisible(false);
      return;
    }
    setSuggestVisible(true);

    for (let i = 0; i < suggestItems.length; i++) {
      const it = suggestItems[i]!;
      const row = el(
        "div",
        "suggestItem" + (i === suggestIndex ? " active" : ""),
      );
      const left = el("div");
      left.textContent = it.label;
      row.appendChild(left);
      const right = el("div", "suggestRight");
      right.textContent = it.detail || "";
      row.appendChild(right);
      row.addEventListener("click", () => acceptSuggestion(i));
      suggestEl.appendChild(row);
    }

    const active = suggestEl.querySelector(".suggestItem.active") as HTMLElement | null;
    if (active) {
      // Keep the active item visible when navigating with keyboard.
      active.scrollIntoView({ block: "nearest" });
    }
  }

  function currentPrefixedToken(
    prefix: string,
  ): { token: string; start: number; end: number } | null {
    const text = inputEl.value;
    const cur = inputEl.selectionStart ?? 0;

    const isWs = (c: string): boolean => c === " " || c === "\n" || c === "\t";

    let left = cur;
    while (left > 0 && !isWs(text[left - 1] || "")) left--;
    let right = cur;
    while (right < text.length && !isWs(text[right] || "")) right++;

    let start = left;
    let end = right;
    if (left === right) {
      // Cursor on whitespace: prefer right token if it starts with prefix.
      // NOTE: We intentionally do NOT fall back to the left token. This avoids reopening the suggest
      // popup after the user accepted an item that inserted a trailing space.
      const rStart = right;
      let rEnd = rStart;
      while (rEnd < text.length && !isWs(text[rEnd] || "")) rEnd++;
      const rTok = text.slice(rStart, rEnd);
      if (rTok.startsWith(prefix))
        return { token: rTok, start: rStart, end: rEnd };
      return null;
    }

    const tok = text.slice(start, end);
    if (!tok.startsWith(prefix)) return null;
    return { token: tok, start, end };
  }

  function rankByPrefix(items: SuggestItem[], query: string): SuggestItem[] {
    const q = query.toLowerCase();
    const scored = items
      .map((it) => {
        const label = it.label.toLowerCase();
        const altLabel = label.startsWith("/prompts:")
          ? ("/" + label.slice("/prompts:".length))
          : label;
        const useAlt = !q.includes("prompts:");
        const hay = useAlt ? altLabel : label;
        const idx = hay.indexOf(q);
        const score = idx === 0 ? 0 : idx > 0 ? 1 : 2;
        return { it, score, idx };
      })
      .sort(
        (a, b) =>
          a.score - b.score ||
          a.idx - b.idx ||
          a.it.label.localeCompare(b.it.label),
      );
    return scored.map((s) => s.it);
  }

  function slashMatches(label: string, query: string): boolean {
    const raw = label.toLowerCase();
    const alt = raw.startsWith("/prompts:")
      ? "/" + raw.slice("/prompts:".length)
      : raw;
    return raw.startsWith("/" + query) || alt.startsWith("/" + query);
  }

  function isSameSuggestList(a: SuggestItem[], b: SuggestItem[]): boolean {
    if (a.length !== b.length) return false;
    for (let i = 0; i < a.length; i += 1) {
      const x = a[i];
      const y = b[i];
      if (!x || !y) return false;
      if (x.label !== y.label) return false;
      if (x.insert !== y.insert) return false;
      if (x.kind !== y.kind) return false;
    }
    return true;
  }

  function updateSuggestions(): void {
    if (!state.activeSession) {
      suggestItems = [];
      renderSuggest();
      return;
    }

    const prevItems = suggestItems;
    const prevIndex = suggestIndex;
    const prevReplace = activeReplace;

    const cursor = inputEl.selectionStart ?? 0;
    const before = inputEl.value.slice(0, cursor);

    const atTok = currentPrefixedToken("@");
    if (atTok) {
      const query = atTok.token.slice(1);
      let items: SuggestItem[] = [...atSuggestions];

      if (query.length > 0 || atTok.token === "@") {
        if (fileIndex && fileIndexForSessionId === state.activeSession.id) {
          const q = query.toLowerCase();
          const rankedPaths = fileIndex
            .filter((p) => p.toLowerCase().includes(q))
            .map((p) => {
              const pl = p.toLowerCase();
              const base = pl.split("/").at(-1) || pl;
              const depth = (p.match(/\//g) || []).length;
              const score =
                base === q || pl === q
                  ? 0
                  : base.startsWith(q)
                    ? 1
                    : pl.startsWith(q)
                      ? 2
                      : pl.includes("/" + q)
                        ? 3
                        : 4;
              return { p, score, depth, len: p.length };
            })
            .sort(
              (a, b) =>
                a.score - b.score ||
                a.depth - b.depth ||
                a.len - b.len ||
                a.p.localeCompare(b.p),
            )
            .slice(0, 50)
            .map((x) => x.p);

          const fileItems = rankedPaths.map((p) => ({
            insert: "@" + p + " ",
            label: "@" + p,
            detail: "",
            kind: "file" as const,
          }));
          items = items.concat(fileItems);
        } else {
          if (!fileIndexRequested) {
            fileIndexRequested = true;
            vscode.postMessage({
              type: "requestFileIndex",
              sessionId: state.activeSession.id,
            });
          }
          items = items.concat([
            {
              insert: "",
              label: "(indexing…)",
              detail: "",
              kind: "file",
            },
          ]);
        }
      }

      const ranked = query ? rankByPrefix(items, query) : items;
      const nextReplace = { from: atTok.start, to: atTok.end, inserted: "" };
      suggestItems = ranked;
      activeReplace = nextReplace;
      if (
        prevReplace &&
        prevReplace.from === nextReplace.from &&
        prevReplace.to === nextReplace.to &&
        isSameSuggestList(prevItems, ranked)
      ) {
        suggestIndex = Math.min(ranked.length - 1, Math.max(0, prevIndex));
      } else {
        suggestIndex = 0;
      }
      renderSuggest();
      return;
    }

    // Slash commands: only show at start of first line.
    const lineStart = before.lastIndexOf("\n") + 1;
    const onFirstLine = before.indexOf("\n") === -1;
    if (onFirstLine && lineStart === 0) {
      const slashTok = currentPrefixedToken("/");
      if (slashTok) {
        const query = slashTok.token.slice(1);
        const allSlash = buildSlashSuggestions();
        if (
          query.length === 0 ||
          allSlash.some((s) => slashMatches(s.label, query))
        ) {
          const ranked = query
            ? rankByPrefix(allSlash, "/" + query)
            : allSlash;
          const nextReplace = {
            from: slashTok.start,
            to: slashTok.end,
            inserted: "",
          };
          suggestItems = ranked;
          activeReplace = nextReplace;
          if (
            prevReplace &&
            prevReplace.from === nextReplace.from &&
            prevReplace.to === nextReplace.to &&
            isSameSuggestList(prevItems, ranked)
          ) {
            suggestIndex = Math.min(ranked.length - 1, Math.max(0, prevIndex));
          } else {
            suggestIndex = 0;
          }
          renderSuggest();
          return;
        }
      }
    }

    suggestItems = [];
    activeReplace = null;
    renderSuggest();
  }

  function acceptSuggestion(idx: number): void {
    const it = suggestItems[idx];
    if (!it || !activeReplace) return;
    if (it.insert === "" && it.label === "(indexing…)") return;

    const text = inputEl.value;
    const next =
      text.slice(0, activeReplace.from) +
      it.insert +
      text.slice(activeReplace.to);
    inputEl.value = next;
    const newCursor = activeReplace.from + it.insert.length;
    inputEl.setSelectionRange(newCursor, newCursor);

    // Close suggest UI after accepting; subsequent Enter should send.
    suggestItems = [];
    activeReplace = null;
    renderSuggest();
  }

  function render(s: ChatViewState): void {
    state = s;
    const shouldAutoScroll = stickLogToBottom && isLogNearBottom();
    titleEl.textContent = s.activeSession
      ? getSessionDisplayTitle(
          s.activeSession,
          (s.sessions || []).findIndex((x) => x.id === s.activeSession!.id),
        ).label
      : "Codex UI (no session selected)";

    const ms = s.modelState || {
      model: null,
      provider: null,
      reasoning: null,
    };
    const models = s.models ?? [];
    const modelOptions = ["default", ...models.map((m) => m.model || m.id)];
    populateSelect(modelSelect, modelOptions, ms.model);

    const effortOptions = (() => {
      if (!ms.model || models.length === 0)
        return ["default", "none", "minimal", "low", "medium", "high", "xhigh"];
      const model = models.find((m) => m.model === ms.model || m.id === ms.model);
      if (!model) return ["default", "none", "minimal", "low", "medium", "high", "xhigh"];
      const supported =
        model.supportedReasoningEfforts
          ?.map((o) => o.reasoningEffort)
          .filter((v): v is string => typeof v === "string" && v.length > 0) ?? [];
      if (supported.length === 0)
        return ["default", "none", "minimal", "low", "medium", "high", "xhigh"];
      return ["default", ...supported];
    })();
    populateSelect(reasoningSelect, effortOptions, ms.reasoning);


    const fullStatus = String(s.statusText || "").trim();
    const shortStatus = (() => {
      const m = fullStatus.match(/ctx\\s+remaining=([0-9]{1,3})%/i);
      if (m) return `ctx ${m[1]}%`;
      return fullStatus;
    })();
    statusTextEl.textContent = shortStatus;
    statusTextEl.title = fullStatus;
    statusTextEl.style.display = shortStatus ? "" : "none";
    diffBtn.disabled = !s.latestDiff;
    sendBtn.disabled = !s.activeSession;
    sendBtn.dataset.mode = s.sending ? "stop" : "send";
    sendBtn.setAttribute("aria-label", s.sending ? "Stop" : "Send");
    sendBtn.title = s.sending ? "Stop (Esc)" : "Send (Enter)";
    statusBtn.disabled = !s.activeSession || s.sending;
    // Keep input enabled so the user can draft messages even before selecting a session.
    // Sending is still guarded by sendBtn.disabled and sendCurrentInput().
    inputEl.disabled = false;

    tabsEl.innerHTML = "";
    (s.sessions || []).forEach((sess, idx) => {
      const div = el(
        "div",
        "tab" +
          (s.activeSession && sess.id === s.activeSession.id ? " active" : ""),
      );
      const dt = getSessionDisplayTitle(sess, idx);
      div.textContent = dt.label;
      div.title = dt.tooltip;
      div.addEventListener("click", () =>
        vscode.postMessage({ type: "selectSession", sessionId: sess.id }),
      );
      div.addEventListener("contextmenu", (e) => {
        e.preventDefault();
        vscode.postMessage({ type: "sessionMenu", sessionId: sess.id });
      });
      tabsEl.appendChild(div);
    });

    if (domSessionId !== (s.activeSession ? s.activeSession.id : null)) {
      domSessionId = s.activeSession ? s.activeSession.id : null;
      blockElByKey.clear();
      logEl.replaceChildren();
      // New session / fresh log should start pinned.
      stickLogToBottom = true;
    }

    approvalsEl.innerHTML = "";
    const approvals = s.approvals || [];
    if (s.activeSession && approvals.length > 0) {
      approvalsEl.style.display = "";
      for (const ap of approvals) {
        const card = el("div", "approval");
        const t = el("div", "approvalTitle");
        t.textContent = ap.title;
        card.appendChild(t);
        const pre = el("pre") as HTMLPreElement;
        pre.textContent = ap.detail;
        card.appendChild(pre);
        const actions = el("div", "approvalActions");

        const btnAccept = document.createElement("button");
        btnAccept.textContent = "Accept";
        btnAccept.addEventListener("click", () =>
          vscode.postMessage({
            type: "approve",
            requestKey: ap.requestKey,
            decision: "accept",
          }),
        );
        actions.appendChild(btnAccept);

        if (ap.canAcceptForSession) {
          const btnAcceptSession = document.createElement("button");
          btnAcceptSession.textContent = "Accept (For Session)";
          btnAcceptSession.addEventListener("click", () =>
            vscode.postMessage({
              type: "approve",
              requestKey: ap.requestKey,
              decision: "acceptForSession",
            }),
          );
          actions.appendChild(btnAcceptSession);
        }

        const btnDecline = document.createElement("button");
        btnDecline.textContent = "Decline";
        btnDecline.addEventListener("click", () =>
          vscode.postMessage({
            type: "approve",
            requestKey: ap.requestKey,
            decision: "decline",
          }),
        );
        actions.appendChild(btnDecline);

        const btnCancel = document.createElement("button");
        btnCancel.textContent = "Cancel";
        btnCancel.addEventListener("click", () =>
          vscode.postMessage({
            type: "approve",
            requestKey: ap.requestKey,
            decision: "cancel",
          }),
        );
        actions.appendChild(btnCancel);

        card.appendChild(actions);
        approvalsEl.appendChild(card);
      }
    } else {
      approvalsEl.style.display = "none";
    }

    const globalBlocks = s.globalBlocks || [];
    if (globalBlocks.length > 0) {
      for (const block of globalBlocks) {
        const id = "global:" + block.type + ":" + block.id;
        const title =
          "title" in block &&
          typeof (block as { title?: unknown }).title === "string"
            ? String((block as { title: string }).title)
            : "";
        const summaryText =
          block.type === "error"
            ? "Notice: " + title
            : "Notice: " + (title || block.type);
        const detClass =
          block.type === "info"
            ? "notice info"
            : title === "Other events (debug)"
              ? "notice debug"
              : "notice";
        const det = ensureDetails(
          id,
          detClass,
          block.type === "info",
          summaryText,
          id,
        );
        const nextText =
          block.type === "user" || block.type === "assistant"
            ? (block as { text: string }).text
            : block.type === "system" ||
                block.type === "info" ||
                block.type === "plan" ||
                block.type === "error"
              ? (block as { text: string }).text
              : JSON.stringify(block, null, 2);
        const pre = det.querySelector(`pre[data-k="body"]`);
        if (pre) pre.remove();
        const mdEl = ensureMd(det, "body");
        renderMarkdownInto(
          mdEl,
          typeof nextText === "string" ? nextText : String(nextText),
        );
      }
    }

    if (!s.activeSession) {
      domSessionId = null;
      blockElByKey.clear();
      logEl.replaceChildren();

      const div = ensureDiv("noSession", "msg system");
      const pre = ensurePre(div, "body");
      if (pre.textContent !== "Select a session in Sessions.") {
        pre.textContent = "Select a session in Sessions.";
      }
      return;
    }

    for (const block of s.blocks || []) {
      if (block.type === "divider") {
        const key = "b:" + block.id;
        const div = ensureDiv(key, "msg system divider");
        const pre = ensurePre(div, "body");
        if (pre.textContent !== block.text) pre.textContent = block.text;
        continue;
      }

      if (block.type === "note") {
        const key = "b:" + block.id;
        const div = ensureDiv(key, "note");
        const text = String(block.text ?? "");
        if (div.textContent !== text) div.textContent = text;
        continue;
      }

      if (block.type === "webSearch") {
        const q = String(block.query || "");
        const summaryQ = truncateOneLine(q, 120);
        const key = "b:" + block.id;
        const div = ensureDiv(key, "msg tool webSearch webSearchCard");
        div.title = q;
        setCardStatusIcon(div, block.status);

        const row =
          (div.querySelector(
            ':scope > div[data-k="row"]',
          ) as HTMLDivElement | null) ??
          (() => {
            const r = document.createElement("div");
            r.dataset.k = "row";
            r.className = "webSearchRow";
            div.appendChild(r);
            return r;
          })();
        const text = summaryQ ? `🔎 ${summaryQ}` : "🔎";
        if (row.textContent !== text) row.textContent = text;
        continue;
      }

      if (block.type === "user" || block.type === "assistant") {
        const key = "b:" + block.id;
        const div = ensureDiv(
          key,
          "msg " + (block.type === "user" ? "user" : "assistant"),
        );
        const pre = div.querySelector(`pre[data-k="body"]`);
        if (pre) pre.remove();
        const mdEl = ensureMd(div, "body");
        renderMarkdownInto(mdEl, block.text);
        continue;
      }

      if (block.type === "reasoning") {
        const summary = (block.summaryParts || []).filter(Boolean).join("");
        const raw = (block.rawParts || []).filter(Boolean).join("");

        const id = "reasoning:" + block.id;
        const det = ensureDetails(
          id,
          "reasoning",
          block.status === "inProgress",
          "Reasoning",
          id,
        );
        setStatusIcon(det, block.status);

        if (summary) {
          const pre = det.querySelector(`pre[data-k="summary"]`);
          if (pre) pre.remove();
          const mdEl = ensureMd(det, "summary");
          renderMarkdownInto(mdEl, summary);
        }
        if (raw) {
          const rawId = id + ":raw";
          const rawDet = ensureDetails(rawId, "", false, "Raw", rawId);
          // Ensure raw is nested under the reasoning details.
          if (rawDet.parentElement !== det) det.appendChild(rawDet);
          const pre = ensurePre(rawDet, "body");
          if (pre.textContent !== raw) pre.textContent = raw;
        }
        continue;
      }

      if (block.type === "command") {
        const id = "command:" + block.id;
        const displayCmd = block.command
          ? stripShellWrapper(block.command)
          : "";
        const cmdPreview =
          displayCmd && !looksOpaqueToken(displayCmd)
            ? truncateCommand(displayCmd, 120)
            : "";
        const actionsPreview = (block.actionsText || "").trim().split("\n")[0];
        const summaryText = cmdPreview
          ? `Command: ${cmdPreview}`
          : actionsPreview
            ? `Command: ${actionsPreview}`
            : block.title || "Command";
        const det = ensureDetails(id, "tool command", false, summaryText, id);
        setStatusIcon(det, block.status);

        const sum = det.querySelector(":scope > summary");
        const sumTxt = sum
          ? (sum.querySelector(
              ':scope > span[data-k="summaryText"]',
            ) as HTMLSpanElement | null)
          : null;
        if (sumTxt && block.command) {
          const raw = String(block.command || "");
          const stripped = String(displayCmd || "");
          sumTxt.title = raw !== stripped ? raw : "";
        }

        const parts = [
          block.exitCode !== null ? "exitCode=" + String(block.exitCode) : null,
          block.durationMs !== null
            ? "durationMs=" + String(block.durationMs)
            : null,
          block.cwd ? "cwd=" + block.cwd : null,
        ].filter(Boolean);
        const pre = ensurePre(det, "body");
        const next =
          (displayCmd ? "$ " + displayCmd + "\n" : "") + (block.output || "");
        if (pre.textContent !== next) pre.textContent = next;
        if (block.terminalStdin && block.terminalStdin.length > 0) {
          const stdinId = id + ":stdin";
          const stdinDet = ensureDetails(stdinId, "", false, "stdin", stdinId);
          if (stdinDet.parentElement !== det) det.appendChild(stdinDet);
          const stdinPre = ensurePre(stdinDet, "body");
          const stdinText = block.terminalStdin.join("");
          if (stdinPre.textContent !== stdinText)
            stdinPre.textContent = stdinText;
        }

        // Meta should be subtle and at the bottom.
        const meta = ensureMeta(det, "meta");
        const metaLines = [
          parts.join(" "),
          (block.actionsText || "").trim()
            ? (block.actionsText || "").trim()
            : null,
        ]
          .filter(Boolean)
          .join("\n");
        const metaText = metaLines;
        if (meta.textContent !== metaText) meta.textContent = metaText;
        det.appendChild(meta);
        continue;
      }

      if (block.type === "fileChange") {
        const id = "fileChange:" + block.id;
        const det = ensureDetails(
          id,
          "tool changes",
          false,
          block.title || "File Change",
          id,
        );
        setStatusIcon(det, block.status);

        // Render a clickable file list (Ctrl/Cmd+click to open).
        const pre = det.querySelector(`pre[data-k="body"]`);
        if (pre) pre.remove();
        const mdEl = det.querySelector(`div.md[data-k="body"]`);
        if (mdEl) mdEl.remove();
        const listEl = ensureFileList(det, "files");
        listEl.innerHTML = "";
        for (const file of block.files || []) {
          const row = document.createElement("div");
          row.className = "fileRow";
          const sp = document.createElement("span");
          sp.className = "fileLink";
          sp.dataset.openFile = file;
          sp.textContent = file;
          row.appendChild(sp);
          listEl.appendChild(row);
        }

        const detailPre = ensurePre(det, "detail");
        const detailText = block.detail || "";
        if (detailPre.textContent !== detailText)
          detailPre.textContent = detailText;

        // Per-file diffs (nested details)
        const diffs = Array.isArray(block.diffs) ? block.diffs : [];
        const wantedKeys = new Set<string>();
        for (let fi = 0; fi < diffs.length; fi++) {
          const d = diffs[fi];
          if (!d || typeof d.path !== "string" || typeof d.diff !== "string")
            continue;
          const fileId = `${id}:diff:${d.path}`;
          wantedKeys.add(fileId);

          const fileDet = ensureNestedDetails(
            det,
            fileId,
            "fileDiff",
            false,
            d.path,
            fileId,
          );
          const filePre = ensurePre(fileDet, "body");
          if (filePre.textContent !== d.diff) filePre.textContent = d.diff;
        }

        // Remove stale per-file diff nodes (files changed / compacted).
        for (const [k, el] of blockElByKey.entries()) {
          if (!k.startsWith(id + ":diff:")) continue;
          if (wantedKeys.has(k)) continue;
          if (el.parentElement) el.parentElement.removeChild(el);
          blockElByKey.delete(k);
          delete detailsState[k];
        }
        continue;
      }

      if (block.type === "mcp") {
        const id = "mcp:" + block.id;
        const det = ensureDetails(
          id,
          "tool mcp",
          false,
          block.title || "MCP",
          id,
        );
        const meta = ensureMeta(det, "meta");
        const metaText = [block.server, block.tool].filter(Boolean).join(" ");
        if (meta.textContent !== metaText) meta.textContent = metaText;
        setStatusIcon(det, block.status);
        const pre = ensurePre(det, "body");
        const text = block.detail || "";
        if (pre.textContent !== text) pre.textContent = text;
        continue;
      }

      if (block.type === "plan") {
        const id = "plan:" + block.id;
        const det = ensureDetails(
          id,
          "system",
          false,
          "Plan: " + block.title,
          id,
        );
        const pre = det.querySelector(`pre[data-k="body"]`);
        if (pre) pre.remove();
        const mdEl = ensureMd(det, "body");
        renderMarkdownInto(mdEl, block.text);
        continue;
      }

      if (block.type === "system") {
        const key = "b:" + block.id;
        const div = ensureDiv(key, "msg system");
        const pre = div.querySelector(`pre[data-k="body"]`);
        if (pre) pre.remove();
        const mdEl = ensureMd(div, "body");
        renderMarkdownInto(mdEl, block.text);
        continue;
      }

      if (block.type === "info") {
        const key = "b:" + block.id;
        const div = ensureDiv(key, "msg info");
        const pre = div.querySelector(`pre[data-k="body"]`);
        if (pre) pre.remove();
        const mdEl = ensureMd(div, "body");
        renderMarkdownInto(mdEl, block.text);
        continue;
      }

      if (block.type === "error") {
        const id = "error:" + block.id;
        const det = ensureDetails(
          id,
          "system",
          true,
          "Error: " + block.title,
          id,
        );
        const pre = det.querySelector(`pre[data-k="body"]`);
        if (pre) pre.remove();
        const mdEl = ensureMd(det, "body");
        renderMarkdownInto(mdEl, block.text);
        continue;
      }

      const _exhaustive: never = block;
      void _exhaustive;
    }

    updateSuggestions();

    if (shouldAutoScroll) {
      logEl.scrollTop = logEl.scrollHeight;
    }
  }

function sendCurrentInput(): void {
    if (!state.activeSession) return;
    if (state.sending) return;
    const text = inputEl.value;
    const trimmed = text.trim();
    if (!trimmed) return;
    vscode.postMessage({ type: "send", text });

    // Keep a simple history for navigating with Up/Down.
    const last = inputHistory.at(-1);
    if (last !== trimmed) inputHistory.push(trimmed);
    historyIndex = null;
    draftBeforeHistory = "";

    inputEl.value = "";
    inputEl.setSelectionRange(0, 0);
    autosizeInput();
    updateSuggestions();
  }

  function stopCurrentTurn(): void {
    if (!state.activeSession) return;
    if (!state.sending) return;
    vscode.postMessage({ type: "stop" });
  }

  sendBtn.addEventListener("click", () =>
    state.sending ? stopCurrentTurn() : sendCurrentInput(),
  );
  newBtn.addEventListener("click", () =>
    vscode.postMessage({ type: "newSession" }),
  );
  statusBtn.addEventListener("click", () =>
    vscode.postMessage({ type: "showStatus" }),
  );
  diffBtn.addEventListener("click", () =>
    vscode.postMessage({ type: "openDiff" }),
  );

  inputEl.addEventListener("input", () => updateSuggestions());
  inputEl.addEventListener("input", () => autosizeInput());
  inputEl.addEventListener("click", () => updateSuggestions());
  inputEl.addEventListener("keyup", (e) => {
    const key = (e as KeyboardEvent).key;
    if ((key === "ArrowDown" || key === "ArrowUp") && suggestItems.length > 0) return;
    updateSuggestions();
  });
  inputEl.addEventListener("compositionstart", () => {
    isComposing = true;
  });
  inputEl.addEventListener("compositionend", () => {
    isComposing = false;
  });

  inputEl.addEventListener("keydown", (e) => {
    if (
      (e as KeyboardEvent).key === "Enter" &&
      !(e as KeyboardEvent).shiftKey
    ) {
      if ((e as KeyboardEvent).isComposing || isComposing) return;
      if (suggestItems.length > 0 && activeReplace) {
        e.preventDefault();
        acceptSuggestion(suggestIndex);
        return;
      }
      e.preventDefault();
      sendCurrentInput();
      return;
    }
    if ((e as KeyboardEvent).key === "Escape" && state.sending) {
      e.preventDefault();
      stopCurrentTurn();
      return;
    }
    if ((e as KeyboardEvent).key === "ArrowDown" && suggestItems.length > 0) {
      e.preventDefault();
      suggestIndex = Math.min(suggestItems.length - 1, suggestIndex + 1);
      renderSuggest();
      return;
    }
    if ((e as KeyboardEvent).key === "ArrowUp" && suggestItems.length > 0) {
      e.preventDefault();
      suggestIndex = Math.max(0, suggestIndex - 1);
      renderSuggest();
      return;
    }
    if ((e as KeyboardEvent).key === "ArrowUp") {
      if ((e as KeyboardEvent).shiftKey) return;
      if ((e as KeyboardEvent).altKey) return;
      if ((e as KeyboardEvent).metaKey) return;
      if ((e as KeyboardEvent).ctrlKey) return;
      if ((e as KeyboardEvent).isComposing) return;
      if (suggestItems.length > 0) return;

      const cur = inputEl.selectionStart ?? 0;
      const end = inputEl.selectionEnd ?? 0;
      if (cur !== end) return;
      if (cur !== 0) return;
      if (inputHistory.length === 0) return;
      e.preventDefault();

      if (historyIndex === null) {
        draftBeforeHistory = inputEl.value;
        historyIndex = inputHistory.length - 1;
      } else {
        historyIndex = Math.max(0, historyIndex - 1);
      }

      inputEl.value = inputHistory[historyIndex] || "";
      const pos = inputEl.value.length;
      inputEl.setSelectionRange(pos, pos);
      autosizeInput();
      updateSuggestions();
      return;
    }
    if ((e as KeyboardEvent).key === "ArrowDown") {
      if ((e as KeyboardEvent).shiftKey) return;
      if ((e as KeyboardEvent).altKey) return;
      if ((e as KeyboardEvent).metaKey) return;
      if ((e as KeyboardEvent).ctrlKey) return;
      if ((e as KeyboardEvent).isComposing) return;
      if (suggestItems.length > 0) return;

      if (historyIndex === null) return;
      e.preventDefault();

      historyIndex += 1;
      if (historyIndex >= inputHistory.length) {
        historyIndex = null;
        inputEl.value = draftBeforeHistory;
        draftBeforeHistory = "";
      } else {
        inputEl.value = inputHistory[historyIndex] || "";
      }
      const pos = inputEl.value.length;
      inputEl.setSelectionRange(pos, pos);
      autosizeInput();
      updateSuggestions();
      return;
    }
    if ((e as KeyboardEvent).key === "Escape" && suggestItems.length > 0) {
      e.preventDefault();
      suggestItems = [];
      activeReplace = null;
      renderSuggest();
      return;
    }
  });

  window.addEventListener("message", (event: MessageEvent) => {
    const msg = event.data;
    if (!msg || typeof msg !== "object") return;
    const anyMsg = msg as { type?: unknown; state?: unknown; files?: unknown };
    if (anyMsg.type === "state") {
      receivedState = true;
      render(anyMsg.state as ChatViewState);
      autosizeInput();
      return;
    }
    if (anyMsg.type === "fileIndex") {
      const files = Array.isArray(anyMsg.files)
        ? (anyMsg.files.filter((f) => typeof f === "string") as string[])
        : [];
      if (state.activeSession) {
        fileIndex = files;
        fileIndexForSessionId = state.activeSession.id;
      } else {
        fileIndex = null;
        fileIndexForSessionId = null;
      }
      renderSuggest();
      return;
    }
  });

  // Open links via the extension host.
  document.addEventListener("click", (e) => {
    const t = e.target as HTMLElement | null;
    const fileLink = t
      ? (t.closest("[data-open-file]") as HTMLElement | null)
      : null;
    if (fileLink) {
      const file = fileLink.getAttribute("data-open-file") || "";
      const me = e as MouseEvent;
      if (file && (me.ctrlKey || me.metaKey)) {
        e.preventDefault();
        vscode.postMessage({ type: "openFile", path: file });
        return;
      }
    }

    const a = t ? (t.closest("a") as HTMLAnchorElement | null) : null;
    if (!a) return;
    const href = a.getAttribute("href") || "";
    if (!href) return;

    // Markdown links: relative paths open files; external URLs open externally.
    // This is intentionally non-heuristic: we only act on explicit links.
    if (href.startsWith("#")) return;

    const decoded = (() => {
      try {
        return decodeURIComponent(href);
      } catch {
        return href;
      }
    })();

    const schemeMatch = decoded.match(/^([a-zA-Z][a-zA-Z0-9+.-]*):/);
    const scheme = schemeMatch ? schemeMatch[1]?.toLowerCase() : null;
    if (scheme) {
      if (scheme === "file") {
        const without = decoded.replace(/^file:(\/\/)?/, "");
        const normalized = without.replace(/^\/+/, "");
        e.preventDefault();
        vscode.postMessage({ type: "openFile", path: normalized });
        return;
      }
      // Unknown/unsupported schemes are delegated to VS Code's openExternal.
      e.preventDefault();
      vscode.postMessage({ type: "openExternal", url: decoded });
      return;
    }

    // Treat "/path" as workspace-root relative (GitHub-style links).
    const normalized = decoded.replace(/^\/+/, "");
    e.preventDefault();
    vscode.postMessage({ type: "openFile", path: normalized });
  });

  // Handshake
  vscode.postMessage({ type: "ready" });

  // If we never receive state, show a hint.
  setTimeout(() => {
    if (!receivedState) {
      statusTextEl.textContent =
        "Waiting for state… (check Extension Host logs)";
      statusTextEl.style.display = "";
    }
  }, 1000);
}

main();
