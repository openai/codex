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

type Session = {
  id: string;
  title: string;
  customTitle?: boolean;
  workspaceFolderUri: string;
  backendId?: "codex" | "codez" | "opencode";
};
type ModelState = {
  model: string | null;
  provider: string | null;
  reasoning: string | null;
  agent?: string | null;
};

type ChatBlock =
  | { id: string; type: "user"; text: string }
  | {
      id: string;
      type: "assistant";
      text: string;
      streaming?: boolean;
      meta?: string | null;
    }
  | {
      id: string;
      type: "opencodePermission";
      requestID: string;
      permission: string;
      status: "pending" | "replied" | "error";
      patterns: string[];
      always: string[];
      metadata: Record<string, unknown> | null;
      reply?: "once" | "always" | "reject" | null;
      error?: string | null;
    }
  | {
      id: string;
      type: "divider";
      text: string;
      status?: "inProgress" | "completed" | "failed";
    }
  | { id: string; type: "note"; text: string }
  | {
      id: string;
      type: "image";
      title: string;
      src: string;
      // Offloaded images omit `src` and use `imageKey` to request data on-demand.
      imageKey?: string;
      mimeType?: string;
      byteLength?: number;
      autoLoad?: boolean;
      alt: string;
      caption: string | null;
      role: "user" | "assistant" | "tool" | "system";
    }
  | {
      id: string;
      type: "imageGallery";
      title: string;
      images: Array<{
        title: string;
        src: string;
        // Offloaded images omit `src` and use `imageKey` to request data on-demand.
        imageKey?: string;
        mimeType?: string;
        byteLength?: number;
        autoLoad?: boolean;
        alt: string;
        caption: string | null;
      }>;
      role: "user" | "assistant" | "tool" | "system";
    }
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
      hideCommandText?: boolean;
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
  | {
      id: string;
      type: "step";
      title: string;
      status: "inProgress" | "completed" | "failed";
      snapshot: string | null;
      reason: string | null;
      cost: number | null;
      tokens: {
        input?: number;
        output?: number;
        reasoning?: number;
        cache?: { read?: number; write?: number };
      } | null;
      tools: Array<{
        id: string;
        tool: string;
        title: string;
        status: "inProgress" | "completed" | "failed";
        inputPreview?: string | null;
        detail: string;
      }>;
    }
  | { id: string; type: "plan"; title: string; text: string }
  | { id: string; type: "error"; title: string; text: string }
  | { id: string; type: "system"; title: string; text: string };

type ChatViewState = {
  globalBlocks?: ChatBlock[];
  capabilities?: {
    agents: boolean;
  };
  workspaceColorOverrides?: Record<string, number>;
  sessions: Session[];
  activeSession: Session | null;
  unreadSessionIds: string[];
  runningSessionIds: string[];
  blocks: ChatBlock[];
  hasLatestDiff: boolean;
  sending: boolean;
  reloading: boolean;
  hydrationBlockedText?: string | null;
  opencodeDefaultModelKey?: string | null;
  opencodeDefaultAgentName?: string | null;
  cliDefaultModelState?: ModelState | null;
  statusText?: string | null;
  statusTooltip?: string | null;
  modelState?: ModelState | null;
  models?: Array<{
    id: string;
    model: string;
    displayName: string;
    description: string;
    supportedReasoningEfforts: Array<{
      reasoningEffort: string;
      description: string;
    }>;
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
  opencodeAgents?: Array<{
    id: string;
    name: string;
    description?: string;
  }> | null;
};

type AskUserQuestionType = "text" | "single_select" | "multi_select";
type AskUserQuestionOption = {
  label: string;
  value: string;
  description?: string;
  recommended?: boolean;
};
type AskUserQuestion = {
  id: string;
  prompt: string;
  type: AskUserQuestionType;
  description?: string;
  placeholder?: string;
  options?: AskUserQuestionOption[];
  allow_other?: boolean;
  required?: boolean;
};
type AskUserQuestionRequest = {
  title?: string;
  questions: AskUserQuestion[];
};
type AskUserQuestionResponse = {
  cancelled: boolean;
  answers: Record<string, unknown>;
};

type SuggestItem = {
  insert: string;
  label: string;
  detail?: string;
  kind: "slash" | "at" | "file" | "dir" | "agent" | "skill";
};

const WORKTREE_COLORS = [
  "#1f6feb",
  "#2ea043",
  "#d29922",
  "#db6d28",
  "#f85149",
  "#a371f7",
  "#ff7b72",
  "#7ee787",
  "#ffa657",
  "#79c0ff",
  "#d2a8ff",
  "#c9d1d9",
] as const;

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
  const maybeGet = <T extends HTMLElement = HTMLElement>(
    id: string,
  ): T | null => (document.getElementById(id) as T | null) ?? null;

  const titleEl = mustGet("title");
  const statusTextEl = mustGet("statusText");
  const hydrateBannerEl = mustGet<HTMLDivElement>("hydrateBanner");
  const logEl = mustGet("log");
  const approvalsEl = mustGet("approvals");
  const askUserQuestionEl = mustGet("askUserQuestion");
  const composerEl = mustGet("composer");
  const editBannerEl = mustGet("editBanner");
  const toastEl = mustGet<HTMLDivElement>("toast");
  const inputRowEl = mustGet("inputRow");
  const inputEl = mustGet<HTMLTextAreaElement>("input");
  const imageInput = mustGet<HTMLInputElement>("imageInput");
  const attachBtn = mustGet<HTMLButtonElement>("attach");
  const attachmentsEl = mustGet("attachments");
  const returnToBottomBtn = mustGet<HTMLButtonElement>("returnToBottom");
  const sendBtn = mustGet<HTMLButtonElement>("send");
  const newBtn = mustGet<HTMLButtonElement>("new");
  const resumeBtn = mustGet<HTMLButtonElement>("resume");
  const reloadBtn = mustGet<HTMLButtonElement>("reload");
  const settingsBtn = mustGet<HTMLButtonElement>("settings");
  const settingsOverlayEl = mustGet<HTMLDivElement>("settingsOverlay");
  const settingsCloseBtn = mustGet<HTMLButtonElement>("settingsClose");
  const settingsBodyEl = mustGet<HTMLDivElement>("settingsBody");
  const diffBtn = maybeGet<HTMLButtonElement>("diff");
  const statusBtn = maybeGet<HTMLButtonElement>("status");
  const tabsEl = mustGet("tabs");
  const modelBarEl = mustGet("modelBar");
  const footerBarEl = (() => {
    const el = statusTextEl.parentElement;
    if (!el)
      throw new Error(
        "Webview DOM element missing: statusText parent (footerBar)",
      );
    return el as HTMLElement;
  })();
  const statusPopoverEl = mustGet("statusPopover");
  const modelSelect = document.createElement("select");
  modelSelect.className = "modelSelect model";
  const reasoningSelect = document.createElement("select");
  reasoningSelect.className = "modelSelect effort";
  const agentSelect = document.createElement("select");
  agentSelect.className = "modelSelect agent";
  // For opencode sessions, `agentSelect` is used as the Build/Plan mode selector.
  // Keep it to the left of model selection.
  modelBarEl.appendChild(agentSelect);
  modelBarEl.appendChild(modelSelect);
  modelBarEl.appendChild(reasoningSelect);

  const placeholder = {
    // Keep the placeholder short (single-line). Put detailed hints in title.
    wide: "Type a message",
    narrow: "Message",
    tiny: "",
    hint: "Type a message (Enter to send / Shift+Enter for newline)",
  } as const;

  const updateInputPlaceholder = (): void => {
    // Prevent placeholder wrapping by keeping it short; use title for the full hint.
    const w = inputEl.clientWidth;
    const next =
      w >= 260
        ? placeholder.wide
        : w >= 170
          ? placeholder.narrow
          : placeholder.tiny;
    if (inputEl.placeholder !== next) inputEl.placeholder = next;
    inputEl.title = placeholder.hint;
  };

  // Respond to sidebar resizing / layout changes without introducing wraps.
  const placeholderObserver = new ResizeObserver(() =>
    updateInputPlaceholder(),
  );
  placeholderObserver.observe(inputRowEl);

  let askUserQuestionState: {
    requestKey: string;
    request: AskUserQuestionRequest;
    index: number;
    answers: Record<string, unknown>;
    otherTextById: Record<string, string>;
  } | null = null;
  let askUserQuestionError: string | null = null;

  const disableComposer = (disabled: boolean): void => {
    inputEl.disabled = disabled;
    imageInput.disabled = disabled;
    attachBtn.disabled = disabled;
    sendBtn.disabled = disabled;
  };

  const clearAskUserQuestion = (): void => {
    askUserQuestionState = null;
    askUserQuestionError = null;
    askUserQuestionEl.style.display = "none";
    askUserQuestionEl.innerHTML = "";
    disableComposer(false);
    inputEl.focus();
  };

  const postAskUserQuestionResponse = (
    response: AskUserQuestionResponse,
  ): void => {
    const st = askUserQuestionState;
    if (!st) return;
    vscode.postMessage({
      type: "askUserQuestionResponse",
      requestKey: st.requestKey,
      response,
    });
  };

  const renderAskUserQuestion = (): void => {
    const st = askUserQuestionState;
    if (!st) {
      askUserQuestionEl.style.display = "none";
      askUserQuestionEl.innerHTML = "";
      disableComposer(false);
      return;
    }

    const title =
      typeof st.request.title === "string" && st.request.title.trim().length > 0
        ? st.request.title.trim()
        : "Codex question";

    const questions = Array.isArray(st.request.questions)
      ? st.request.questions
      : [];
    const q = questions[st.index] ?? null;
    if (!q || typeof q.id !== "string" || typeof q.prompt !== "string") {
      postAskUserQuestionResponse({ cancelled: true, answers: st.answers });
      clearAskUserQuestion();
      return;
    }

    // Override the default `.askUserQuestion { display: none; }` stylesheet rule.
    askUserQuestionEl.style.display = "block";
    askUserQuestionEl.innerHTML = "";

    const card = document.createElement("div");
    card.className = "askCard";

    const header = document.createElement("div");
    header.className = "askHeader";

    const titleNode = document.createElement("div");
    titleNode.className = "askTitle";
    titleNode.textContent = title;

    const progress = document.createElement("div");
    progress.className = "askProgress";
    progress.textContent = `${String(st.index + 1)}/${String(questions.length)}`;

    header.appendChild(titleNode);
    header.appendChild(progress);

    const prompt = document.createElement("div");
    prompt.className = "askPrompt";
    prompt.textContent = q.prompt;

    card.appendChild(header);
    card.appendChild(prompt);

    if (typeof q.description === "string" && q.description) {
      const desc = document.createElement("div");
      desc.className = "askDesc";
      desc.textContent = q.description;
      card.appendChild(desc);
    }

    const required = Boolean(q.required);
    const allowOther = Boolean(q.allow_other);
    const otherSentinel = "__other__";

    const errorEl = document.createElement("div");
    errorEl.className = "askError";
    errorEl.style.display = askUserQuestionError ? "" : "none";
    errorEl.textContent = askUserQuestionError || "";

    const setError = (msg: string | null): void => {
      askUserQuestionError = msg;
      errorEl.textContent = msg || "";
      errorEl.style.display = msg ? "" : "none";
    };

    const commitAndAdvance = (answer: unknown | null): void => {
      setError(null);
      if (answer === null) delete st.answers[q.id];
      else st.answers[q.id] = answer;

      st.index += 1;
      if (st.index >= questions.length) {
        postAskUserQuestionResponse({ cancelled: false, answers: st.answers });
        clearAskUserQuestion();
        return;
      }
      renderAskUserQuestion();
    };

    const controls = document.createElement("div");
    controls.className = "askControls";

    const cancelBtn = document.createElement("button");
    cancelBtn.className = "askBtn";
    cancelBtn.textContent = "Cancel";
    cancelBtn.addEventListener("click", () => {
      postAskUserQuestionResponse({ cancelled: true, answers: st.answers });
      clearAskUserQuestion();
    });

    const nextBtn = document.createElement("button");
    nextBtn.className = "askBtn primary";
    nextBtn.textContent = st.index + 1 >= questions.length ? "Submit" : "Next";

    const optionsWrap = document.createElement("div");
    optionsWrap.className = "askOptions";

    const renderOtherInputIfNeeded = (
      host: HTMLElement,
      checked: () => boolean,
      placeholder: string,
    ): HTMLInputElement => {
      const input = document.createElement("input");
      input.className = "askInput";
      input.placeholder = placeholder;
      input.value = st.otherTextById[q.id] ?? "";
      input.addEventListener("input", () => {
        st.otherTextById[q.id] = input.value;
        if (checked()) setError(null);
      });
      const wrapper = document.createElement("div");
      wrapper.style.marginTop = "8px";
      wrapper.appendChild(input);
      host.appendChild(wrapper);
      return input;
    };

    if (q.type === "text") {
      const input = document.createElement("textarea");
      input.className = "askInput";
      input.rows = 3;
      input.placeholder =
        typeof q.placeholder === "string" ? q.placeholder : "";
      input.value =
        typeof st.answers[q.id] === "string"
          ? (st.answers[q.id] as string)
          : "";
      input.addEventListener("input", () => setError(null));
      card.appendChild(input);

      nextBtn.addEventListener("click", () => {
        const v = input.value.trim();
        if (required && !v) {
          setError("This question is required.");
          input.focus();
          return;
        }
        commitAndAdvance(v ? v : null);
      });
    } else if (q.type === "single_select") {
      const selected =
        typeof st.answers[q.id] === "string"
          ? (st.answers[q.id] as string)
          : null;

      const rawOptions = Array.isArray(q.options) ? q.options : [];
      const options = rawOptions
        .slice()
        .sort(
          (a, b) =>
            Number(Boolean(b.recommended)) - Number(Boolean(a.recommended)),
        );

      const makeOptionRow = (opt: AskUserQuestionOption): HTMLLabelElement => {
        const row = document.createElement("label");
        row.className = "askOption";
        row.style.cursor = "pointer";

        const radio = document.createElement("input");
        radio.type = "radio";
        radio.name = `ask_${st.requestKey}_${q.id}`;
        radio.value = opt.value;
        radio.checked = selected === opt.value;
        radio.addEventListener("change", () => {
          st.answers[q.id] = opt.value;
          setError(null);
          renderAskUserQuestion();
        });

        const meta = document.createElement("div");
        const label = document.createElement("div");
        label.className = "askOptionLabel";
        label.textContent = opt.recommended
          ? `${opt.label} (Recommended)`
          : opt.label;
        meta.appendChild(label);
        if (opt.description) {
          const d = document.createElement("div");
          d.className = "askOptionMeta";
          d.textContent = opt.description;
          meta.appendChild(d);
        }

        row.appendChild(radio);
        row.appendChild(meta);
        return row;
      };

      for (const opt of options) optionsWrap.appendChild(makeOptionRow(opt));
      if (allowOther) {
        optionsWrap.appendChild(
          makeOptionRow({ label: "Other…", value: otherSentinel }),
        );
      }
      card.appendChild(optionsWrap);

      let otherInput: HTMLInputElement | null = null;
      if (allowOther && selected === otherSentinel) {
        otherInput = renderOtherInputIfNeeded(
          card,
          () => selected === otherSentinel,
          typeof q.placeholder === "string"
            ? q.placeholder
            : "Type your answer…",
        );
      }

      nextBtn.addEventListener("click", () => {
        if (!selected) {
          if (required) {
            setError("Pick an option, or cancel.");
            return;
          }
          commitAndAdvance(null);
          return;
        }

        if (selected === otherSentinel) {
          const v = (otherInput?.value ?? st.otherTextById[q.id] ?? "").trim();
          if (required && !v) {
            setError("This question is required.");
            otherInput?.focus();
            return;
          }
          commitAndAdvance(v ? v : null);
          return;
        }

        commitAndAdvance(selected);
      });
    } else if (q.type === "multi_select") {
      const prev = Array.isArray(st.answers[q.id])
        ? (st.answers[q.id] as unknown[])
        : [];
      const selected = new Set<string>(prev.map((v) => String(v)));

      const rawOptions = Array.isArray(q.options) ? q.options : [];
      const options = rawOptions
        .slice()
        .sort(
          (a, b) =>
            Number(Boolean(b.recommended)) - Number(Boolean(a.recommended)),
        );

      const makeOptionRow = (opt: AskUserQuestionOption): HTMLLabelElement => {
        const row = document.createElement("label");
        row.className = "askOption";
        row.style.cursor = "pointer";

        const cb = document.createElement("input");
        cb.type = "checkbox";
        cb.value = opt.value;
        cb.checked = selected.has(opt.value);
        cb.addEventListener("change", () => {
          if (cb.checked) selected.add(opt.value);
          else selected.delete(opt.value);
          st.answers[q.id] = Array.from(selected);
          setError(null);
          renderAskUserQuestion();
        });

        const meta = document.createElement("div");
        const label = document.createElement("div");
        label.className = "askOptionLabel";
        label.textContent = opt.recommended
          ? `${opt.label} (Recommended)`
          : opt.label;
        meta.appendChild(label);
        if (opt.description) {
          const d = document.createElement("div");
          d.className = "askOptionMeta";
          d.textContent = opt.description;
          meta.appendChild(d);
        }

        row.appendChild(cb);
        row.appendChild(meta);
        return row;
      };

      for (const opt of options) optionsWrap.appendChild(makeOptionRow(opt));
      if (allowOther) {
        optionsWrap.appendChild(
          makeOptionRow({ label: "Other…", value: otherSentinel }),
        );
      }
      card.appendChild(optionsWrap);

      let otherInput: HTMLInputElement | null = null;
      if (allowOther && selected.has(otherSentinel)) {
        otherInput = renderOtherInputIfNeeded(
          card,
          () => selected.has(otherSentinel),
          typeof q.placeholder === "string"
            ? q.placeholder
            : "Type your answer…",
        );
      }

      nextBtn.addEventListener("click", () => {
        const out: string[] = [];
        for (const v of selected) {
          if (v === otherSentinel) continue;
          out.push(v);
        }

        if (allowOther && selected.has(otherSentinel)) {
          const other = (
            otherInput?.value ??
            st.otherTextById[q.id] ??
            ""
          ).trim();
          if (other) out.push(other);
        }

        if (required && out.length === 0) {
          setError("Pick at least one option, or cancel.");
          return;
        }

        commitAndAdvance(out.length > 0 ? out : null);
      });
    } else {
      setError(`Unsupported question type: ${String((q as any).type)}`);
      nextBtn.addEventListener("click", () => {
        postAskUserQuestionResponse({ cancelled: true, answers: st.answers });
        clearAskUserQuestion();
      });
    }

    controls.appendChild(cancelBtn);
    controls.appendChild(nextBtn);
    card.appendChild(errorEl);
    card.appendChild(controls);
    askUserQuestionEl.appendChild(card);
    disableComposer(true);
  };

  let statusPopoverOpen = false;
  let statusPopoverDetails = "";
  let statusHoverDetails = "";
  let statusPopoverEnabled = false;
  let statusTextHovering = false;

  const hideStatusPopover = (): void => {
    statusPopoverOpen = false;
    statusPopoverEl.style.display = "none";
    statusPopoverEl.textContent = "";
  };

  const toggleStatusPopover = (): void => {
    if (!statusPopoverDetails) return;
    statusPopoverOpen = !statusPopoverOpen;
    if (!statusPopoverOpen) {
      hideStatusPopover();
      return;
    }
    statusPopoverEl.textContent = statusPopoverDetails;
    statusPopoverEl.style.display = "";
  };

  statusTextEl.addEventListener("click", (e) => {
    if (!statusPopoverEnabled || !statusPopoverDetails) return;
    e.preventDefault();
    e.stopPropagation();
    toggleStatusPopover();
  });
  statusTextEl.addEventListener("mouseenter", () => {
    statusTextHovering = true;
    if (statusPopoverOpen) return;
    if (!statusHoverDetails) return;
    statusPopoverEl.textContent = statusHoverDetails;
    statusPopoverEl.style.display = "";
  });
  statusTextEl.addEventListener("mouseleave", () => {
    statusTextHovering = false;
    if (statusPopoverOpen) return;
    hideStatusPopover();
  });
  document.addEventListener("click", () => hideStatusPopover());
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") hideStatusPopover();
  });

  type ParsedFooterStatus = {
    ctxPercent: string | null;
    ctxDetail: string | null;
    limitA: { label: string; percent: string } | null;
    limitB: { label: string; percent: string } | null;
  };

  const parseFooterStatus = (fullStatus: string): ParsedFooterStatus => {
    const status = fullStatus.trim();

    const ctx = (() => {
      const m = status.match(/\bremaining=([0-9]{1,3})%\s*\(([^)]+)\)/i);
      if (m) return { pct: m[1] ?? null, detail: (m[2] ?? "").trim() || null };
      const m2 = status.match(/\bctx\s+remaining=([0-9]{1,3})%/i);
      if (m2) return { pct: m2[1] ?? null, detail: null };
      return { pct: null, detail: null };
    })();

    const limits = (() => {
      const found: Array<{ label: string; percent: string }> = [];
      const re = /\b([a-zA-Z0-9]+)[:=]([0-9]+(?:\.[0-9]+)?)%\b/g;
      for (const m of status.matchAll(re)) {
        const rawLabel = (m[1] ?? "").trim();
        const percent = (m[2] ?? "").trim();
        if (!rawLabel || !percent) continue;
        const lower = rawLabel.toLowerCase();
        if (lower === "remaining" || lower === "ctx") continue;
        const label =
          lower === "primary" ? "5h" : lower === "secondary" ? "wk" : rawLabel;
        found.push({ label, percent });
      }
      const rank = (label: string): number => {
        const lower = label.toLowerCase();
        if (lower === "5h") return 0;
        if (lower === "wk") return 1;
        return 10;
      };
      const byLabel = new Map<string, { label: string; percent: string }>();
      for (const it of found) {
        if (!byLabel.has(it.label)) byLabel.set(it.label, it);
      }
      return [...byLabel.values()].sort(
        (a, b) => rank(a.label) - rank(b.label),
      );
    })();

    const a = limits[0] ?? null;
    const b = limits[1] ?? null;

    return {
      ctxPercent: ctx.pct,
      ctxDetail: ctx.detail,
      limitA: a,
      limitB: b,
    };
  };

  const fmtPctCompact = (raw: string): string => {
    const n = Number(raw);
    if (!Number.isFinite(n)) return raw;
    const rounded = Math.round(n * 100) / 100;
    return String(rounded)
      .replace(/\.0+$/, "")
      .replace(/(\.\d*[1-9])0+$/, "$1");
  };

  const buildFooterStatusCandidates = (
    p: ParsedFooterStatus,
  ): Array<{ tier: number; text: string }> => {
    const hasCtx = !!p.ctxPercent;
    const hasLimits = !!p.limitA || !!p.limitB;
    if (!hasCtx && !hasLimits) return [];

    const ctxTier0 = (() => {
      if (!p.ctxPercent) return null;
      if (p.ctxDetail) return `ctx ${p.ctxPercent}% (${p.ctxDetail})`;
      return `ctx ${p.ctxPercent}%`;
    })();
    const ctxTier1 = p.ctxPercent ? `ctx ${p.ctxPercent}%` : null;
    const ctxTier2 = p.ctxPercent ? `${p.ctxPercent}%` : null;

    const limitsTier0 = (() => {
      const parts: string[] = [];
      if (p.limitA)
        parts.push(`${p.limitA.label}:${fmtPctCompact(p.limitA.percent)}%`);
      if (p.limitB)
        parts.push(`${p.limitB.label}:${fmtPctCompact(p.limitB.percent)}%`);
      return parts.length > 0 ? parts.join(" ") : null;
    })();
    const limitsTier1 = (() => {
      const parts: string[] = [];
      if (p.limitA)
        parts.push(`${p.limitA.label}:${fmtPctCompact(p.limitA.percent)}`);
      if (p.limitB)
        parts.push(`${p.limitB.label}:${fmtPctCompact(p.limitB.percent)}`);
      return parts.length > 0 ? parts.join(" ") : null;
    })();
    const limitsTier2 = (() => {
      const a = p.limitA ? fmtPctCompact(p.limitA.percent) : null;
      const b = p.limitB ? fmtPctCompact(p.limitB.percent) : null;
      if (a && b) return `L:${a}/${b}`;
      if (a && p.limitA) return `${p.limitA.label}:${a}`;
      if (b && p.limitB) return `${p.limitB.label}:${b}`;
      return null;
    })();

    const join = (a: string | null, b: string | null): string | null => {
      const parts = [a, b].filter(
        (v): v is string => typeof v === "string" && v.length > 0,
      );
      return parts.length > 0 ? parts.join(" • ") : null;
    };

    const t0 = join(ctxTier0, limitsTier0);
    const t1 = join(ctxTier1, limitsTier1);
    const t2 = join(ctxTier2, limitsTier2);

    return [
      { tier: 0, text: t0 ?? "" },
      { tier: 1, text: t1 ?? "" },
      { tier: 2, text: t2 ?? "" },
      { tier: 3, text: "ⓘ" },
    ].filter((c) => c.text.length > 0);
  };

  const fitsStatusText = (): boolean =>
    statusTextEl.scrollWidth <= statusTextEl.clientWidth + 1;

  const populateSelect = (
    el: HTMLSelectElement,
    options: string[],
    value: string | null | undefined,
  ): void => {
    const v = (value && value.trim()) || "default";
    const opts = options.includes(v) ? options : [v, ...options];
    const sig = v + "\n" + opts.join("\n");
    if (el.dataset.sig === sig) return;
    el.dataset.sig = sig;
    el.innerHTML = "";
    for (const opt of opts) {
      const o = document.createElement("option");
      o.value = opt;
      o.textContent = opt === "default" ? "default" : opt;
      if (opt === v) o.selected = true;
      el.appendChild(o);
    }
  };

  const populateSelectWithLabels = (
    el: HTMLSelectElement,
    options: Array<{ value: string; label: string }>,
    value: string | null | undefined,
    opts?: { defaultLabel?: string | null },
  ): void => {
    const v = (value && value.trim()) || "default";
    const wanted = options.some((o) => o.value === v)
      ? options
      : [{ value: v, label: v }, ...options];
    const sig =
      v +
      "\n" +
      wanted.map((o) => `${o.value}\t${o.label}`).join("\n") +
      "\n" +
      String(opts?.defaultLabel ?? "");
    if (el.dataset.sig === sig) return;
    el.dataset.sig = sig;
    el.innerHTML = "";
    for (const opt of wanted) {
      const o = document.createElement("option");
      o.value = opt.value;
      o.textContent =
        opt.value === "default"
          ? (opts?.defaultLabel ?? "default")
          : opt.label;
      if (opt.value === v) o.selected = true;
      el.appendChild(o);
    }
  };

  const sendModelState = (): void => {
    if (!domSessionId) {
      showToast("error", "No session selected.");
      return;
    }
    const backendId = state.activeSession?.backendId ?? null;
    vscode.postMessage({
      type: "setModel",
      sessionId: domSessionId,
      model: modelSelect.value === "default" ? null : modelSelect.value,
      reasoning:
        reasoningSelect.value === "default" ? null : reasoningSelect.value,
      agent:
        backendId === "opencode" && agentSelect.value !== "default"
          ? agentSelect.value
          : null,
    });
  };

  modelSelect.addEventListener("change", sendModelState);
  reasoningSelect.addEventListener("change", sendModelState);
  agentSelect.addEventListener("change", sendModelState);
  const suggestEl = mustGet("suggest");
  type PendingImage = { id: string; name: string; url: string };

  type PersistedWebviewState = {
    detailsState?: Record<string, boolean>;
    composerDrafts?: Record<
      string,
      { text: string; selectionStart: number; selectionEnd: number }
    >;
  };

  let persistedWebviewState: PersistedWebviewState =
    (vscode.getState() as PersistedWebviewState | undefined) || {};

  const updatePersistedWebviewState = (
    patch: Partial<PersistedWebviewState>,
  ): void => {
    persistedWebviewState = { ...persistedWebviewState, ...patch };
    vscode.setState(persistedWebviewState);
  };

  // Composer state is per session so drafts/attachments don't leak across tabs.
  type ComposerState = {
    text: string;
    selectionStart: number;
    selectionEnd: number;
    pendingImages: PendingImage[];
  };

  const NO_SESSION_KEY = "__no_session__";
  const composerBySessionKey = new Map<string, ComposerState>();
  let activeComposerKey = NO_SESSION_KEY;
  let pendingImages: PendingImage[] = [];

  const composerKeyForSessionId = (sessionId: string | null): string =>
    sessionId ?? NO_SESSION_KEY;

  const ensureComposerState = (key: string): ComposerState => {
    const existing = composerBySessionKey.get(key);
    if (existing) return existing;
    const next: ComposerState = {
      text: "",
      selectionStart: 0,
      selectionEnd: 0,
      pendingImages: [],
    };
    composerBySessionKey.set(key, next);
    return next;
  };

  const saveComposerState = (): void => {
    const st = ensureComposerState(activeComposerKey);
    st.text = inputEl.value;
    st.selectionStart = inputEl.selectionStart ?? inputEl.value.length;
    st.selectionEnd = inputEl.selectionEnd ?? inputEl.value.length;
    st.pendingImages = pendingImages;

    const drafts = { ...(persistedWebviewState.composerDrafts ?? {}) };
    drafts[activeComposerKey] = {
      text: st.text,
      selectionStart: st.selectionStart,
      selectionEnd: st.selectionEnd,
    };
    updatePersistedWebviewState({ composerDrafts: drafts });
  };

  const restoreComposerState = (
    sessionId: string | null,
    opts?: { updateSuggestions?: boolean },
  ): void => {
    activeComposerKey = composerKeyForSessionId(sessionId);
    const st = ensureComposerState(activeComposerKey);
    pendingImages = st.pendingImages;
    inputEl.value = st.text;
    autosizeInput();
    try {
      const start = Math.max(
        0,
        Math.min(st.selectionStart, inputEl.value.length),
      );
      const end = Math.max(0, Math.min(st.selectionEnd, inputEl.value.length));
      inputEl.setSelectionRange(start, end);
    } catch {
      // ignore
    }
    renderAttachments();
    if (opts?.updateSuggestions ?? true) updateSuggestions();
  };

  // Input history is per session so Up/Down navigation doesn't leak across tabs.
  type InputHistoryState = {
    items: string[];
    index: number | null;
    draftBeforeHistory: string;
  };

  const inputHistoryBySessionKey = new Map<string, InputHistoryState>();

  const ensureInputHistoryState = (key: string): InputHistoryState => {
    const existing = inputHistoryBySessionKey.get(key);
    if (existing) return existing;
    const next: InputHistoryState = {
      items: [],
      index: null,
      draftBeforeHistory: "",
    };
    inputHistoryBySessionKey.set(key, next);
    return next;
  };

  const inputHistoryKeyForActiveComposer = (): string => activeComposerKey;

  const exitInputHistoryNavigation = (sessionId: string | null): void => {
    const key = composerKeyForSessionId(sessionId);
    const st = ensureInputHistoryState(key);
    if (st.index === null) return;
    inputEl.value = st.draftBeforeHistory;
    st.index = null;
    st.draftBeforeHistory = "";
  };

  // Chat auto-scroll:
  // - While the user is near the bottom, new content keeps the log pinned to the bottom.
  // - Once the user scrolls up, stop forcing scroll (free mode) until they scroll back near bottom.
  let stickLogToBottom = true;
  const isLogNearBottom = (): boolean => {
    const slackPx = 40;
    return logEl.scrollHeight - logEl.scrollTop - logEl.clientHeight <= slackPx;
  };

  const positionReturnToBottomBtn = (): void => {
    const composerHeight = composerEl.clientHeight;
    const inputRowTop = (inputRowEl as HTMLElement).offsetTop;
    const bottomToInputRowTop = Math.max(0, composerHeight - inputRowTop);
    const gapPx = 6;
    returnToBottomBtn.style.bottom = `${bottomToInputRowTop + gapPx}px`;
  };

  const updateReturnToBottomVisibility = (): void => {
    const show = !isLogNearBottom();
    returnToBottomBtn.style.display = show ? "inline-flex" : "none";
    if (show) positionReturnToBottomBtn();
  };

  const MAX_INPUT_HEIGHT_PX = 200;
  const MIN_INPUT_HEIGHT_PX = 30;
  function autosizeInput(): void {
    inputEl.style.height = "auto";
    const nextHeight = Math.min(MAX_INPUT_HEIGHT_PX, inputEl.scrollHeight);
    inputEl.style.height = `${Math.max(MIN_INPUT_HEIGHT_PX, nextHeight)}px`;
    inputEl.style.overflowY =
      inputEl.scrollHeight > MAX_INPUT_HEIGHT_PX ? "auto" : "hidden";
    updateReturnToBottomVisibility();
  }

  // Restore persisted drafts (text + selection only; images are kept in-memory only).
  const initialDrafts = persistedWebviewState.composerDrafts ?? {};
  for (const [k, v] of Object.entries(initialDrafts)) {
    if (!v || typeof v !== "object") continue;
    const anyV = v as any;
    const text = typeof anyV.text === "string" ? anyV.text : "";
    const selectionStart =
      typeof anyV.selectionStart === "number" ? anyV.selectionStart : 0;
    const selectionEnd =
      typeof anyV.selectionEnd === "number" ? anyV.selectionEnd : 0;
    const st = ensureComposerState(k);
    st.text = text;
    st.selectionStart = selectionStart;
    st.selectionEnd = selectionEnd;
  }

  // Initialize composer state for "no session selected".
  restoreComposerState(null, { updateSuggestions: false });
  logEl.addEventListener("scroll", () => {
    stickLogToBottom = isLogNearBottom();
    updateReturnToBottomVisibility();
  });
  updateReturnToBottomVisibility();

  returnToBottomBtn.addEventListener("click", () => {
    logEl.scrollTop = logEl.scrollHeight;
    stickLogToBottom = true;
    updateReturnToBottomVisibility();
  });

  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden") saveComposerState();
  });
  window.addEventListener("pagehide", () => saveComposerState());

  const getSessionDisplayTitle = (
    sess: Session,
    idx: number,
  ): { label: string; tooltip: string } => {
    const title = String(sess.title || "").trim() || "Untitled";
    if (sess.customTitle) return { label: title, tooltip: title };
    if (Number.isFinite(idx) && idx >= 0) {
      return { label: `${title} #${idx + 1}`, tooltip: title };
    }
    return { label: title, tooltip: title };
  };

  if (typeof markdownit !== "function") {
    throw new Error("markdown-it is not loaded");
  }
  const md = markdownit({ html: false, linkify: true, breaks: true }) as any;
  // Avoid auto-linkifying bare domains / emails (e.g. "README.md" where ".md" is a ccTLD),
  // and rely on our own linkification for URLs and file paths instead.
  if (!md?.linkify?.set) {
    throw new Error("markdown-it linkify is unavailable");
  }
  md.linkify.set({ fuzzyLink: false, fuzzyEmail: false });
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
  let pendingState: ChatViewState | null = null;
  let pendingStateSeq: number | null = null;
  let renderScheduled = false;
  let controlRenderScheduled = false;
  let pendingControlState: ChatViewState | null = null;
  let pendingControlSeq: number | null = null;
  let blocksRenderScheduled = false;
  let pendingBlocksState: ChatViewState | null = null;
  let forceScrollToBottomNextRender = false;
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

  let hoveredAutoLink: HTMLElement | null = null;

  const setAutoLinkHoverState = (
    next: HTMLElement | null,
    modPressed: boolean,
  ): void => {
    if (hoveredAutoLink === next) {
      hoveredAutoLink?.classList.toggle("modHover", modPressed);
      return;
    }
    if (hoveredAutoLink) hoveredAutoLink.classList.remove("modHover");
    hoveredAutoLink = next;
    hoveredAutoLink?.classList.toggle("modHover", modPressed);
  };

  const eventTargetEl = (target: EventTarget | null): HTMLElement | null => {
    if (!target) return null;
    if (target instanceof HTMLElement) return target;
    if (target instanceof Element) return target as unknown as HTMLElement;
    const node = target as Node;
    return node && "parentElement" in node
      ? (node.parentElement as HTMLElement | null)
      : null;
  };

  window.addEventListener("mousemove", (e) => {
    const t = eventTargetEl(e.target);
    const next = t
      ? (t.closest(
          ".autoFileLink[data-open-file],.autoUrlLink[data-open-url]",
        ) as HTMLElement | null)
      : null;
    setAutoLinkHoverState(next, Boolean(e.ctrlKey || e.metaKey));
  });
  window.addEventListener("keydown", (e) => {
    if (!hoveredAutoLink) return;
    hoveredAutoLink.classList.toggle(
      "modHover",
      Boolean(e.ctrlKey || e.metaKey),
    );
  });
  window.addEventListener("keyup", (e) => {
    if (!hoveredAutoLink) return;
    hoveredAutoLink.classList.toggle(
      "modHover",
      Boolean(e.ctrlKey || e.metaKey),
    );
  });
  window.addEventListener("blur", () => setAutoLinkHoverState(null, false));

  let state: ChatViewState = {
    sessions: [],
    activeSession: null,
    unreadSessionIds: [],
    runningSessionIds: [],
    blocks: [],
    hasLatestDiff: false,
    sending: false,
    reloading: false,
    statusText: null,
    modelState: null,
    approvals: [],
    customPrompts: [],
  };

  let toastTimer: number | null = null;
  const showToast = (
    kind: "info" | "success" | "error",
    message: string,
    timeoutMs = 2500,
  ): void => {
    if (toastTimer !== null) {
      window.clearTimeout(toastTimer);
      toastTimer = null;
    }
    toastEl.className = `toast ${kind}`;
    toastEl.textContent = message;
    toastEl.style.display = "";
    toastTimer = window.setTimeout(() => {
      toastEl.style.display = "none";
      toastEl.textContent = "";
      toastEl.className = "toast";
      toastTimer = null;
    }, timeoutMs);
  };

  let domSessionId: string | null = null;
  let rewindTurnIndex: number | null = null;
  const blockElByKey = new Map<string, HTMLElement>();
  let tabsSig: string | null = null;
  let approvalsSig: string | null = null;
  const tabElBySessionId = new Map<string, HTMLDivElement>();
  let isComposing = false;

  const syncTabGroupLabelWidths = (): void => {
    const groups = tabsEl.querySelectorAll<HTMLElement>(".tabGroup");
    for (const groupEl of Array.from(groups)) {
      const labelEl = groupEl.querySelector<HTMLElement>(".tabGroupLabel");
      const groupTabsEl = groupEl.querySelector<HTMLElement>(".tabGroupTabs");
      if (!labelEl || !groupTabsEl) continue;
      const w = Math.ceil(groupTabsEl.getBoundingClientRect().width);
      if (w <= 0) continue;
      const next = `${w}px`;
      if (labelEl.style.maxWidth !== next) labelEl.style.maxWidth = next;
    }
  };

  const tabsResizeObserver = new ResizeObserver(() =>
    syncTabGroupLabelWidths(),
  );
  tabsResizeObserver.observe(tabsEl);

  type SettingsResponseResult =
    | { ok: true; data: unknown }
    | { ok: false; error: string };
  const pendingSettingsRequestsById = new Map<
    string,
    { resolve: (r: SettingsResponseResult) => void }
  >();

  const settingsRequest = async (
    op: string,
    payload: Record<string, unknown>,
  ): Promise<SettingsResponseResult> => {
    const requestId = `${Date.now()}:${Math.random().toString(16).slice(2)}`;
    const p = new Promise<SettingsResponseResult>((resolve) => {
      pendingSettingsRequestsById.set(requestId, { resolve });
    });
    vscode.postMessage({ type: "settingsRequest", op, requestId, ...payload });
    return await p;
  };

  let settingsOpen = false;
  let settingsBusy = false;
  let settingsLastActiveSessionId: string | null = null;
  // Active session backend (tab-scoped).
  let settingsSessionBackendId: "codex" | "codez" | "opencode" | null = null;
  let settingsActiveAccount: string | null = null;
  let settingsSelectedAccount: string | null = null;
  let settingsAuthAccount: { type: string; [key: string]: unknown } | null =
    null;
  let settingsRequiresOpenaiAuth: boolean | null = null;
  let settingsAccounts: Array<{
    name: string;
    kind?: string;
    email?: string;
  }> = [];
  let settingsLoginInFlight: { loginId: string; authUrl: string } | null = null;

  let settingsOpencodeProviders: Array<{
    id: string;
    name?: string;
    connected?: boolean;
    methods?: Array<{ type: "oauth" | "api"; label: string; index: number }>;
  }> = [];
  let settingsOpencodeSelectedProviderId: string | null = null;
  let settingsOpencodeOauthInFlight: {
    providerID: string;
    methodIndex: number;
    url: string;
    method: "auto" | "code";
    instructions: string;
  } | null = null;

  const closeSettings = (): void => {
    settingsOpen = false;
    settingsOverlayEl.style.display = "none";
    settingsOverlayEl.setAttribute("aria-hidden", "true");
    settingsBodyEl.textContent = "";
  };

  const openSettings = async (): Promise<void> => {
    if (settingsOpen) return;
    settingsOpen = true;
    settingsLastActiveSessionId = state.activeSession?.id ?? null;
    settingsOverlayEl.style.display = "flex";
    settingsOverlayEl.setAttribute("aria-hidden", "false");
    settingsBodyEl.textContent = "Loading…";
    await loadSettings();
  };

  const validateAccountName = (name: string): string | null => {
    const trimmed = name.trim();
    if (trimmed.length === 0) return "Account name cannot be empty.";
    if (trimmed.length > 64) return "Account name is too long (max 64 chars).";
    if (!/^[A-Za-z0-9_-]+$/.test(trimmed))
      return "Invalid account name. Use only [A-Za-z0-9_-].";
    return null;
  };

  const applyOpencodeProviderLoad = (data: unknown): void => {
    const d = data as any;
    const providers = d?.providers ?? d ?? null;
    const authMethods = d?.authMethods ?? null;
    const all = Array.isArray(providers?.all) ? providers.all : [];
    const connected = Array.isArray(providers?.connected)
      ? providers.connected
      : [];
    const methodsByProvider =
      authMethods && typeof authMethods === "object"
        ? (authMethods as any)
        : {};

    settingsOpencodeProviders = all
      .map((p: any) => {
        const id = typeof p?.id === "string" ? p.id : "";
        if (!id) return null;
        const name = typeof p?.name === "string" ? p.name : undefined;
        const connectedFlag = connected.includes(id);
        const rawMethods = Array.isArray(methodsByProvider?.[id])
          ? methodsByProvider[id]
          : [];
        const methods = rawMethods
          .map((m: any, index: number) => {
            const type = typeof m?.type === "string" ? String(m.type) : "";
            const label =
              typeof m?.label === "string" ? String(m.label) : type || "method";
            if (type !== "oauth" && type !== "api") return null;
            return { type, label, index } as const;
          })
          .filter(Boolean) as Array<{
          type: "oauth" | "api";
          label: string;
          index: number;
        }>;
        return { id, name, connected: connectedFlag, methods };
      })
      .filter(Boolean) as any;

    if (
      settingsOpencodeSelectedProviderId &&
      !settingsOpencodeProviders.some(
        (p) => p.id === settingsOpencodeSelectedProviderId,
      )
    ) {
      settingsOpencodeSelectedProviderId = null;
    }
  };

  const renderSettings = (): void => {
    settingsBodyEl.textContent = "";

    const sectionSession = document.createElement("div");
    sectionSession.className = "settingsSection";
    const sessionTitle = document.createElement("div");
    sessionTitle.className = "settingsSectionTitle";
    sessionTitle.textContent = "Session";
    sectionSession.appendChild(sessionTitle);

    const sessionRow = document.createElement("div");
    sessionRow.className = "settingsRow";
    const sessionLabel = document.createElement("div");
    if (!state.activeSession || !settingsSessionBackendId) {
      sessionLabel.textContent =
        "No active session. Create or select a session first.";
    } else {
      sessionLabel.textContent = `Backend: ${settingsSessionBackendId}`;
    }
    sessionRow.appendChild(sessionLabel);
    sectionSession.appendChild(sessionRow);

    if (state.activeSession && settingsSessionBackendId) {
      const help = document.createElement("div");
      help.className = "settingsHelp";
      help.textContent =
        settingsSessionBackendId === "opencode"
          ? "opencode の履歴は codex/codez と互換がないため、このセッションを codex/codez へ引き継ぐことはできません。"
          : "codex と codez は履歴形式が互換のため、このスレッドは codex/codez のどちらでも開き直せます（opencode へは引き継げません）。";
      sectionSession.appendChild(help);

      const actions = document.createElement("div");
      actions.className = "settingsRow split";
      const left = document.createElement("div");
      left.className = "settingsBtnGroup";
      const right = document.createElement("div");
      right.className = "settingsBtnGroup";

      if (settingsSessionBackendId === "codex") {
        const btn = document.createElement("button");
        btn.className = "settingsBtn";
        btn.textContent = "Open in codez";
        btn.disabled = settingsBusy;
        btn.addEventListener("click", async () => {
          if (!state.activeSession) return;
          settingsBusy = true;
          renderSettings();
          const res = await settingsRequest("reopenSessionInBackend", {
            sessionId: state.activeSession.id,
            backendId: "codez",
          });
          settingsBusy = false;
          if (!res.ok) showToast("error", res.error);
          renderSettings();
        });
        left.appendChild(btn);
      } else if (settingsSessionBackendId === "codez") {
        const btn = document.createElement("button");
        btn.className = "settingsBtn";
        btn.textContent = "Open in codex";
        btn.disabled = settingsBusy;
        btn.addEventListener("click", async () => {
          if (!state.activeSession) return;
          settingsBusy = true;
          renderSettings();
          const res = await settingsRequest("reopenSessionInBackend", {
            sessionId: state.activeSession.id,
            backendId: "codex",
          });
          settingsBusy = false;
          if (!res.ok) showToast("error", res.error);
          renderSettings();
        });
        left.appendChild(btn);
      }

      const newBtn = document.createElement("button");
      newBtn.className = "settingsBtn primary";
      newBtn.textContent = "New session…";
      newBtn.disabled = settingsBusy;
      newBtn.addEventListener("click", () =>
        vscode.postMessage({ type: "newSession" }),
      );
      right.appendChild(newBtn);

      actions.appendChild(left);
      actions.appendChild(right);
      sectionSession.appendChild(actions);
    }

    const sectionAcct = document.createElement("div");
    sectionAcct.className = "settingsSection";
    const acctTitle = document.createElement("div");
    acctTitle.className = "settingsSectionTitle";
    acctTitle.textContent = "Accounts";
    sectionAcct.appendChild(acctTitle);

    if (!state.activeSession) {
      const msg = document.createElement("div");
      msg.className = "settingsHelp";
      msg.textContent = "No active session. Create or select a session first.";
      sectionAcct.appendChild(msg);
    } else {
      const acctRow = document.createElement("div");
      acctRow.className = "settingsRow";
      const activeText = document.createElement("div");
      const accountsSwitchSupported = settingsSessionBackendId === "codez";
      if (accountsSwitchSupported) {
        activeText.textContent = settingsActiveAccount
          ? `Active: ${settingsActiveAccount}`
          : "Active: (none) (legacy auth)";
      } else if (settingsAuthAccount?.type === "chatgpt") {
        const email = String(settingsAuthAccount["email"] ?? "");
        activeText.textContent = email
          ? `Active: chatgpt (${email})`
          : "Active: chatgpt";
      } else if (settingsAuthAccount?.type === "apiKey") {
        activeText.textContent = "Active: apiKey";
      } else {
        activeText.textContent = "Active: (none) (legacy auth)";
      }
      acctRow.appendChild(activeText);
      sectionAcct.appendChild(acctRow);

      if (settingsRequiresOpenaiAuth) {
        const msg = document.createElement("div");
        msg.className = "settingsHelp";
        msg.textContent = "OpenAI authentication is required. Use Login below.";
        sectionAcct.appendChild(msg);
      }

      if (!accountsSwitchSupported) {
        const msg = document.createElement("div");
        msg.className = "settingsHelp";
        msg.textContent =
          "アカウントの作成/切り替えは codez セッションのみ対応です。";
        sectionAcct.appendChild(msg);
      }

      const list = document.createElement("div");
      list.className = "settingsList";
      if (accountsSwitchSupported) {
        for (const a of settingsAccounts) {
          const row = document.createElement("div");
          row.className =
            "settingsListItem" +
            (settingsSelectedAccount === a.name ? " active" : "");
          row.addEventListener("click", () => {
            settingsSelectedAccount = a.name;
            renderSettings();
          });

          const left = document.createElement("div");
          left.textContent = a.name;
          row.appendChild(left);

          const meta = document.createElement("div");
          meta.className = "settingsListMeta";
          const kind = a.kind ? String(a.kind) : "";
          const email = a.email ? String(a.email) : "";
          meta.textContent =
            kind === "chatgpt"
              ? email
                ? `chatgpt (${email})`
                : "chatgpt"
              : kind === "apiKey"
                ? "apiKey"
                : "";
          row.appendChild(meta);

          list.appendChild(row);
        }
      }
      sectionAcct.appendChild(list);

      const acctBtnRow2 = document.createElement("div");
      acctBtnRow2.className = "settingsRow split";

      const acctBtnLeft = document.createElement("div");
      acctBtnLeft.className = "settingsBtnGroup";
      const acctBtnRight = document.createElement("div");
      acctBtnRight.className = "settingsBtnGroup";

      const refreshBtn = document.createElement("button");
      refreshBtn.className = "settingsBtn";
      refreshBtn.textContent = "Refresh";
      refreshBtn.disabled = settingsBusy;
      refreshBtn.addEventListener("click", async () => {
        if (settingsBusy) return;
        await loadSettings();
      });
      acctBtnLeft.appendChild(refreshBtn);

      if (accountsSwitchSupported) {
        const switchBtn = document.createElement("button");
        switchBtn.className = "settingsBtn primary";
        switchBtn.textContent = "Switch";
        switchBtn.disabled =
          settingsBusy ||
          !settingsSelectedAccount ||
          settingsSelectedAccount === settingsActiveAccount;
        switchBtn.addEventListener("click", async () => {
          if (!settingsSelectedAccount) return;
          settingsBusy = true;
          renderSettings();
          const res = await settingsRequest("accountSwitch", {
            name: settingsSelectedAccount,
            createIfMissing: false,
          });
          settingsBusy = false;
          if (res.ok) {
            const unsupported =
              res.data &&
              typeof res.data === "object" &&
              (res.data as any).unsupported === true;
            if (unsupported) {
              const msg =
                typeof (res.data as any).message === "string"
                  ? String((res.data as any).message)
                  : "アカウントの作成/切り替えは codez 選択時のみ対応です。";
              showToast("info", msg, 4000);
              renderSettings();
              return;
            }

            const migratedLegacy =
              res.data &&
              typeof (res.data as any).migratedLegacy === "boolean" &&
              Boolean((res.data as any).migratedLegacy);
            showToast(
              "success",
              migratedLegacy
                ? `Switched to ${settingsSelectedAccount} (migrated legacy auth).`
                : `Switched to ${settingsSelectedAccount}.`,
            );
            await loadSettings();
          } else {
            showToast("error", res.error);
            renderSettings();
          }
        });
        acctBtnLeft.appendChild(switchBtn);
      }

      const logoutBtn = document.createElement("button");
      logoutBtn.className = "settingsBtn";
      logoutBtn.textContent = "Logout (active)";
      logoutBtn.disabled = settingsBusy;
      logoutBtn.addEventListener("click", async () => {
        settingsBusy = true;
        renderSettings();
        const res = await settingsRequest("accountLogout", {});
        settingsBusy = false;
        if (res.ok) {
          showToast("success", "Logged out (active account).");
          await loadSettings();
        } else {
          showToast("error", res.error);
          renderSettings();
        }
      });
      acctBtnRight.appendChild(logoutBtn);

      acctBtnRow2.appendChild(acctBtnLeft);
      acctBtnRow2.appendChild(acctBtnRight);
      sectionAcct.appendChild(acctBtnRow2);

      if (accountsSwitchSupported) {
        const createRow = document.createElement("div");
        createRow.className = "settingsRow";
        const createInput = document.createElement("input");
        createInput.className = "settingsInput grow";
        createInput.placeholder = "new-account-name";
        createInput.addEventListener("input", () => {
          const v = createInput.value;
          const err = validateAccountName(v);
          createInput.title = err ?? "";
        });
        const createBtn = document.createElement("button");
        createBtn.className = "settingsBtn primary";
        createBtn.textContent = "Create & Switch";
        createBtn.disabled = settingsBusy;
        createBtn.addEventListener("click", async () => {
          const name = createInput.value.trim();
          const err = validateAccountName(name);
          if (err) {
            showToast("error", err);
            return;
          }
          settingsBusy = true;
          renderSettings();
          const res = await settingsRequest("accountSwitch", {
            name,
            createIfMissing: true,
          });
          settingsBusy = false;
          if (res.ok) {
            const unsupported =
              res.data &&
              typeof res.data === "object" &&
              (res.data as any).unsupported === true;
            if (unsupported) {
              const msg =
                typeof (res.data as any).message === "string"
                  ? String((res.data as any).message)
                  : "アカウントの作成/切り替えは codez 選択時のみ対応です。";
              showToast("info", msg, 4000);
              renderSettings();
              return;
            }

            const migratedLegacy =
              res.data &&
              typeof (res.data as any).migratedLegacy === "boolean" &&
              Boolean((res.data as any).migratedLegacy);
            showToast(
              "success",
              migratedLegacy
                ? `Created and switched to ${name} (migrated legacy auth).`
                : `Created and switched to ${name}.`,
            );
            await loadSettings();
          } else {
            showToast("error", res.error);
            renderSettings();
          }
        });
        createRow.appendChild(createInput);
        createRow.appendChild(createBtn);
        sectionAcct.appendChild(createRow);
      }

      const help = document.createElement("div");
      help.className = "settingsHelp";
      help.textContent =
        "Account names: [A-Za-z0-9_-], 1..64 chars\nLogout logs out the active account only.";
      sectionAcct.appendChild(help);

      const sectionLogin = document.createElement("div");
      sectionLogin.className = "settingsSubsection";
      const loginTitle = document.createElement("div");
      loginTitle.className = "settingsSubsectionTitle";
      loginTitle.textContent = "Login";
      sectionLogin.appendChild(loginTitle);

      const loginRow = document.createElement("div");
      loginRow.className = "settingsRow";
      const loginInfo = document.createElement("div");
      loginInfo.textContent = settingsActiveAccount
        ? `Active account: ${settingsActiveAccount}`
        : "Active account: (none) (legacy auth)";
      loginRow.appendChild(loginInfo);
      sectionLogin.appendChild(loginRow);

      const chatgptBtn = document.createElement("button");
      chatgptBtn.className = "settingsBtn primary";
      chatgptBtn.textContent = "Login with ChatGPT";
      chatgptBtn.disabled = settingsBusy;
      chatgptBtn.addEventListener("click", async () => {
        settingsBusy = true;
        renderSettings();
        const res = await settingsRequest("accountLoginChatgptStart", {});
        settingsBusy = false;
        if (!res.ok) {
          showToast("error", res.error);
          renderSettings();
          return;
        }
        const authUrl =
          res.data && typeof (res.data as any).authUrl === "string"
            ? String((res.data as any).authUrl)
            : "";
        const loginId =
          res.data && typeof (res.data as any).loginId === "string"
            ? String((res.data as any).loginId)
            : "";
        if (!authUrl || !loginId) {
          showToast("error", "Login start returned invalid data.");
          renderSettings();
          return;
        }
        settingsLoginInFlight = { loginId, authUrl };
        vscode.postMessage({ type: "openExternal", url: authUrl });
        showToast("info", "Opened browser for ChatGPT login.");
        renderSettings();
      });
      const chatgptRow = document.createElement("div");
      chatgptRow.className = "settingsRow";
      chatgptRow.appendChild(chatgptBtn);
      sectionLogin.appendChild(chatgptRow);

      const apiKeyInput = document.createElement("input");
      apiKeyInput.className = "settingsInput grow";
      apiKeyInput.placeholder = "API key";
      apiKeyInput.type = "password";
      apiKeyInput.disabled = settingsBusy;

      const apiKeyBtn = document.createElement("button");
      apiKeyBtn.className = "settingsBtn";
      apiKeyBtn.textContent = "Login with API key";
      apiKeyBtn.disabled = settingsBusy;
      apiKeyBtn.addEventListener("click", async () => {
        const k = apiKeyInput.value.trim();
        if (!k) {
          showToast("error", "API key cannot be empty.");
          return;
        }
        settingsBusy = true;
        renderSettings();
        const res = await settingsRequest("accountLoginApiKey", { apiKey: k });
        settingsBusy = false;
        apiKeyInput.value = "";
        if (!res.ok) {
          showToast("error", res.error);
          renderSettings();
          return;
        }
        showToast("success", "Logged in with API key.");
        await loadSettings();
      });
      const apiKeyRow = document.createElement("div");
      apiKeyRow.className = "settingsRow";
      apiKeyRow.appendChild(apiKeyInput);
      apiKeyRow.appendChild(apiKeyBtn);
      sectionLogin.appendChild(apiKeyRow);

      if (settingsLoginInFlight) {
        const inflight = document.createElement("div");
        inflight.className = "settingsHelp";
        inflight.textContent = `Login in progress…\nloginId=${settingsLoginInFlight.loginId}`;
        sectionLogin.appendChild(inflight);
      }

      sectionAcct.appendChild(sectionLogin);
    }

    settingsBodyEl.appendChild(sectionSession);
    if (settingsSessionBackendId !== "opencode") {
      settingsBodyEl.appendChild(sectionAcct);
    }

    if (settingsSessionBackendId === "opencode") {
      const sectionProv = document.createElement("div");
      sectionProv.className = "settingsSection";
      const title = document.createElement("div");
      title.className = "settingsSectionTitle";
      title.textContent = "Providers (opencode)";
      sectionProv.appendChild(title);

      const help = document.createElement("div");
      help.className = "settingsHelp";
      help.textContent =
        "Provider registration is handled by the opencode server. Configure OAuth/API keys here, then select provider/model in the model picker.";
      sectionProv.appendChild(help);

      const controlsRow = document.createElement("div");
      controlsRow.className = "settingsRow split";
      const controlsLeft = document.createElement("div");
      controlsLeft.className = "settingsBtnGroup";
      const controlsRight = document.createElement("div");
      controlsRight.className = "settingsBtnGroup";

      const select = document.createElement("select");
      select.className = "settingsSelect grow";
      select.disabled = settingsBusy;
      const opt0 = document.createElement("option");
      opt0.value = "";
      opt0.textContent = "Select a provider…";
      select.appendChild(opt0);
      for (const p of settingsOpencodeProviders) {
        const o = document.createElement("option");
        o.value = p.id;
        const name = p.name ? String(p.name) : p.id;
        const suffix = p.connected ? " (connected)" : "";
        o.textContent = `${name}${suffix}`;
        if (settingsOpencodeSelectedProviderId === p.id) o.selected = true;
        select.appendChild(o);
      }
      select.addEventListener("change", () => {
        const v = select.value.trim();
        settingsOpencodeSelectedProviderId = v || null;
        settingsOpencodeOauthInFlight = null;
        renderSettings();
      });
      controlsLeft.appendChild(select);

      const refreshBtn = document.createElement("button");
      refreshBtn.className = "settingsBtn";
      refreshBtn.textContent = "Reload providers";
      refreshBtn.disabled = settingsBusy;
      refreshBtn.addEventListener("click", async () => {
        settingsBusy = true;
        renderSettings();
        const res = await settingsRequest("opencodeProviderLoad", {});
        settingsBusy = false;
        if (!res.ok) {
          showToast("error", res.error);
          renderSettings();
          return;
        }
        const unsupported =
          res.data &&
          typeof res.data === "object" &&
          (res.data as any).unsupported === true;
        if (unsupported) {
          const msg =
            typeof (res.data as any).message === "string"
              ? String((res.data as any).message)
              : "opencode backend is not active.";
          showToast("info", msg, 4000);
          renderSettings();
          return;
        }
        applyOpencodeProviderLoad(res.data);
        showToast("success", "Loaded providers.");
        renderSettings();
      });
      controlsRight.appendChild(refreshBtn);

      controlsRow.appendChild(controlsLeft);
      controlsRow.appendChild(controlsRight);
      sectionProv.appendChild(controlsRow);

      const selected = settingsOpencodeProviders.find(
        (p) => p.id === settingsOpencodeSelectedProviderId,
      );
      if (selected) {
        const methods = Array.isArray(selected.methods) ? selected.methods : [];
        for (const m of methods) {
          const row = document.createElement("div");
          row.className = "settingsRow";
          row.style.gap = "8px";
          row.style.flexWrap = "wrap";

          const label = document.createElement("div");
          label.textContent = `${m.label} (${m.type})`;
          row.appendChild(label);

          if (m.type === "oauth") {
            const startBtn = document.createElement("button");
            startBtn.className = "settingsBtn primary";
            startBtn.textContent = "Start OAuth";
            startBtn.disabled = settingsBusy;
            startBtn.addEventListener("click", async () => {
              settingsBusy = true;
              renderSettings();
              const res = await settingsRequest(
                "opencodeProviderOauthAuthorize",
                {
                  providerID: selected.id,
                  method: m.index,
                },
              );
              settingsBusy = false;
              if (!res.ok) {
                showToast("error", res.error);
                renderSettings();
                return;
              }
              const auth = res.data as any;
              const url =
                auth && typeof auth.url === "string" ? String(auth.url) : "";
              const mode =
                auth && typeof auth.method === "string"
                  ? String(auth.method)
                  : "";
              const instructions =
                auth && typeof auth.instructions === "string"
                  ? String(auth.instructions)
                  : "";
              if (!url || (mode !== "auto" && mode !== "code")) {
                showToast("error", "OAuth authorize returned invalid data.");
                renderSettings();
                return;
              }
              settingsOpencodeOauthInFlight = {
                providerID: selected.id,
                methodIndex: m.index,
                url,
                method: mode as "auto" | "code",
                instructions,
              };
              vscode.postMessage({ type: "openExternal", url });
              showToast("info", "Opened browser for OAuth.");
              renderSettings();
            });
            row.appendChild(startBtn);
          }

          if (m.type === "api") {
            const input = document.createElement("input");
            input.className = "settingsInput grow";
            input.type = "password";
            input.placeholder = "API key";
            input.disabled = settingsBusy;
            row.appendChild(input);

            const saveBtn = document.createElement("button");
            saveBtn.className = "settingsBtn";
            saveBtn.textContent = "Save API key";
            saveBtn.disabled = settingsBusy;
            saveBtn.addEventListener("click", async () => {
              const k = input.value.trim();
              if (!k) {
                showToast("error", "API key cannot be empty.");
                return;
              }
              settingsBusy = true;
              renderSettings();
              const res = await settingsRequest("opencodeProviderSetApiKey", {
                providerID: selected.id,
                apiKey: k,
              });
              settingsBusy = false;
              input.value = "";
              if (!res.ok) {
                showToast("error", res.error);
                renderSettings();
                return;
              }
              showToast("success", "Saved API key.");
              const reload = await settingsRequest("opencodeProviderLoad", {});
              if (reload.ok) applyOpencodeProviderLoad(reload.data);
              renderSettings();
            });
            row.appendChild(saveBtn);
          }

          sectionProv.appendChild(row);
        }

        if (
          settingsOpencodeOauthInFlight &&
          settingsOpencodeOauthInFlight.providerID === selected.id
        ) {
          const inflight = settingsOpencodeOauthInFlight;
          const inst = document.createElement("div");
          inst.className = "settingsHelp";
          inst.textContent = inflight.instructions || "OAuth in progress…";
          sectionProv.appendChild(inst);

          const completeRow = document.createElement("div");
          completeRow.className = "settingsRow";
          completeRow.style.gap = "8px";

          if (inflight.method === "code") {
            const codeInput = document.createElement("input");
            codeInput.className = "settingsInput grow";
            codeInput.placeholder = "Authorization code";
            codeInput.disabled = settingsBusy;
            completeRow.appendChild(codeInput);

            const completeBtn = document.createElement("button");
            completeBtn.className = "settingsBtn primary";
            completeBtn.textContent = "Complete OAuth";
            completeBtn.disabled = settingsBusy;
            completeBtn.addEventListener("click", async () => {
              const code = codeInput.value.trim();
              if (!code) {
                showToast("error", "Authorization code cannot be empty.");
                return;
              }
              settingsBusy = true;
              renderSettings();
              const res = await settingsRequest(
                "opencodeProviderOauthCallback",
                {
                  providerID: inflight.providerID,
                  method: inflight.methodIndex,
                  code,
                },
              );
              settingsBusy = false;
              if (!res.ok) {
                showToast("error", res.error);
                renderSettings();
                return;
              }
              settingsOpencodeOauthInFlight = null;
              showToast("success", "OAuth completed.");
              const reload = await settingsRequest("opencodeProviderLoad", {});
              if (reload.ok) applyOpencodeProviderLoad(reload.data);
              renderSettings();
            });
            completeRow.appendChild(completeBtn);
          } else {
            const completeBtn = document.createElement("button");
            completeBtn.className = "settingsBtn primary";
            completeBtn.textContent = "Complete OAuth";
            completeBtn.disabled = settingsBusy;
            completeBtn.addEventListener("click", async () => {
              settingsBusy = true;
              renderSettings();
              const res = await settingsRequest(
                "opencodeProviderOauthCallback",
                {
                  providerID: inflight.providerID,
                  method: inflight.methodIndex,
                },
              );
              settingsBusy = false;
              if (!res.ok) {
                showToast("error", res.error);
                renderSettings();
                return;
              }
              settingsOpencodeOauthInFlight = null;
              showToast("success", "OAuth completed.");
              const reload = await settingsRequest("opencodeProviderLoad", {});
              if (reload.ok) applyOpencodeProviderLoad(reload.data);
              renderSettings();
            });
            completeRow.appendChild(completeBtn);
          }
          sectionProv.appendChild(completeRow);
        }
      }

      settingsBodyEl.appendChild(sectionProv);
    }
  };

  const loadSettings = async (): Promise<void> => {
    settingsBusy = true;
    renderSettings();
    const res = await settingsRequest("load", {});
    settingsBusy = false;
    if (!res.ok) {
      settingsBodyEl.textContent = "";
      const err = document.createElement("div");
      err.className = "askError";
      err.textContent = res.error;
      settingsBodyEl.appendChild(err);
      return;
    }
    const data = res.data as any;
    settingsSessionBackendId =
      data?.sessionBackendId === "codex" ||
      data?.sessionBackendId === "codez" ||
      data?.sessionBackendId === "opencode"
        ? data.sessionBackendId
        : (state.activeSession?.backendId ?? null);

    const accounts = data?.accounts ?? null;
    const activeAccount =
      typeof accounts?.activeAccount === "string"
        ? accounts.activeAccount
        : null;
    const list = Array.isArray(accounts?.accounts) ? accounts.accounts : [];

    const account = data?.account ?? null;
    const rawAccountObj = account ? (account as any).account : null;
    const accountObj =
      rawAccountObj && typeof rawAccountObj === "object" ? rawAccountObj : null;
    settingsAuthAccount =
      accountObj && typeof accountObj.type === "string" ? accountObj : null;
    settingsRequiresOpenaiAuth =
      account && typeof (account as any).requiresOpenaiAuth === "boolean"
        ? Boolean((account as any).requiresOpenaiAuth)
        : null;

    settingsActiveAccount = activeAccount;
    settingsAccounts = list
      .filter((x: any) => x && typeof x.name === "string")
      .map((x: any) => ({
        name: String(x.name),
        kind: typeof x.kind === "string" ? x.kind : undefined,
        email: typeof x.email === "string" ? x.email : undefined,
      }));
    settingsSelectedAccount =
      settingsSelectedAccount &&
      settingsAccounts.some((a) => a.name === settingsSelectedAccount)
        ? settingsSelectedAccount
        : (settingsActiveAccount ??
          (settingsAccounts.length > 0 ? settingsAccounts[0]!.name : null));

    if (settingsSessionBackendId === "opencode") {
      const opencode = data?.opencode ?? null;
      if (opencode) applyOpencodeProviderLoad(opencode);
    }
    settingsLastActiveSessionId = state.activeSession?.id ?? null;
    renderSettings();
  };

  type ImageLoadResult =
    | { ok: true; imageKey: string; mimeType: string; base64: string }
    | { ok: false; imageKey: string; error: string };
  const pendingImageRequestsById = new Map<
    string,
    { imageKey: string; resolve: (r: ImageLoadResult) => void }
  >();
  const imageObjectUrlByKey = new Map<
    string,
    { url: string; byteLength: number; lastUsedAt: number }
  >();
  const IMAGE_OBJECT_URL_CACHE_MAX_ITEMS = 24;
  const IMAGE_OBJECT_URL_CACHE_MAX_TOTAL_BYTES = 12_000_000;
  const IMAGE_RENDER_MAX_EDGE_PX = 1024;
  const IMAGE_RENDER_MAX_BYTES = 350_000;

  function pruneImageObjectUrls(): void {
    const entries = Array.from(imageObjectUrlByKey.entries());
    let total = entries.reduce((sum, [, v]) => sum + (v.byteLength || 0), 0);
    if (
      entries.length <= IMAGE_OBJECT_URL_CACHE_MAX_ITEMS &&
      total <= IMAGE_OBJECT_URL_CACHE_MAX_TOTAL_BYTES
    )
      return;
    entries.sort((a, b) => (a[1].lastUsedAt || 0) - (b[1].lastUsedAt || 0));
    for (const [key, v] of entries) {
      if (
        imageObjectUrlByKey.size <= IMAGE_OBJECT_URL_CACHE_MAX_ITEMS &&
        total <= IMAGE_OBJECT_URL_CACHE_MAX_TOTAL_BYTES
      )
        break;
      URL.revokeObjectURL(v.url);
      imageObjectUrlByKey.delete(key);
      total -= v.byteLength || 0;
    }
  }

  async function decodeBase64ToBytes(base64: string): Promise<Uint8Array> {
    try {
      const binary = atob(base64);
      const out = new Uint8Array(binary.length);
      for (let i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
      return out;
    } catch (err) {
      throw new Error(`Failed to decode base64: ${String(err)}`);
    }
  }

  async function resizeImageBlob(
    blob: Blob,
  ): Promise<{ blob: Blob; byteLength: number }> {
    const bitmap = await createImageBitmap(blob);
    try {
      const w = bitmap.width;
      const h = bitmap.height;
      if (!w || !h) return { blob, byteLength: blob.size };
      const scale = Math.min(1, IMAGE_RENDER_MAX_EDGE_PX / Math.max(w, h));
      const tw = Math.max(1, Math.round(w * scale));
      const th = Math.max(1, Math.round(h * scale));

      const canvas = document.createElement("canvas");
      canvas.width = tw;
      canvas.height = th;
      const ctx = canvas.getContext("2d");
      if (!ctx) return { blob, byteLength: blob.size };
      ctx.drawImage(bitmap, 0, 0, tw, th);

      const toBlob = (quality: number): Promise<Blob | null> =>
        new Promise((resolve) =>
          canvas.toBlob((b) => resolve(b), "image/jpeg", quality),
        );
      const candidates = [0.85, 0.75, 0.65, 0.55];
      let best: Blob | null = null;
      for (const q of candidates) {
        const b = await toBlob(q);
        if (!b) continue;
        best = b;
        if (b.size <= IMAGE_RENDER_MAX_BYTES) break;
      }
      if (!best) return { blob, byteLength: blob.size };
      return { blob: best, byteLength: best.size };
    } finally {
      bitmap.close();
    }
  }

  function requestImageData(imageKey: string): Promise<ImageLoadResult> {
    const requestId = `${Date.now()}:${Math.random().toString(16).slice(2)}`;
    return new Promise((resolve) => {
      pendingImageRequestsById.set(requestId, { imageKey, resolve });
      vscode.postMessage({ type: "loadImage", imageKey, requestId });
    });
  }

  type OffloadableImageRef = {
    imageKey?: string;
    src: string;
    mimeType?: string;
    autoLoad?: boolean;
    caption: string | null;
  };

  async function ensureImageRendered(
    imageRef: OffloadableImageRef,
    imgEl: HTMLImageElement,
    captionEl: HTMLDivElement,
  ): Promise<void> {
    const imageKey =
      typeof imageRef.imageKey === "string" ? imageRef.imageKey : "";
    if (!imageKey) {
      if (imageRef.src && imgEl.src !== imageRef.src) imgEl.src = imageRef.src;
      return;
    }

    const cached = imageObjectUrlByKey.get(imageKey);
    if (cached) {
      cached.lastUsedAt = Date.now();
      if (imgEl.src !== cached.url) imgEl.src = cached.url;
      return;
    }

    if (!imageRef.autoLoad) {
      imgEl.removeAttribute("src");
      imgEl.style.cursor = "pointer";
      const caption = (imageRef.caption || "").trim();
      captionEl.textContent =
        caption || "画像はオフロードされています（クリックで読み込み）";
      captionEl.style.display = "";
      imgEl.addEventListener(
        "click",
        () => {
          (imageRef as any).autoLoad = true;
          void ensureImageRendered(imageRef, imgEl, captionEl);
        },
        { once: true },
      );
      return;
    }

    captionEl.textContent = "画像を読み込み中…";
    captionEl.style.display = "";

    const res = await requestImageData(imageKey);
    if (!res.ok) {
      captionEl.textContent = `画像の読み込みに失敗: ${res.error}`;
      captionEl.style.display = "";
      return;
    }

    const bytes = await decodeBase64ToBytes(res.base64);
    const mimeType =
      res.mimeType || (imageRef.mimeType ? String(imageRef.mimeType) : "");
    const copy = new Uint8Array(bytes.byteLength);
    copy.set(bytes);
    const rawBlob = new Blob([copy.buffer as ArrayBuffer], {
      type: mimeType || "application/octet-stream",
    });
    const resized = await resizeImageBlob(rawBlob);
    const url = URL.createObjectURL(resized.blob);
    imageObjectUrlByKey.set(imageKey, {
      url,
      byteLength: resized.byteLength,
      lastUsedAt: Date.now(),
    });
    pruneImageObjectUrls();

    imgEl.style.cursor = "";
    if (imgEl.src !== url) imgEl.src = url;
    const caption = (imageRef.caption || "").trim();
    captionEl.textContent = caption;
    captionEl.style.display = caption ? "" : "none";
  }

  window.addEventListener("unload", () => {
    for (const v of imageObjectUrlByKey.values()) URL.revokeObjectURL(v.url);
    imageObjectUrlByKey.clear();
  });

  let detailsState = persistedWebviewState.detailsState ?? {};

  function saveDetailsState(key: string, open: boolean): void {
    detailsState[key] = open;
    updatePersistedWebviewState({ detailsState });
  }

  const baseSlashSuggestions: SuggestItem[] = [
    {
      insert: "/compact ",
      label: "/compact",
      detail: "Compact context",
      kind: "slash",
    },
  ];
  const uiSlashSuggestions: SuggestItem[] = [
    {
      insert: "/new ",
      label: "/new",
      detail: "New session",
      kind: "slash",
    },
    {
      insert: "/init ",
      label: "/init",
      detail: "Create AGENTS.md",
      kind: "slash",
    },
    {
      insert: "/resume ",
      label: "/resume",
      detail: "Resume from history",
      kind: "slash",
    },
    {
      insert: "/status ",
      label: "/status",
      detail: "Show status",
      kind: "slash",
    },
    {
      insert: "/mcp ",
      label: "/mcp",
      detail: "List MCP servers",
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
    {
      insert: "/skills ",
      label: "/skills",
      detail: "Browse skills",
      kind: "slash",
    },
    {
      insert: "/agents ",
      label: "/agents",
      detail: "Browse agents (codez)",
      kind: "slash",
    },
    { insert: "/help ", label: "/help", detail: "Show help", kind: "slash" },
  ];

  function buildSlashSuggestions(): SuggestItem[] {
    const agents = state.capabilities?.agents ?? false;
    const base =
      state.activeSession?.backendId === "codez"
        ? baseSlashSuggestions
        : ([] as const);
    const ui = agents
      ? uiSlashSuggestions
      : uiSlashSuggestions.filter((s) => s.label !== "/agents");
    const reserved = new Set(
      [...base, ...ui].map((s) => s.label.replace(/^\//, "")),
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
    return [...base, ...custom, ...ui];
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
  let fileSearch: null | { sessionId: string; query: string; paths: string[] } =
    null;
  let fileSearchInFlight: null | { sessionId: string; query: string } = null;
  let fileSearchTimer: number | null = null;
  const FILE_SEARCH_DEBOUNCE_MS = 250;
  const FILE_SEARCH_MIN_QUERY_LEN = 2;
  const MAX_LINKIFY_TEXT_CHARS = 10_000;
  let agentIndex: string[] | null = null;
  let agentIndexForSessionId: string | null = null;
  let agentIndexRequestedForSessionId: string | null = null;
  let skillIndex: Array<{
    name: string;
    description: string | null;
    scope: string;
    path: string;
  }> | null = null;
  let skillIndexForSessionId: string | null = null;
  let skillIndexRequestedForSessionId: string | null = null;
  let activeReplace: null | {
    from: number;
    to: number;
    inserted: string;
  } = null;
  const pendingBlocksBySessionId = new Map<string, ChatBlock[]>();
  const blocksBySessionId = new Map<string, ChatBlock[]>();
  const BLOCK_CACHE_MAX_SESSIONS = 3;
  const blockCacheTouchOrder = new Map<string, number>();
  let blockCacheTouchClock = 0;

  function touchBlockCache(sessionId: string): void {
    if (!sessionId) return;
    blockCacheTouchClock += 1;
    blockCacheTouchOrder.set(sessionId, blockCacheTouchClock);
    pruneBlockCaches();
  }

  function pruneBlockCaches(): void {
    const activeId = state.activeSession?.id ?? null;
    const ids = new Set<string>();
    for (const k of pendingBlocksBySessionId.keys()) ids.add(k);
    for (const k of blocksBySessionId.keys()) ids.add(k);
    for (const k of blockCacheTouchOrder.keys()) ids.add(k);
    if (ids.size <= BLOCK_CACHE_MAX_SESSIONS) return;

    const entries = [...ids].map((id) => ({
      id,
      t: blockCacheTouchOrder.get(id) ?? 0,
      active: id === activeId,
    }));
    entries.sort((a, b) => {
      if (a.active !== b.active) return a.active ? 1 : -1;
      return a.t - b.t;
    });

    for (let i = 0; i < entries.length; i++) {
      const e = entries[i]!;
      const remaining = entries.length - i;
      if (remaining <= BLOCK_CACHE_MAX_SESSIONS) break;
      if (e.active) continue;
      pendingBlocksBySessionId.delete(e.id);
      blocksBySessionId.delete(e.id);
      blockCacheTouchOrder.delete(e.id);
    }
  }

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

  function sessionBlockIdForKey(key: string): string | null {
    if (key.startsWith("b:")) return key.slice(2) || null;
    const prefixes = [
      "command:",
      "fileChange:",
      "reasoning:",
      "mcp:",
      "webSearch:",
      "image:",
      "plan:",
      "error:",
      "sys:",
    ];
    for (const p of prefixes) {
      if (!key.startsWith(p)) continue;
      const rest = key.slice(p.length);
      const id = rest.split(":")[0] || "";
      return id || null;
    }
    return null;
  }

  function pruneStaleSessionBlockEls(blockIds: Set<string>): void {
    for (const [k, el] of blockElByKey.entries()) {
      if (k.startsWith("global:")) continue;
      const id = sessionBlockIdForKey(k);
      if (!id) continue;
      if (blockIds.has(id)) continue;
      if (el.parentElement) el.parentElement.removeChild(el);
      blockElByKey.delete(k);
      delete detailsState[k];
    }
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
    // Keep notices at the top of the log (they're not part of the conversation stream).
    if (/\bnotice\b/.test(className)) {
      logEl.insertBefore(det, logEl.firstChild);
    } else {
      logEl.appendChild(det);
    }
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

  function ensureNestedDetailsWithStatusIcon(
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
      if (sum) {
        const txt = sum.querySelector(
          ':scope > span[data-k="summaryText"]',
        ) as HTMLSpanElement | null;
        if (txt) txt.textContent = summaryText;
        else sum.textContent = summaryText;
        const icon = sum.querySelector(
          ':scope > span[data-k="statusIcon"]',
        ) as HTMLSpanElement | null;
        if (!icon) {
          const sp = document.createElement("span");
          sp.dataset.k = "statusIcon";
          sp.className = "statusIcon";
          sp.style.display = "none";
          sum.appendChild(sp);
        }
      }
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
    delete (el.dataset as any).fileLinks;
    linkifyFilePaths(el);
  }

  function linkifyFilePaths(root: HTMLElement): void {
    // Avoid double-processing the same subtree.
    if (root.dataset.fileLinks === "1") return;
    root.dataset.fileLinks = "1";

    // NOTE: For plain text nodes, keep detection conservative to avoid accidental
    // linkification (e.g. emails). This code runs over all text nodes and will
    // wrap matches in an element (but only opens on Ctrl/Cmd+Click).
    // For <code> tokens, we allow basename-style paths like "README.md:10" and
    // ".env.local:23" because they are explicitly formatted by the author.
    // Allow Unicode letters/numbers in path segments so e.g. "docs/日本語/仕様.md:10"
    // is recognized. Keep other constraints (requires "." extension; plain text
    // requires at least one "/") to avoid over-linkifying.
    //
    // Policy:
    // - Outside code blocks: allow full-width space (　) but do NOT allow ASCII
    //   spaces inside path segments. This keeps linkification conservative.
    // - Inside code blocks (<pre><code>): allow ASCII spaces too, so paths from
    //   tool output like "確認事項 ver1.1_記入済み.docx" can be linkified.
    //
    // We also allow "・" (Japanese middle dot), which commonly appears in names.
    const pathSegmentNoAsciiSpace = String.raw`[\p{L}\p{N}\p{M}_@.+\-・　]+`;
    const pathSegmentNoAsciiSpaceNoAt = String.raw`[\p{L}\p{N}\p{M}_.+\-・　]+`;
    const pathSegmentWithAsciiSpace = String.raw`[\p{L}\p{N}\p{M}_@.+\-・　 ]+`;
    const fileTokenRe = new RegExp(
      String.raw`(?:\.?\/)?${pathSegmentNoAsciiSpace}(?:\/${pathSegmentNoAsciiSpace})+\.[A-Za-z0-9]{1,8}(?:(?::\d+(?::\d+)?)|(?:#L\d+(?:C\d+)?))?`,
      "gu",
    );
    const fileTokenWithAtRe = new RegExp(
      String.raw`@?(?:\.?\/)?${pathSegmentNoAsciiSpace}(?:\/${pathSegmentNoAsciiSpace})+\.[A-Za-z0-9]{1,8}(?:(?::\d+(?::\d+)?)|(?:#L\d+(?:C\d+)?))?`,
      "gu",
    );
    const textFileTokenWithAtRe = new RegExp(
      // Allow basename-style tokens in plain text, but disallow "@" inside segments
      // to reduce false positives like emails.
      String.raw`@?(?:\.?\/)?${pathSegmentNoAsciiSpaceNoAt}(?:\/${pathSegmentNoAsciiSpaceNoAt})*\.[A-Za-z0-9]{1,8}(?:(?::\d+(?::\d+)?)|(?:#L\d+(?:C\d+)?))?`,
      "gu",
    );
    const codeBlockFileTokenWithAtRe = new RegExp(
      String.raw`@?(?:\.?\/)?${pathSegmentWithAsciiSpace}(?:\/${pathSegmentWithAsciiSpace})*\.[A-Za-z0-9]{1,8}(?:(?::\d+(?::\d+)?)|(?:#L\d+(?:C\d+)?))?`,
      "gu",
    );

    const codeFileTokenRe = new RegExp(
      String.raw`^(?:\.?\/)?${pathSegmentNoAsciiSpace}(?:\/${pathSegmentNoAsciiSpace})*\.[A-Za-z0-9]{1,8}(?:(?::\d+(?::\d+)?)|(?:#L\d+(?:C\d+)?))?$`,
      "u",
    );

    const urlRe = /https?:\/\/[^\s<>()]+/gi;

    const normalizeToken = (raw: string): string => {
      let t = raw.trim();
      if (t.startsWith("@")) t = t.slice(1);
      while (t.length > 0 && /[),.;:!?]/.test(t[t.length - 1] || "")) {
        t = t.slice(0, -1);
      }
      return t;
    };

    for (const code of Array.from(root.querySelectorAll("code"))) {
      const el = code as HTMLElement;
      if (el.dataset.openFile) continue;
      const raw = (el.textContent || "").trim();
      // Keep inline <code> conservative: treat ASCII whitespace as a delimiter,
      // but allow full-width space (　) in file names.
      if (!raw || /[ \t\r\n]/.test(raw)) continue;
      if (/^https?:\/\//i.test(raw)) {
        const normalizedUrl = normalizeToken(raw);
        if (!normalizedUrl) continue;
        el.dataset.openUrl = normalizedUrl;
        el.title = "Ctrl/Cmd+Click to open";
        el.classList.add("autoUrlLink");
        continue;
      }
      const rawForMatch = raw.startsWith("@") ? raw.slice(1) : raw;
      if (!codeFileTokenRe.test(rawForMatch)) continue;
      const normalized = normalizeToken(rawForMatch);
      if (!normalized) continue;
      el.dataset.openFile = normalized;
      el.title = "Ctrl/Cmd+Click to open";
      el.classList.add("autoFileLink");
    }

    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    const textNodes: Text[] = [];
    for (;;) {
      const n = walker.nextNode();
      if (!n) break;
      const parent = (n as Text).parentElement;
      if (!parent) continue;
      if (parent.closest("a,[data-open-file],[data-open-url]")) continue;
      textNodes.push(n as Text);
    }

    for (const node of textNodes) {
      const rawText = node.nodeValue || "";
      const parentEl = node.parentElement;
      const inCodeBlock = parentEl ? Boolean(parentEl.closest("pre")) : false;

      const appendTextWithFileLinks = (
        frag: DocumentFragment,
        text: string,
      ): boolean => {
        const fileRe = inCodeBlock
          ? codeBlockFileTokenWithAtRe
          : textFileTokenWithAtRe;
        fileRe.lastIndex = 0;
        let m: RegExpExecArray | null = null;
        let lastIdx = 0;
        let changed = false;

        while ((m = fileRe.exec(text))) {
          changed = true;
          const start = m.index;
          const end = start + m[0].length;
          const before = text.slice(lastIdx, start);
          if (before) frag.appendChild(document.createTextNode(before));

          const normalized = normalizeToken(m[0]);
          if (normalized) {
            const sp = document.createElement("span");
            sp.className = "autoFileLink";
            sp.dataset.openFile = normalized;
            sp.title = "Ctrl/Cmd+Click to open";
            sp.textContent = m[0];
            frag.appendChild(sp);
          } else {
            frag.appendChild(document.createTextNode(m[0]));
          }

          lastIdx = end;
        }

        if (!changed) {
          if (text) frag.appendChild(document.createTextNode(text));
          return false;
        }

        const tail = text.slice(lastIdx);
        if (tail) frag.appendChild(document.createTextNode(tail));
        return true;
      };

      urlRe.lastIndex = 0;
      let um: RegExpExecArray | null = null;
      let lastUrlIdx = 0;
      let didReplace = false;
      const frag = document.createDocumentFragment();

      while ((um = urlRe.exec(rawText))) {
        didReplace = true;
        const start = um.index;
        const end = start + um[0].length;
        const before = rawText.slice(lastUrlIdx, start);
        if (before) appendTextWithFileLinks(frag, before);

        const normalizedUrl = normalizeToken(um[0]);
        if (normalizedUrl) {
          const sp = document.createElement("span");
          sp.className = "autoUrlLink";
          sp.dataset.openUrl = normalizedUrl;
          sp.title = "Ctrl/Cmd+Click to open";
          sp.textContent = um[0];
          frag.appendChild(sp);
        } else {
          frag.appendChild(document.createTextNode(um[0]));
        }

        lastUrlIdx = end;
      }

      if (didReplace) {
        const tail = rawText.slice(lastUrlIdx);
        if (tail) appendTextWithFileLinks(frag, tail);
        node.parentNode?.replaceChild(frag, node);
        continue;
      }

      const fileOnlyFrag = document.createDocumentFragment();
      const changed = appendTextWithFileLinks(fileOnlyFrag, rawText);
      if (!changed) continue;
      node.parentNode?.replaceChild(fileOnlyFrag, node);
    }
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

    const active = suggestEl.querySelector(
      ".suggestItem.active",
    ) as HTMLElement | null;
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
          ? "/" + label.slice("/prompts:".length)
          : label.startsWith("@")
            ? label.slice(1)
            : label.startsWith("$")
              ? label.slice(1)
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
      const agents = state.capabilities?.agents ?? false;

      if (query.length > 0 || atTok.token === "@") {
        if (agents) {
          if (agentIndex && agentIndexForSessionId === state.activeSession.id) {
            const q = query.toLowerCase();
            const rankedNames = agentIndex
              .filter((n) => n.toLowerCase().includes(q))
              .slice(0, 50);
            const agentItems = rankedNames.map((name) => ({
              insert: "@agents:" + name + " ",
              label: "@agents:" + name,
              detail: "",
              kind: "agent" as const,
            }));
            items = items.concat(agentItems);
          } else {
            if (agentIndexRequestedForSessionId !== state.activeSession.id) {
              agentIndexRequestedForSessionId = state.activeSession.id;
              vscode.postMessage({
                type: "requestAgentIndex",
                sessionId: state.activeSession.id,
              });
            }
          }
        }

        // File search (TUI-like): run a query-based search rather than indexing all files.
        if (query.length >= FILE_SEARCH_MIN_QUERY_LEN) {
          const existing =
            fileSearch &&
            fileSearch.sessionId === state.activeSession.id &&
            fileSearch.query === query
              ? fileSearch
              : null;

          if (existing) {
            const ranked = rankFilePaths(existing.paths, query);
            const dirItems = rankDirPrefixes(ranked, query).map((p) => ({
              insert: "@" + p,
              label: "@" + p,
              detail: "dir",
              kind: "dir" as const,
            }));
            const fileItems = ranked.map((p) => ({
              insert: "@" + p + " ",
              label: "@" + p,
              detail: "",
              kind: "file" as const,
            }));
            // Prefer agents (above), then directories, then files.
            items = items.concat(dirItems, fileItems);
          } else {
            scheduleFileSearch(state.activeSession.id, query);
            items = items.concat([
              {
                insert: "",
                label: "(searching…)",
                detail: "",
                kind: "file",
              },
            ]);
          }
        } else if (query.length > 0) {
          items = items.concat([
            {
              insert: "",
              label: "(type 2+ chars to search files)",
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

    const dollarTok = currentPrefixedToken("$");
    if (dollarTok) {
      const query = dollarTok.token.slice(1);
      const sessionId = state.activeSession.id;

      if (
        (!skillIndex || skillIndexForSessionId !== sessionId) &&
        skillIndexRequestedForSessionId !== sessionId
      ) {
        skillIndexRequestedForSessionId = sessionId;
        vscode.postMessage({ type: "requestSkillIndex", sessionId });
      }

      if (!skillIndex || skillIndexForSessionId !== sessionId) {
        suggestItems = [];
        activeReplace = null;
        renderSuggest();
        return;
      }

      const q = query.toLowerCase();
      const filtered = query
        ? skillIndex.filter((s) => {
            const name = (s.name || "").toLowerCase();
            if (name.includes(q)) return true;
            const desc = (s.description || "").toLowerCase();
            return desc.includes(q);
          })
        : skillIndex;

      const items = filtered.slice(0, 50).map((s) => ({
        insert: `$${s.name} `,
        label: `$${s.name}`,
        detail: s.description ? truncateOneLine(s.description, 60) : "",
        kind: "skill" as const,
      }));

      const ranked = query ? rankByPrefix(items, query) : items;
      const nextReplace = {
        from: dollarTok.start,
        to: dollarTok.end,
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
          const ranked = query ? rankByPrefix(allSlash, "/" + query) : allSlash;
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
    saveComposerState();

    // If a directory is selected, keep the suggest UI open to allow drilling down.
    const keepOpen =
      it.kind === "dir" ||
      (it.insert.startsWith("@") && it.insert.endsWith("/"));

    if (!keepOpen) {
      // Close suggest UI after accepting; subsequent Enter should send.
      suggestItems = [];
      activeReplace = null;
      renderSuggest();
      return;
    }

    autosizeInput();
    updateSuggestions();
  }

  function scheduleRender(): void {
    if (renderScheduled) return;
    renderScheduled = true;
    requestAnimationFrame(() => {
      renderScheduled = false;
      if (!pendingState) return;
      const renderedSeq = pendingStateSeq;
      try {
        renderFull(pendingState);
      } catch (err) {
        vscode.postMessage({
          type: "uiError",
          message: `Webview render failed: ${String((err as Error)?.message ?? err)}`,
        });
      }
      pendingState = null;
      pendingStateSeq = null;
      autosizeInput();
      if (typeof renderedSeq === "number") {
        vscode.postMessage({ type: "stateAck", seq: renderedSeq });
      }
    });
  }

  function scheduleControlRender(): void {
    if (controlRenderScheduled) return;
    controlRenderScheduled = true;
    requestAnimationFrame(() => {
      controlRenderScheduled = false;
      if (!pendingControlState) return;
      const renderedSeq = pendingControlSeq;
      try {
        renderControl(pendingControlState);
      } catch (err) {
        vscode.postMessage({
          type: "uiError",
          message: `Webview render(control) failed: ${String((err as Error)?.message ?? err)}`,
        });
      }
      pendingControlState = null;
      pendingControlSeq = null;
      autosizeInput();
      if (typeof renderedSeq === "number") {
        vscode.postMessage({ type: "stateAck", seq: renderedSeq });
      }
    });
  }

  function scheduleBlocksRender(): void {
    if (blocksRenderScheduled) return;
    blocksRenderScheduled = true;
    requestAnimationFrame(() => {
      blocksRenderScheduled = false;
      if (!pendingBlocksState) return;
      try {
        renderBlocks(pendingBlocksState);
      } catch (err) {
        vscode.postMessage({
          type: "uiError",
          message: `Webview render(blocks) failed: ${String((err as Error)?.message ?? err)}`,
        });
      }
      pendingBlocksState = null;
      updateReturnToBottomVisibility();
    });
  }

  function renderFull(s: ChatViewState): void {
    state = s;
    renderControl(s);
    renderBlocks(s);
  }

  function normalizeFsPath(p: string): string {
    const trimmed = String(p || "").trim();
    if (!trimmed) return "";
    if (trimmed === "/") return "/";
    return trimmed.replace(/\/+$/, "");
  }

  function workspacePathForSession(sess: Session): string {
    return normalizeFsPath(uriToHashKey(String(sess.workspaceFolderUri || "")));
  }

  function shouldShowGlobalBlock(
    block: ChatBlock,
    sess: Session | null,
  ): boolean {
    if (!sess) return true;

    const id = String((block as any).id || "");
    const title = "title" in block ? String((block as any).title || "") : "";
    if (block.type !== "info") return true;

    const activeBackendId = sess.backendId;
    const activeCwd = workspacePathForSession(sess);

    if (title === "Thread started") {
      const backendPrefix = "global:threadStarted:backend:";
      const legacyCwdPrefix = "global:threadStarted:cwd:";
      if (id.startsWith(backendPrefix)) {
        const rest = id.slice(backendPrefix.length);
        const idx = rest.indexOf(":cwd:");
        if (idx <= 0) return false;
        const backendId = rest.slice(0, idx);
        const cwd = normalizeFsPath(rest.slice(idx + ":cwd:".length));
        return backendId === activeBackendId && cwd === activeCwd;
      }
      if (id.startsWith(legacyCwdPrefix)) {
        const cwd = normalizeFsPath(id.slice(legacyCwdPrefix.length));
        return activeBackendId !== "opencode" && cwd === activeCwd;
      }
    }

    if (title === "OpenCode started") {
      const prefix = "global:opencodeStarted:cwd:";
      if (!id.startsWith(prefix)) return true;
      const cwd = normalizeFsPath(id.slice(prefix.length));
      return activeBackendId === "opencode" && cwd === activeCwd;
    }

    return true;
  }

  function renderControl(s: ChatViewState): void {
    state = s;
    titleEl.textContent = (() => {
      const active = s.activeSession;
      if (!active) return "Codex UI (no session selected)";
      let idx = -1;
      if (!active.customTitle) {
        const sessionsList = s.sessions || [];
        let seen = 0;
        for (const sess of sessionsList) {
          if (sess.backendId !== active.backendId) continue;
          if (sess.workspaceFolderUri !== active.workspaceFolderUri) continue;
          if (sess.id === active.id) {
            idx = seen;
            break;
          }
          seen += 1;
        }
      }
      return getSessionDisplayTitle(active, idx).label;
    })();

    const ms = s.modelState || {
      model: null,
      provider: null,
      reasoning: null,
    };
    const models = s.models ?? [];
    const backendId = s.activeSession?.backendId ?? null;
    const opencodeDefaultKey =
      backendId === "opencode" ? String(s.opencodeDefaultModelKey || "") : "";
    const opencodeDefaultDisplay = (() => {
      if (backendId !== "opencode") return null;
      if (!opencodeDefaultKey) return "default (opencode config)";
      const match = models.find(
        (m) => (m.model || m.id) === opencodeDefaultKey,
      );
      const label = match?.displayName ? match.displayName : opencodeDefaultKey;
      return `default (opencode: ${label})`;
    })();
    const cliDefaultDisplay = (() => {
      if (backendId === "opencode") return null;
      const d = s.cliDefaultModelState || {
        model: null,
        provider: null,
        reasoning: null,
      };
      const provider = d.provider ? String(d.provider).trim() : "";
      const model = d.model ? String(d.model).trim() : "";
      if (provider && model)
        return `default (CLI config: ${provider} / ${model})`;
      if (provider) return `default (CLI config: ${provider} / default)`;
      if (model) return `default (CLI config: ${model})`;
      const backendDefault = models.find((m) => Boolean(m.isDefault));
      if (backendDefault) {
        const label = String(
          backendDefault.displayName ||
            backendDefault.model ||
            backendDefault.id,
        );
        return `default (${backendId || "backend"}: ${label})`;
      }
      return "default (CLI config)";
    })();
    const modelOptions = [
      { value: "default", label: "default" },
      ...models.map((m) => ({
        value: String(m.model || m.id),
        label: String(m.displayName || m.model || m.id),
      })),
    ];
    const selectedModelKey = String(ms.model || "").trim() || "default";
    const defaultModelLabel = (() => {
      if (backendId === "opencode") {
        if (!opencodeDefaultKey) return "default";
        const match = models.find((m) => (m.model || m.id) === opencodeDefaultKey);
        return String(match?.displayName || opencodeDefaultKey);
      }
      const d = s.cliDefaultModelState || { model: null, provider: null, reasoning: null };
      const provider = d.provider ? String(d.provider).trim() : "";
      const model = d.model ? String(d.model).trim() : "";
      if (provider && model) return `${provider} / ${model}`;
      if (provider) return `${provider} / default`;
      if (model) return model;
      const backendDefault = models.find((m) => Boolean(m.isDefault));
      if (backendDefault) {
        const label = String(backendDefault.displayName || backendDefault.model || backendDefault.id);
        return label;
      }
      return "default";
    })();
    modelSelect.title = selectedModelKey === "default" ? defaultModelLabel : "";
    populateSelectWithLabels(modelSelect, modelOptions, ms.model, {
      defaultLabel: defaultModelLabel,
    });

    const effortOptions = (() => {
      if (!ms.model || models.length === 0)
        return ["default", "none", "minimal", "low", "medium", "high", "xhigh"];
      const model = models.find(
        (m) => m.model === ms.model || m.id === ms.model,
      );
      if (!model)
        return ["default", "none", "minimal", "low", "medium", "high", "xhigh"];
      const supported =
        model.supportedReasoningEfforts
          ?.map((o) => o.reasoningEffort)
          .filter((v): v is string => typeof v === "string" && v.length > 0) ??
        [];
      if (supported.length === 0)
        return ["default", "none", "minimal", "low", "medium", "high", "xhigh"];
      return ["default", ...supported];
    })();
    const defaultReasoningLabel = (() => {
      if (backendId === "opencode") {
        // opencode does not expose the effective default variant/effort; show the explicit options instead.
        return "server default";
      }
      const d = s.cliDefaultModelState || { model: null, provider: null, reasoning: null };
      const reasoning = d.reasoning ? String(d.reasoning).trim() : "";
      if (reasoning) return reasoning;
      const backendDefault = models.find((m) => Boolean(m.isDefault));
      if (backendDefault) {
        const eff = String(backendDefault.defaultReasoningEffort || "").trim();
        if (eff) return eff;
      }
      return "default";
    })();
    const reasoningOptions = effortOptions.map((v) => ({
      value: v,
      label: v === "default" ? defaultReasoningLabel : v,
    }));
    const selectedReasoningKey = String(ms.reasoning || "").trim() || "default";
    reasoningSelect.title =
      selectedReasoningKey === "default" ? defaultReasoningLabel : "";
    populateSelectWithLabels(reasoningSelect, reasoningOptions, ms.reasoning, {
      defaultLabel: defaultReasoningLabel,
    });

    // Mode selector for opencode sessions (Build/Plan)
    const agents = s.opencodeAgents ?? [];
    if (backendId === "opencode") {
      agentSelect.style.display = "";
      const buildLabel =
        agents.find(
          (a) =>
            String(a.id || "")
              .trim()
              .toLowerCase() === "build",
        )?.name || "build";
      const planLabel =
        agents.find(
          (a) =>
            String(a.id || "")
              .trim()
              .toLowerCase() === "plan",
        )?.name || "plan";
      const agentOptions = [
        { value: "default", label: "default" },
        { value: "build", label: buildLabel },
        { value: "plan", label: planLabel },
      ];
      const defaultAgent = String(s.opencodeDefaultAgentName || "").trim();
      const defaultAgentLabel = defaultAgent || "default";
      agentSelect.title = defaultAgent ? `default_agent=${defaultAgent}` : "";
      populateSelectWithLabels(agentSelect, agentOptions, ms.agent ?? null, {
        defaultLabel: defaultAgentLabel,
      });
    } else {
      agentSelect.style.display = "none";
    }

    const fullStatus = String(s.statusText || "").trim();
    const statusTooltip = String(s.statusTooltip || fullStatus).trim();
    const parsed = parseFooterStatus(fullStatus);
    const candidates = buildFooterStatusCandidates(parsed);

    // Constrain the footer status area so it never steals space from the selectors.
    const gapPx = 10;
    const availablePx = Math.max(
      0,
      footerBarEl.clientWidth -
        Math.round(modelBarEl.getBoundingClientRect().width) -
        gapPx,
    );
    statusTextEl.style.maxWidth = `${availablePx}px`;

    statusPopoverDetails = fullStatus;
    // Include the full status (e.g. percentages) in the hover popover so users can
    // see both the compact headline and the tooltip details (e.g. reset time).
    statusHoverDetails =
      statusTooltip !== fullStatus
        ? fullStatus
          ? `${fullStatus}\n\n${statusTooltip}`
          : statusTooltip
        : "";
    statusPopoverEnabled = false;

    // Do not dismiss the hover popover during refresh. Rate limit updates (and other
    // streaming refreshes) can arrive frequently; keep the popover visible while
    // the user is hovering so the tooltip remains readable.
    if (statusPopoverOpen) {
      statusPopoverEl.textContent = statusPopoverDetails;
      statusPopoverEl.style.display = "";
    } else if (statusTextHovering) {
      if (statusHoverDetails) {
        statusPopoverEl.textContent = statusHoverDetails;
        statusPopoverEl.style.display = "";
      } else {
        hideStatusPopover();
      }
    } else {
      hideStatusPopover();
    }

    if (fullStatus && candidates.length > 0) {
      let chosen = candidates[candidates.length - 1]!;
      for (const c of candidates) {
        statusTextEl.textContent = c.text;
        statusTextEl.title = statusTooltip;
        statusTextEl.style.display = "";
        if (fitsStatusText()) {
          chosen = c;
          break;
        }
      }
      statusPopoverEnabled = chosen.tier >= 2;
      statusTextEl.classList.toggle("clickable", statusPopoverEnabled);
    } else {
      statusTextEl.textContent = fullStatus;
      statusTextEl.title = statusTooltip;
      statusTextEl.style.display = fullStatus ? "" : "none";
      statusTextEl.classList.remove("clickable");
      statusPopoverEnabled = false;
    }
    if (diffBtn) diffBtn.disabled = !s.hasLatestDiff;
    sendBtn.disabled = !s.activeSession;
    sendBtn.dataset.mode = s.sending ? "stop" : "send";
    sendBtn.setAttribute("aria-label", s.sending ? "Stop" : "Send");
    sendBtn.title = s.sending ? "Stop (Esc)" : "Send (Enter)";
    if (statusBtn) statusBtn.disabled = !s.activeSession || s.sending;
    resumeBtn.disabled = s.sending;
    attachBtn.disabled = !s.activeSession;
    reloadBtn.disabled =
      !s.activeSession || s.sending || s.reloading || backendId !== "codez";
    reloadBtn.title =
      backendId === "codez"
        ? "Reload session (re-read config.toml, agents, etc.)"
        : "Reload session (codez sessions only)";
    settingsBtn.disabled = false;
    settingsBtn.title = backendId
      ? `Settings (session backend: ${backendId})`
      : "Settings";
    if (backendId !== "codez" && rewindTurnIndex !== null) setEditMode(null);
    // Keep input enabled so the user can draft messages even before selecting a session,
    // but do not override ask_user_question which intentionally locks the composer.
    // Sending is still guarded by sendBtn.disabled and sendCurrentInput().
    if (!askUserQuestionState) inputEl.disabled = false;
    updateInputPlaceholder();

    const hydrationText = String(s.hydrationBlockedText || "").trim();
    if (hydrationText && s.activeSession) {
      hydrateBannerEl.replaceChildren();
      const text = document.createElement("div");
      text.className = "hydrateBannerText";
      text.textContent = hydrationText;
      hydrateBannerEl.appendChild(text);

      const actions = document.createElement("div");
      actions.className = "hydrateBannerActions";

      const loadBtn = document.createElement("button");
      loadBtn.className = "hydrateBannerBtn primary";
      loadBtn.textContent = "Load history";
      loadBtn.disabled = Boolean(s.sending || s.reloading);
      loadBtn.addEventListener("click", () => {
        const sessionId = s.activeSession?.id ?? null;
        if (!sessionId) return;
        vscode.postMessage({ type: "loadSessionHistory", sessionId });
      });
      actions.appendChild(loadBtn);

      hydrateBannerEl.appendChild(actions);
      hydrateBannerEl.style.display = "flex";
    } else {
      if (hydrateBannerEl.style.display !== "none")
        hydrateBannerEl.style.display = "none";
      if (hydrateBannerEl.childNodes.length > 0)
        hydrateBannerEl.replaceChildren();
    }

    const sessionsList = s.sessions || [];
    const unread = new Set<string>(s.unreadSessionIds || []);
    const running = new Set<string>(s.runningSessionIds || []);
    const activeId = s.activeSession ? s.activeSession.id : null;
    const workspaceColorOverrides = s.workspaceColorOverrides || {};
    const overridesSig = workspaceOverridesSig(workspaceColorOverrides);

    // The tab UI groups sessions by workspace folder; match the Sessions tree numbering
    // (index within workspace + backend), rather than using the global session index.
    const visibleSessions = sessionsList;
    const displayIndexBySessionId = new Map<string, number>();
    const nextIndexByGroupKey = new Map<string, number>();
    for (const sess of visibleSessions) {
      const groupKey = `${sess.workspaceFolderUri}\t${sess.backendId}`;
      const next = nextIndexByGroupKey.get(groupKey) ?? 0;
      displayIndexBySessionId.set(sess.id, next);
      nextIndexByGroupKey.set(groupKey, next + 1);
    }

    const nextTabsSig = visibleSessions
      .map((sess) => {
        const isUnread = unread.has(sess.id);
        const isRunning = running.has(sess.id);
        const isActive = activeId === sess.id;
        const displayIdx = displayIndexBySessionId.get(sess.id) ?? -1;
        const dt = getSessionDisplayTitle(sess, displayIdx);
        return [
          sess.id,
          sess.workspaceFolderUri,
          dt.label,
          dt.tooltip,
          isActive ? "a" : "",
          isRunning ? "r" : "",
          isUnread ? "u" : "",
        ].join("\t");
      })
      .join("\n");

    const sig = `${overridesSig}\n${nextTabsSig}`;

    if (tabsSig !== sig) {
      tabsSig = sig;
      const frag = document.createDocumentFragment();
      const wanted = new Set<string>();
      const groupElByWorkspaceUri = new Map<string, HTMLDivElement>();
      const groupTabsElByWorkspaceUri = new Map<string, HTMLDivElement>();
      const groupOrder: string[] = [];

      visibleSessions.forEach((sess) => {
        wanted.add(sess.id);
        const isUnread = unread.has(sess.id);
        const isRunning = running.has(sess.id);
        const isActive = activeId === sess.id;
        const displayIdx = displayIndexBySessionId.get(sess.id) ?? -1;
        const dt = getSessionDisplayTitle(sess, displayIdx);

        const groupKey = sess.workspaceFolderUri;
        let groupEl = groupElByWorkspaceUri.get(groupKey);
        if (!groupEl) {
          groupEl = document.createElement("div") as HTMLDivElement;
          groupEl.className = "tabGroup";

          const wt = workspaceTagFromUri(groupKey, workspaceColorOverrides);
          groupEl.style.setProperty("--wt-color", wt.color);

          const labelEl = document.createElement("div") as HTMLDivElement;
          labelEl.className = "tabGroupLabel";
          labelEl.textContent = wt.label;
          labelEl.addEventListener("contextmenu", (e) => {
            e.preventDefault();
            vscode.postMessage({
              type: "pickWorkspaceColor",
              workspaceFolderUri: groupKey,
            });
          });
          groupEl.appendChild(labelEl);

          const tabsEl = document.createElement("div") as HTMLDivElement;
          tabsEl.className = "tabGroupTabs";
          groupEl.appendChild(tabsEl);
          groupTabsElByWorkspaceUri.set(groupKey, tabsEl);

          groupElByWorkspaceUri.set(groupKey, groupEl);
          groupOrder.push(groupKey);
        }

        const groupTabsEl = groupTabsElByWorkspaceUri.get(groupKey);
        if (!groupTabsEl)
          throw new Error(`tab group tabs element missing for ${groupKey}`);

        const existing = tabElBySessionId.get(sess.id);
        const div =
          existing ?? (document.createElement("div") as HTMLDivElement);
        if (!existing) {
          tabElBySessionId.set(sess.id, div);
          div.addEventListener("click", () =>
            vscode.postMessage({ type: "selectSession", sessionId: sess.id }),
          );
          div.addEventListener("contextmenu", (e) => {
            e.preventDefault();
            vscode.postMessage({ type: "sessionMenu", sessionId: sess.id });
          });
        }
        div.className =
          "tab" +
          (isActive ? " active" : "") +
          (isRunning ? " running" : isUnread ? " unread" : "");
        if (div.textContent !== dt.label) div.textContent = dt.label;
        if (div.title !== dt.tooltip) div.title = dt.tooltip;
        groupTabsEl.appendChild(div);
      });

      for (const groupKey of groupOrder) {
        const groupEl = groupElByWorkspaceUri.get(groupKey);
        if (groupEl) frag.appendChild(groupEl);
      }

      for (const [id, div] of tabElBySessionId.entries()) {
        if (wanted.has(id)) continue;
        if (div.parentElement) div.parentElement.removeChild(div);
        tabElBySessionId.delete(id);
      }
      tabsEl.replaceChildren(frag);
      window.requestAnimationFrame(() => syncTabGroupLabelWidths());
    }

    const nextSessionId = s.activeSession ? s.activeSession.id : null;
    if (domSessionId !== nextSessionId) {
      // Persist the previous session's draft before switching.
      // If the user is browsing input history, exit that mode so the draft is saved.
      exitInputHistoryNavigation(domSessionId);
      saveComposerState();
      domSessionId = nextSessionId;
      restoreComposerState(domSessionId);
      setEditMode(null);
      blockElByKey.clear();
      logEl.replaceChildren();
      // New session / fresh log should start pinned.
      stickLogToBottom = true;
      forceScrollToBottomNextRender = true;

      // If blocks for the next session are already known, immediately re-render them.
      // This avoids a race where a blocks render runs before this control render,
      // and then gets wiped by the DOM reset above.
      if (domSessionId) {
        const cached =
          pendingBlocksBySessionId.get(domSessionId) ??
          blocksBySessionId.get(domSessionId) ??
          null;
        if (cached) {
          pendingBlocksBySessionId.delete(domSessionId);
          blocksBySessionId.set(domSessionId, cached);
          touchBlockCache(domSessionId);
          state = { ...(state as any), blocks: cached } as ChatViewState;
          pendingBlocksState = state;
          scheduleBlocksRender();
        }
      }
    }

    const approvals = s.approvals || [];
    const approvalsVisible = Boolean(s.activeSession && approvals.length > 0);
    const nextApprovalsSig = approvalsVisible
      ? approvals
          .map((ap) =>
            [
              ap.requestKey,
              ap.canAcceptForSession ? "s" : "",
              ap.title,
              ap.detail,
            ].join("\t"),
          )
          .join("\n")
      : "hidden";

    if (approvalsSig !== nextApprovalsSig) {
      approvalsSig = nextApprovalsSig;
      approvalsEl.innerHTML = "";
      if (approvalsVisible) {
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
          btnDecline.addEventListener("click", () => {
            vscode.postMessage({
              type: "approve",
              requestKey: ap.requestKey,
              decision: "decline",
            });
            vscode.postMessage({ type: "stop" });
          });
          actions.appendChild(btnDecline);

          const btnCancel = document.createElement("button");
          btnCancel.textContent = "Cancel";
          btnCancel.addEventListener("click", () => {
            vscode.postMessage({
              type: "approve",
              requestKey: ap.requestKey,
              decision: "cancel",
            });
            vscode.postMessage({ type: "stop" });
          });
          actions.appendChild(btnCancel);

          card.appendChild(actions);
          approvalsEl.appendChild(card);
        }
      } else {
        approvalsEl.style.display = "none";
      }
    } else {
      approvalsEl.style.display = approvalsVisible ? "" : "none";
    }

    const globalBlocks = (s.globalBlocks || []).filter((b) =>
      shouldShowGlobalBlock(b, s.activeSession),
    );
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

    updateSuggestions();
  }

  function renderBlocks(s: ChatViewState): void {
    if (!s.activeSession) return;

    const shouldAutoScroll = stickLogToBottom && isLogNearBottom();
    const forceScrollToBottom = forceScrollToBottomNextRender;
    forceScrollToBottomNextRender = false;

    const reorderBlocksForOpencode = (blocks: ChatBlock[]): ChatBlock[] => {
      const getSeq = (b: ChatBlock): number | null => {
        const raw = (b as any)?.opencodeSeq;
        if (typeof raw !== "number" || !Number.isFinite(raw)) return null;
        return Math.trunc(raw);
      };
      const offsetFor = (b: ChatBlock): number => {
        const explicit = (b as any)?.opencodeOffset;
        if (typeof explicit === "number" && Number.isFinite(explicit))
          return Math.trunc(explicit);
        if (b.type === "reasoning") return 0;
        if (b.type === "step") return 5;
        if (b.type === "assistant") return 9;
        return 9;
      };
      const scored = blocks.map((b, originalIndex) => ({
        b,
        originalIndex,
        seq: getSeq(b),
        offset: offsetFor(b),
      }));
      scored.sort((a, b) => {
        // Only reorder blocks when both sides have a known OpenCode message sequence.
        // This avoids reordering unrelated blocks (e.g. user messages) while still fixing
        // the common issue where Step/Reasoning arrive after the final response.
        if (a.seq === null || b.seq === null) {
          return a.originalIndex - b.originalIndex;
        }
        if (a.seq !== b.seq) return a.seq - b.seq;
        if (a.offset !== b.offset) return a.offset - b.offset;
        return a.originalIndex - b.originalIndex;
      });
      return scored.map((x) => x.b);
    };

	    const blocks = (() => {
	      const raw = s.blocks || [];
	      return s.activeSession?.backendId === "opencode"
	        ? reorderBlocksForOpencode(raw)
	        : raw;
	    })();

	    const opencodeReorder = s.activeSession?.backendId === "opencode";
	    const cursorStart = (() => {
	      if (!opencodeReorder) return null;
	      let cur: ChildNode | null = logEl.firstChild;
	      while (cur) {
	        const el = cur as HTMLElement;
	        const cls = typeof el.className === "string" ? el.className : "";
	        if (el.tagName.toLowerCase() === "details" && /\bnotice\b/.test(cls)) {
	          cur = cur.nextSibling;
	          continue;
	        }
	        break;
	      }
	      return cur;
	    })();
	    let cursor: ChildNode | null = cursorStart;
	    const placeTopLevel = (el: HTMLElement): void => {
	      if (!opencodeReorder) return;
	      // Reorder DOM to match the (potentially reordered) blocks list.
	      if (el.parentElement !== logEl || el !== cursor) {
	        logEl.insertBefore(el, cursor);
	      }
	      cursor = el.nextSibling;
	    };

    let userTurnIndex = 0;
    for (const block of blocks) {
	      if (block.type === "divider") {
        const key = "b:" + block.id;
        const dividerText = String(block.text ?? "");
        const hasText = dividerText.trim().length > 0;
        const hasStatus = Boolean(block.status);
        if (!hasText && !hasStatus) {
          const existing = blockElByKey.get(key);
          if (existing?.parentElement)
            existing.parentElement.removeChild(existing);
          blockElByKey.delete(key);
          delete detailsState[key];
          continue;
        }
        const div = ensureDiv(key, "msg system divider");
        const status = block.status;
        const existingIcon = div.querySelector(
          'span[data-k="statusIcon"]',
        ) as HTMLSpanElement | null;
        if (status) {
          const icon =
            existingIcon ??
            (() => {
              const el = document.createElement("span");
              el.dataset.k = "statusIcon";
              div.appendChild(el);
              return el;
            })();
          const className = `statusIcon status-${status}`;
          if (icon.className !== className) icon.className = className;
          const label =
            status === "inProgress"
              ? "Compacting"
              : status === "completed"
                ? "Completed"
                : "Failed";
          if (icon.getAttribute("aria-label") !== label)
            icon.setAttribute("aria-label", label);
        } else if (existingIcon) {
          existingIcon.remove();
        }
	        const pre = ensurePre(div, "body");
	        if (pre.textContent !== dividerText) pre.textContent = dividerText;
	        placeTopLevel(div);
	        continue;
	      }

	      if (block.type === "note") {
	        const key = "b:" + block.id;
	        const div = ensureDiv(key, "note");
	        const text = String(block.text ?? "");
	        if (div.textContent !== text) div.textContent = text;
	        placeTopLevel(div);
	        continue;
	      }

	      if (block.type === "image") {
        const key = "b:" + block.id;
        const div = ensureDiv(
          key,
          "msg imageBlock imageBlock-" + String(block.role || "system"),
        );
        const titleEl =
          (div.querySelector('div[data-k="title"]') as HTMLDivElement | null) ??
          (() => {
            const t = document.createElement("div");
            t.dataset.k = "title";
            t.className = "imageTitle";
            div.appendChild(t);
            return t;
          })();
        if (titleEl.textContent !== block.title)
          titleEl.textContent = block.title;

        const img =
          (div.querySelector(
            'img[data-k="image"]',
          ) as HTMLImageElement | null) ??
          (() => {
            const i = document.createElement("img");
            i.dataset.k = "image";
            i.className = "imageContent";
            i.loading = "lazy";
            div.appendChild(i);
            return i;
          })();
        img.alt = block.alt || "image";

        const captionEl =
          (div.querySelector(
            'div[data-k="caption"]',
          ) as HTMLDivElement | null) ??
          (() => {
            const c = document.createElement("div");
            c.dataset.k = "caption";
            c.className = "imageCaption";
            div.appendChild(c);
            return c;
          })();
	        void ensureImageRendered(block, img, captionEl).catch((err) => {
	          captionEl.textContent = `画像の描画に失敗: ${String(err)}`;
	          captionEl.style.display = "";
	        });
	        placeTopLevel(div);
	        continue;
	      }

      if (block.type === "imageGallery") {
        const key = "b:" + block.id;
        const div = ensureDiv(
          key,
          "msg imageGallery imageGallery-" + String(block.role || "system"),
        );
        const titleEl =
          (div.querySelector('div[data-k="title"]') as HTMLDivElement | null) ??
          (() => {
            const t = document.createElement("div");
            t.dataset.k = "title";
            t.className = "imageGalleryTitle";
            div.appendChild(t);
            return t;
          })();
        if (titleEl.textContent !== block.title)
          titleEl.textContent = block.title;

        const gridEl =
          (div.querySelector('div[data-k="grid"]') as HTMLDivElement | null) ??
          (() => {
            const g = document.createElement("div");
            g.dataset.k = "grid";
            g.className = "imageGalleryGrid";
            div.appendChild(g);
            return g;
          })();

        gridEl.replaceChildren();
        for (let i = 0; i < block.images.length; i++) {
          const imageRef = block.images[i]!;
          const tile = document.createElement("div");
          tile.className = "imageGalleryTile";

          const img =
            (tile.querySelector(
              'img[data-k="image"]',
            ) as HTMLImageElement | null) ??
            (() => {
              const im = document.createElement("img");
              im.dataset.k = "image";
              im.className = "imageGalleryImage";
              im.loading = "lazy";
              tile.appendChild(im);
              return im;
            })();
          img.alt = imageRef.alt || "image";

          const captionEl =
            (tile.querySelector(
              'div[data-k="caption"]',
            ) as HTMLDivElement | null) ??
            (() => {
              const c = document.createElement("div");
              c.dataset.k = "caption";
              c.className = "imageGalleryCaption";
              tile.appendChild(c);
              return c;
            })();

          void ensureImageRendered(imageRef, img, captionEl).catch((err) => {
            captionEl.textContent = `画像の描画に失敗: ${String(err)}`;
            captionEl.style.display = "";
          });

          gridEl.appendChild(tile);
        }
        placeTopLevel(div);
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
        placeTopLevel(div);
        continue;
      }

	      if (block.type === "user" || block.type === "assistant") {
	        const key = "b:" + block.id;
	        const div = ensureDiv(
	          key,
	          "msg " + (block.type === "user" ? "user" : "assistant"),
	        );

        if (block.type === "user") {
          const backendId = state.activeSession?.backendId ?? null;
          const canEdit = backendId === "codez" || backendId === "opencode";
          userTurnIndex += 1;
          div.dataset.turnIndex = String(userTurnIndex);

          const header =
            (div.querySelector(
              ':scope > div[data-k="header"]',
            ) as HTMLDivElement | null) ??
            (() => {
              const h = document.createElement("div");
              h.dataset.k = "header";
              h.className = "msgHeader";
              div.prepend(h);
              return h;
            })();

          const title =
            (header.querySelector(
              ':scope > div[data-k="title"]',
            ) as HTMLDivElement | null) ??
            (() => {
              const t = document.createElement("div");
              t.dataset.k = "title";
              t.className = "msgHeaderTitle";
              header.appendChild(t);
              return t;
            })();
          title.textContent = `Turn #${userTurnIndex}`;

          const actions =
            (header.querySelector(
              ':scope > div[data-k="actions"]',
            ) as HTMLDivElement | null) ??
            (() => {
              const a = document.createElement("div");
              a.dataset.k = "actions";
              a.className = "msgActions";
              header.appendChild(a);
              return a;
            })();

          actions.replaceChildren();

          const editBtn = document.createElement("button");
          editBtn.className = "msgActionBtn";
          editBtn.textContent = "Edit";
          editBtn.disabled = Boolean(state.sending);
          editBtn.title = canEdit
            ? "Edit this turn (rewind)"
            : "Edit (codez/opencode sessions only)";
          editBtn.addEventListener("click", () => {
            if (state.sending) return;
            if (!canEdit) {
              showToast(
                "info",
                "Edit/Rewind は codez または opencode セッションのみ対応です。",
              );
              return;
            }
            setEditMode(userTurnIndex, block.text);
            inputEl.focus();
          });
          actions.appendChild(editBtn);
        } else {
          const header = div.querySelector(':scope > div[data-k="header"]');
          if (header) header.remove();
        }

        if (block.type === "assistant" && block.streaming) {
          const mdEl = div.querySelector(`div.md[data-k="body"]`);
          if (mdEl) mdEl.remove();
          const pre = ensurePre(div, "body");
          if (pre.textContent !== block.text) pre.textContent = block.text;
        } else {
          const pre = div.querySelector(`pre[data-k="body"]`);
          if (pre) pre.remove();
          const mdEl = ensureMd(div, "body");
          renderMarkdownInto(mdEl, block.text);
        }

        const metaText =
          typeof (block as any).meta === "string"
            ? String((block as any).meta || "").trim()
            : "";
        const existingMeta = div.querySelector(
          'div[data-k="meta"]',
        ) as HTMLDivElement | null;
        if (!metaText) {
          if (existingMeta) existingMeta.remove();
	        } else {
	          const metaEl =
            existingMeta ??
            (() => {
              const m = document.createElement("div");
              m.dataset.k = "meta";
              m.className = "msgMeta";
              div.appendChild(m);
              return m;
            })();
	          if (metaEl.textContent !== metaText) metaEl.textContent = metaText;
	        }
	        placeTopLevel(div);
	        continue;
	      }

	      if (block.type === "opencodePermission") {
	        const id = "opencodePermission:" + block.id;
        const title = `Permission required: ${String(block.permission ?? "").trim() || "permission"}`;
        const open = block.status === "pending";
        const det = ensureDetails(id, "tool opencodePermission", open, title, id);
        const status =
          block.status === "pending"
            ? "inProgress"
            : block.status === "replied"
              ? "completed"
              : "failed";
        setStatusIcon(det, status);

        const meta = ensureMeta(det, "meta");
        const metaParts: string[] = [];
        if (Array.isArray(block.patterns) && block.patterns.length > 0) {
          metaParts.push(`patterns=${block.patterns.join(", ")}`);
        }
        if (Array.isArray(block.always) && block.always.length > 0) {
          metaParts.push(`always=${block.always.join(", ")}`);
        }
        if (block.reply) metaParts.push(`reply=${block.reply}`);
        const metaText = metaParts.join(" ");
        if (meta.textContent !== metaText) meta.textContent = metaText;

        const bodyLines: string[] = [];
        if (Array.isArray(block.patterns) && block.patterns.length > 0) {
          bodyLines.push("patterns:");
          for (const p of block.patterns) bodyLines.push(`- ${p}`);
        }
        if (Array.isArray(block.always) && block.always.length > 0) {
          if (bodyLines.length > 0) bodyLines.push("");
          bodyLines.push("always:");
          for (const a of block.always) bodyLines.push(`- ${a}`);
        }
        if (block.metadata) {
          if (bodyLines.length > 0) bodyLines.push("");
          bodyLines.push("metadata:");
          bodyLines.push(JSON.stringify(block.metadata, null, 2));
        }
        if (block.error) {
          if (bodyLines.length > 0) bodyLines.push("");
          bodyLines.push(`error: ${String(block.error)}`);
        }
        const pre = ensurePre(det, "body");
        const bodyText = bodyLines.join("\n").trimEnd();
        if (pre.textContent !== bodyText) pre.textContent = bodyText;

        const actions =
          (det.querySelector(
            ':scope > div[data-k="actions"]',
          ) as HTMLDivElement | null) ??
          (() => {
            const a = document.createElement("div");
            a.dataset.k = "actions";
            a.className = "approvalActions";
            det.appendChild(a);
            return a;
          })();

        actions.replaceChildren();
        const disabled = block.status !== "pending";

        const mk = (label: string, reply: "once" | "always" | "reject") => {
          const btn = document.createElement("button");
          btn.textContent = label;
          btn.disabled = disabled;
          btn.addEventListener("click", () => {
            const sessionId = state.activeSession?.id ?? null;
            if (!sessionId) return;
            vscode.postMessage({
              type: "opencodePermissionReply",
              sessionId,
              requestID: String(block.requestID),
              reply,
            });
          });
          return btn;
        };

	        actions.appendChild(mk("Allow once", "once"));
	        actions.appendChild(mk("Always allow", "always"));
	        actions.appendChild(mk("Reject", "reject"));
	        placeTopLevel(det);
	        continue;
	      }

	      if (block.type === "reasoning") {
        const summary = (block.summaryParts || [])
          .map((s) => String(s ?? ""))
          .filter((s) => s.trim().length > 0)
          .join("");
        const raw = (block.rawParts || [])
          .map((s) => String(s ?? ""))
          .filter((s) => s.trim().length > 0)
          .join("");
        if (!summary.trim() && !raw.trim()) {
          const id = "reasoning:" + block.id;
          for (const [k, el] of blockElByKey.entries()) {
            if (k !== id && !k.startsWith(id + ":")) continue;
            if (el.parentElement) el.parentElement.removeChild(el);
            blockElByKey.delete(k);
            delete detailsState[k];
          }
          continue;
        }

        const id = "reasoning:" + block.id;
        const det = ensureDetails(
          id,
          "reasoning",
          block.status === "inProgress",
          "Reasoning",
          id,
        );
        setStatusIcon(det, block.status);

        if (summary.trim()) {
          const pre = det.querySelector(`pre[data-k="summary"]`);
          if (pre) pre.remove();
          const mdEl = ensureMd(det, "summary");
          renderMarkdownInto(mdEl, summary);
        }
	        if (raw.trim()) {
          const rawId = id + ":raw";
          const rawDet = ensureDetails(rawId, "", false, "Raw", rawId);
          // Ensure raw is nested under the reasoning details.
          if (rawDet.parentElement !== det) det.appendChild(rawDet);
	          const pre = ensurePre(rawDet, "body");
	          if (pre.textContent !== raw) pre.textContent = raw;
	        }
	        placeTopLevel(det);
	        continue;
	      }

      if (block.type === "command") {
        const id = "command:" + block.id;
        if (
          block.hideCommandText &&
          !block.output &&
          (!block.terminalStdin || block.terminalStdin.length === 0) &&
          !block.actionsText &&
          !block.cwd &&
          block.exitCode === null &&
          block.durationMs === null
        ) {
          const existing = blockElByKey.get(id);
          if (existing?.parentElement)
            existing.parentElement.removeChild(existing);
          blockElByKey.delete(id);
          delete detailsState[id];
          continue;
        }
        const displayCmd = block.command
          ? stripShellWrapper(block.command)
          : "";
        const cmdPreview =
          displayCmd && !looksOpaqueToken(displayCmd)
            ? truncateCommand(displayCmd, 120)
            : "";
        const actionsPreview = (block.actionsText || "").trim().split("\n")[0];
        const summaryText = block.hideCommandText
          ? (block.title || "Command") + " (hidden)"
          : cmdPreview
            ? `Command: ${cmdPreview}`
            : actionsPreview
              ? `Command: ${actionsPreview}`
              : block.title || "Command";
        const det = ensureDetails(id, "tool command", false, summaryText, id);
        setStatusIcon(det, block.status);
        if (block.cwd) det.dataset.cwd = block.cwd;
        else delete (det.dataset as any).cwd;

        const sum = det.querySelector(":scope > summary");
        const sumTxt = sum
          ? (sum.querySelector(
              ':scope > span[data-k="summaryText"]',
            ) as HTMLSpanElement | null)
          : null;
        if (sumTxt && block.command && !block.hideCommandText) {
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
        const next = block.hideCommandText
          ? block.output || "[command hidden]"
          : (displayCmd ? "$ " + displayCmd + "\n" : "") + (block.output || "");
        if (pre.textContent !== next) {
          pre.textContent = next;
          delete (pre.dataset as any).fileLinks;
          if (next.length <= MAX_LINKIFY_TEXT_CHARS) linkifyFilePaths(pre);
        }
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
        if (meta.textContent !== metaText) {
          meta.textContent = metaText;
          delete (meta.dataset as any).fileLinks;
          if (metaText.length <= MAX_LINKIFY_TEXT_CHARS) linkifyFilePaths(meta);
        }
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
        const isOpenCode = block.server === "opencode";
        const det = ensureDetails(
          id,
          isOpenCode ? "tool opencode" : "tool mcp",
          false,
          block.title || (isOpenCode ? "OpenCode" : "MCP"),
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

	      if (block.type === "step") {
        const id = "step:" + block.id;
        const tools = Array.isArray(block.tools) ? block.tools : [];
        const toolCount = tools.length;
        const reason =
          typeof block.reason === "string" && block.reason.trim()
            ? block.reason.trim()
            : null;
        const summaryText =
          toolCount > 0
            ? reason
              ? `${block.title} (${String(toolCount)} tools, ${reason})`
              : `${block.title} (${String(toolCount)} tools)`
            : reason
              ? `${block.title} (${reason})`
              : block.title;

        const det = ensureDetails(
          id,
          "tool step opencodeStep",
          block.status === "inProgress",
          summaryText,
          id,
        );
        setStatusIcon(det, block.status);

        const meta = ensureMeta(det, "meta");
        const metaParts: string[] = [];
        if (block.snapshot)
          metaParts.push("snapshot=" + block.snapshot.slice(0, 8));
        if (typeof block.cost === "number")
          metaParts.push("cost=" + String(block.cost));
        if (block.tokens) {
          if (typeof block.tokens.input === "number")
            metaParts.push("in=" + String(block.tokens.input));
          if (typeof block.tokens.output === "number")
            metaParts.push("out=" + String(block.tokens.output));
          if (typeof block.tokens.reasoning === "number")
            metaParts.push("reasoning=" + String(block.tokens.reasoning));
          if (block.tokens.cache) {
            if (typeof block.tokens.cache.read === "number")
              metaParts.push("cacheRead=" + String(block.tokens.cache.read));
            if (typeof block.tokens.cache.write === "number")
              metaParts.push("cacheWrite=" + String(block.tokens.cache.write));
          }
        }
        const metaText = metaParts.join(" ");
        if (meta.textContent !== metaText) meta.textContent = metaText;

        const wantedKeys = new Set<string>();
        for (let ti = 0; ti < tools.length; ti++) {
          const t = tools[ti];
          if (
            !t ||
            typeof t.id !== "string" ||
            typeof t.tool !== "string" ||
            typeof t.title !== "string"
          )
            continue;
          const toolId = `${id}:tool:${t.id}`;
          wantedKeys.add(toolId);
          const inputPreview =
            typeof (t as any).inputPreview === "string"
              ? String((t as any).inputPreview || "").trim()
              : "";
          const suffix = inputPreview ? truncateOneLine(inputPreview, 80) : "";
          const toolSummary = suffix
            ? `${t.tool}: ${t.title} — ${suffix}`
            : `${t.tool}: ${t.title}`;
          const toolDet = ensureNestedDetailsWithStatusIcon(
            det,
            toolId,
            "toolChild",
            false,
            toolSummary,
            toolId,
          );
          setStatusIcon(toolDet, t.status);
          const pre = ensurePre(toolDet, "body");
          const text = t.detail || "";
          if (pre.textContent !== text) pre.textContent = text;
        }

	        for (const [k, el] of blockElByKey.entries()) {
	          if (!k.startsWith(id + ":tool:")) continue;
	          if (wantedKeys.has(k)) continue;
	          if (el.parentElement) el.parentElement.removeChild(el);
	          blockElByKey.delete(k);
	          delete detailsState[k];
	        }

	        placeTopLevel(det);
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
        placeTopLevel(det);
        continue;
      }

      if (block.type === "system") {
        const key = "b:" + block.id;
        if (block.title === "Other events (debug)") {
          const id = "sys:" + block.id;
          const det = ensureDetails(
            id,
            "notice debug",
            false,
            "Debug: " + block.title,
            id,
          );
          const pre = det.querySelector(`pre[data-k="body"]`);
          if (pre) pre.remove();
          const mdEl = ensureMd(det, "body");
          renderMarkdownInto(mdEl, block.text);
          placeTopLevel(det);
          continue;
        }

        if (block.title === "MCP startup issues") {
          const id = "mcpStartupIssues:" + block.id;
          const det = ensureDetails(
            id,
            "notice",
            false,
            "Notice: " + block.title,
            id,
          );
          const pre = det.querySelector(`pre[data-k="body"]`);
          if (pre) pre.remove();
          const mdEl = ensureMd(det, "body");
          renderMarkdownInto(mdEl, block.text);
          placeTopLevel(det);
          continue;
        }

        const div = ensureDiv(key, "msg system");
        const pre = div.querySelector(`pre[data-k="body"]`);
        if (pre) pre.remove();
        const mdEl = ensureMd(div, "body");
        renderMarkdownInto(mdEl, block.text);
        placeTopLevel(div);
        continue;
      }

      if (block.type === "info") {
        const key = "b:" + block.id;
        const div = ensureDiv(key, "msg info");
        const pre = div.querySelector(`pre[data-k="body"]`);
        if (pre) pre.remove();
        const mdEl = ensureMd(div, "body");
        renderMarkdownInto(mdEl, block.text);
        placeTopLevel(div);
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
        placeTopLevel(det);
        continue;
      }

      const _exhaustive: never = block;
      void _exhaustive;
    }

    pruneStaleSessionBlockEls(new Set((s.blocks || []).map((b) => b.id)));
    updateReturnToBottomVisibility();

    if (forceScrollToBottom || shouldAutoScroll) {
      logEl.scrollTop = logEl.scrollHeight;
      stickLogToBottom = true;
      updateReturnToBottomVisibility();
    }
  }

  function setEditMode(next: number | null, presetText?: string): void {
    const backendId = state.activeSession?.backendId ?? null;
    const canEdit = backendId === "codez" || backendId === "opencode";
    if (next !== null && !canEdit) {
      showToast(
        "info",
        "Edit/Rewind は codez または opencode セッションのみ対応です。",
      );
      return;
    }
    rewindTurnIndex = next;

    if (typeof presetText === "string") {
      inputEl.value = presetText;
      autosizeInput();
      updateSuggestions();
      saveComposerState();
    }

    if (rewindTurnIndex === null) {
      editBannerEl.style.display = "none";
      editBannerEl.replaceChildren();
      return;
    }

    editBannerEl.replaceChildren();
    const text = document.createElement("div");
    text.className = "editBannerText";
    text.textContent = `Editing turn #${rewindTurnIndex}. Sending will rewind and replace subsequent messages.`;
    const cancel = document.createElement("button");
    cancel.textContent = "Cancel";
    cancel.addEventListener("click", () => setEditMode(null));
    editBannerEl.appendChild(text);
    editBannerEl.appendChild(cancel);
    editBannerEl.style.display = "";
  }

  function sendCurrentInput(): void {
    if (!state.activeSession) return;
    if (state.sending) return;
    const backendId = state.activeSession.backendId;
    const canEdit = backendId === "codez" || backendId === "opencode";
    const text = inputEl.value;
    const trimmed = text.trim();
    if (!trimmed && pendingImages.length === 0) return;
    if (pendingImages.length > 0) {
      vscode.postMessage({
        type: "sendWithImages",
        text,
        images: pendingImages.map((img) => ({ name: img.name, url: img.url })),
        rewind:
          canEdit && rewindTurnIndex !== null
            ? { turnIndex: rewindTurnIndex }
            : null,
      });
      pendingImages.splice(0, pendingImages.length);
      renderAttachments();
    } else {
      vscode.postMessage({
        type: "send",
        text,
        rewind:
          canEdit && rewindTurnIndex !== null
            ? { turnIndex: rewindTurnIndex }
            : null,
      });
    }

    // Keep a simple history for navigating with Up/Down.
    const hist = ensureInputHistoryState(activeComposerKey);
    const last = hist.items.at(-1);
    if (last !== trimmed) hist.items.push(trimmed);
    hist.index = null;
    hist.draftBeforeHistory = "";

    inputEl.value = "";
    inputEl.setSelectionRange(0, 0);
    autosizeInput();
    updateSuggestions();
    saveComposerState();
    setEditMode(null);
  }

  function renderAttachments(): void {
    attachmentsEl.innerHTML = "";
    if (pendingImages.length === 0) {
      attachmentsEl.style.display = "none";
      saveComposerState();
      return;
    }
    attachmentsEl.style.display = "flex";
    for (const img of pendingImages) {
      const chip = document.createElement("div");
      chip.className = "attachmentChip";
      const thumb = document.createElement("img");
      thumb.className = "attachmentThumb";
      thumb.src = img.url;
      thumb.alt = img.name || "image";
      thumb.loading = "lazy";
      const name = document.createElement("span");
      name.className = "attachmentName";
      name.textContent = img.name;
      const remove = document.createElement("span");
      remove.className = "attachmentRemove";
      remove.textContent = "×";
      remove.title = "Remove";
      remove.addEventListener("click", () => {
        const idx = pendingImages.findIndex((p) => p.id === img.id);
        if (idx >= 0) pendingImages.splice(idx, 1);
        renderAttachments();
      });
      chip.appendChild(thumb);
      chip.appendChild(name);
      chip.appendChild(remove);
      attachmentsEl.appendChild(chip);
    }
    saveComposerState();
  }

  function readFileAsDataUrl(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onerror = () =>
        reject(reader.error ?? new Error("File read failed"));
      reader.onload = () => {
        if (typeof reader.result === "string") resolve(reader.result);
        else reject(new Error("Unexpected file read result"));
      };
      reader.readAsDataURL(file);
    });
  }

  function fileExtFromMime(mime: string): string {
    const m = String(mime || "").toLowerCase();
    if (m === "image/png") return "png";
    if (m === "image/jpeg") return "jpg";
    if (m === "image/gif") return "gif";
    if (m === "image/webp") return "webp";
    if (m === "image/bmp") return "bmp";
    if (m === "image/svg+xml") return "svg";
    if (m === "image/tiff") return "tiff";
    return "png";
  }

  async function attachImageFile(
    file: File,
    fallbackBaseName: string,
  ): Promise<void> {
    const url = await readFileAsDataUrl(file);
    const ext = fileExtFromMime(file.type);
    const rawName = String((file as any).name || "").trim();
    const name = rawName ? rawName : `${fallbackBaseName}.${ext}`;
    pendingImages.push({
      id: `${Date.now()}:${Math.random().toString(16).slice(2)}`,
      name,
      url,
    });
  }

  function stopCurrentTurn(): void {
    if (!state.activeSession) return;
    if (!state.sending) return;
    vscode.postMessage({ type: "stop" });
  }

  sendBtn.addEventListener("click", () =>
    state.sending ? stopCurrentTurn() : sendCurrentInput(),
  );

  // Only stop from Esc when the input itself is focused.
  // (Do not bind a global Esc handler; that would conflict with VS Code keybindings.)
  document.addEventListener(
    "keydown",
    (e) => {
      const ke = e as KeyboardEvent;
      if (ke.key !== "Escape") return;
      if (!state.sending) return;
      if (document.activeElement !== inputEl) return;
      e.preventDefault();
      e.stopPropagation();
      stopCurrentTurn();
    },
    true,
  );

  attachBtn.addEventListener("click", () => imageInput.click());
  imageInput.addEventListener("change", async () => {
    const files = Array.from(imageInput.files ?? []);
    imageInput.value = "";
    if (files.length === 0) return;
    for (const file of files) {
      try {
        const url = await readFileAsDataUrl(file);
        pendingImages.push({
          id: `${Date.now()}:${Math.random().toString(16).slice(2)}`,
          name: file.name || "image",
          url,
        });
      } catch (err) {
        vscode.postMessage({
          type: "uiError",
          message: `Failed to read image ${file.name || ""}: ${String(err)}`,
        });
      }
    }
    renderAttachments();
  });
  newBtn.addEventListener("click", () =>
    vscode.postMessage({ type: "newSessionPickFolder" }),
  );
  resumeBtn.addEventListener("click", () =>
    vscode.postMessage({ type: "resumeFromHistory" }),
  );
  reloadBtn.addEventListener("click", () =>
    state.reloading
      ? undefined
      : (() => {
          const backendId = state.activeSession?.backendId ?? null;
          if (backendId !== "codez") {
            showToast("info", "Reload は codez セッションのみ対応です。");
            return;
          }
          vscode.postMessage({ type: "reloadSession" });
        })(),
  );
  if (statusBtn) {
    statusBtn.addEventListener("click", () =>
      vscode.postMessage({ type: "showStatus" }),
    );
  }
  if (diffBtn) {
    diffBtn.addEventListener("click", () =>
      vscode.postMessage({ type: "openDiff" }),
    );
  }
  settingsBtn.addEventListener("click", () => void openSettings());
  settingsCloseBtn.addEventListener("click", () => closeSettings());
  settingsOverlayEl.addEventListener("click", (e) => {
    if (e.target === settingsOverlayEl) closeSettings();
  });
  document.addEventListener(
    "keydown",
    (e) => {
      if (!settingsOpen) return;
      if ((e as KeyboardEvent).key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      closeSettings();
    },
    true,
  );

  inputEl.addEventListener("input", () => updateSuggestions());
  inputEl.addEventListener("input", () => autosizeInput());
  inputEl.addEventListener("input", () => saveComposerState());
  inputEl.addEventListener("paste", async (e) => {
    try {
      const dt = e.clipboardData;
      if (!dt) return;
      const items = Array.from(dt.items || []);
      const imageItems = items.filter(
        (it) =>
          it.kind === "file" && String(it.type || "").startsWith("image/"),
      );
      if (imageItems.length === 0) return;
      if (!state.activeSession) return;

      for (let i = 0; i < imageItems.length; i++) {
        const it = imageItems[i]!;
        const file = it.getAsFile();
        if (!file) continue;
        const base = `pasted-image-${Date.now()}-${i + 1}`;
        await attachImageFile(file, base);
      }
      renderAttachments();
    } catch (err) {
      vscode.postMessage({
        type: "uiError",
        message: `Failed to paste image: ${String(err)}`,
      });
    }
  });
  inputEl.addEventListener("click", () => {
    updateSuggestions();
    saveComposerState();
  });
  inputEl.addEventListener("keyup", (e) => {
    const key = (e as KeyboardEvent).key;
    if ((key === "ArrowDown" || key === "ArrowUp") && suggestItems.length > 0)
      return;
    updateSuggestions();
  });
  inputEl.addEventListener("compositionstart", () => {
    isComposing = true;
  });
  inputEl.addEventListener("compositionend", () => {
    isComposing = false;
  });
  // NOTE: On some platforms/IME flows, `compositionend` may not fire if the
  // textarea loses focus mid-composition. If we keep `isComposing=true`, Enter
  // will never send. Reset on blur to avoid a stuck composer.
  inputEl.addEventListener("blur", () => {
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
      const hist = ensureInputHistoryState(inputHistoryKeyForActiveComposer());
      if (hist.items.length === 0) return;
      e.preventDefault();

      if (hist.index === null) {
        hist.draftBeforeHistory = inputEl.value;
        hist.index = hist.items.length - 1;
      } else {
        hist.index = Math.max(0, hist.index - 1);
      }

      inputEl.value = hist.items[hist.index] || "";
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

      const hist = ensureInputHistoryState(inputHistoryKeyForActiveComposer());
      if (hist.index === null) return;
      e.preventDefault();

      hist.index += 1;
      if (hist.index >= hist.items.length) {
        hist.index = null;
        inputEl.value = hist.draftBeforeHistory;
        hist.draftBeforeHistory = "";
      } else {
        inputEl.value = hist.items[hist.index] || "";
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

  function appendDeltaToPre(pre: HTMLPreElement, delta: string): void {
    // Avoid creating one Text node per delta (can eventually freeze the webview).
    const last = pre.lastChild;
    if (last && last.nodeType === Node.TEXT_NODE) {
      (last as Text).appendData(delta);
      return;
    }
    pre.append(document.createTextNode(delta));
  }

  window.addEventListener("message", (event: MessageEvent) => {
    const msg = event.data;
    if (!msg || typeof msg !== "object") return;
    const anyMsg = msg as {
      type?: unknown;
      seq?: unknown;
      state?: unknown;
      blocks?: unknown;
      block?: unknown;
      blockId?: unknown;
      field?: unknown;
      delta?: unknown;
      streaming?: unknown;
      paths?: unknown;
      sessionId?: unknown;
      query?: unknown;
      agents?: unknown;
      skills?: unknown;
      text?: unknown;
      kind?: unknown;
      message?: unknown;
      timeoutMs?: unknown;
      requestId?: unknown;
      ok?: unknown;
      mimeType?: unknown;
      base64?: unknown;
      error?: unknown;
      data?: unknown;
    };
    if (anyMsg.type === "toast") {
      const kind =
        anyMsg.kind === "success" || anyMsg.kind === "error"
          ? anyMsg.kind
          : "info";
      const message = typeof anyMsg.message === "string" ? anyMsg.message : "";
      if (!message) return;
      const timeoutMs =
        typeof anyMsg.timeoutMs === "number" &&
        Number.isFinite(anyMsg.timeoutMs)
          ? Math.max(0, Math.trunc(anyMsg.timeoutMs))
          : 2500;
      showToast(kind, message, timeoutMs);
      return;
    }
    if (anyMsg.type === "accountLoginCompleted") {
      const success = Boolean((anyMsg as any).success);
      const error =
        typeof (anyMsg as any).error === "string"
          ? String((anyMsg as any).error)
          : null;
      settingsLoginInFlight = null;
      if (settingsOpen) {
        if (success) {
          showToast("success", "Login completed.");
        } else {
          showToast(
            "error",
            error ? `Login failed: ${error}` : "Login failed.",
          );
        }
        void loadSettings();
      }
      return;
    }
    if (anyMsg.type === "settingsResponse") {
      const requestId =
        typeof anyMsg.requestId === "string" ? anyMsg.requestId : null;
      if (!requestId) return;
      const pending = pendingSettingsRequestsById.get(requestId);
      if (!pending) return;
      pendingSettingsRequestsById.delete(requestId);
      if (anyMsg.ok) {
        pending.resolve({ ok: true, data: (anyMsg as any).data });
      } else {
        pending.resolve({
          ok: false,
          error:
            typeof anyMsg.error === "string" ? anyMsg.error : "Unknown error",
        });
      }
      return;
    }
    if (anyMsg.type === "imageData") {
      const requestId =
        typeof anyMsg.requestId === "string" ? anyMsg.requestId : null;
      if (!requestId) return;
      const pending = pendingImageRequestsById.get(requestId);
      if (!pending) return;
      pendingImageRequestsById.delete(requestId);
      if (anyMsg.ok) {
        pending.resolve({
          ok: true,
          imageKey: pending.imageKey,
          mimeType: typeof anyMsg.mimeType === "string" ? anyMsg.mimeType : "",
          base64: typeof anyMsg.base64 === "string" ? anyMsg.base64 : "",
        });
      } else {
        pending.resolve({
          ok: false,
          imageKey: pending.imageKey,
          error:
            typeof anyMsg.error === "string" ? anyMsg.error : "Unknown error",
        });
      }
      return;
    }
    if (anyMsg.type === "state") {
      receivedState = true;
      const seq = typeof anyMsg.seq === "number" ? anyMsg.seq : null;
      state = anyMsg.state as ChatViewState;
      pendingState = state;
      pendingStateSeq = seq;
      scheduleRender();
      return;
    }
    if (anyMsg.type === "controlState") {
      receivedState = true;
      const seq = typeof anyMsg.seq === "number" ? anyMsg.seq : null;
      const next = (anyMsg.state as Partial<ChatViewState>) ?? {};
      const prevBlocks = state.blocks;
      state = {
        ...(state as any),
        ...(next as any),
        blocks: prevBlocks,
      } as ChatViewState;

      const activeId = state.activeSession?.id ?? null;
      if (activeId) {
        const pendingBlocks = pendingBlocksBySessionId.get(activeId);
        if (pendingBlocks) {
          pendingBlocksBySessionId.delete(activeId);
          blocksBySessionId.set(activeId, pendingBlocks);
          touchBlockCache(activeId);
          state = { ...(state as any), blocks: pendingBlocks } as ChatViewState;
          pendingBlocksState = state;
          scheduleBlocksRender();
        } else {
          const cachedBlocks = blocksBySessionId.get(activeId);
          if (cachedBlocks) {
            touchBlockCache(activeId);
            state = {
              ...(state as any),
              blocks: cachedBlocks,
            } as ChatViewState;
            pendingBlocksState = state;
            scheduleBlocksRender();
          }
        }
      }
      pendingControlState = state;
      pendingControlSeq = seq;
      scheduleControlRender();

      if (settingsOpen) {
        const activeIdNow = state.activeSession?.id ?? null;
        if (activeIdNow !== settingsLastActiveSessionId) {
          settingsLastActiveSessionId = activeIdNow;
          void loadSettings();
        }
      }
      return;
    }
    if (anyMsg.type === "blocksReset") {
      const sessionId =
        typeof anyMsg.sessionId === "string" ? anyMsg.sessionId : null;
      if (!sessionId) return;
      const blocks = Array.isArray(anyMsg.blocks)
        ? (anyMsg.blocks as ChatBlock[])
        : [];
      touchBlockCache(sessionId);
      blocksBySessionId.set(sessionId, blocks);
      if (!state.activeSession || state.activeSession.id !== sessionId) {
        pendingBlocksBySessionId.set(sessionId, blocks);
        touchBlockCache(sessionId);
        return;
      }
      state = { ...(state as any), blocks } as ChatViewState;
      pendingBlocksState = state;
      scheduleBlocksRender();
      return;
    }
    if (anyMsg.type === "askUserQuestionStart") {
      const requestKey =
        typeof (anyMsg as any).requestKey === "string"
          ? (anyMsg as any).requestKey
          : null;
      if (!requestKey) return;
      const request = (anyMsg as any).request as AskUserQuestionRequest;
      if (!request || typeof request !== "object") return;

      askUserQuestionState = {
        requestKey,
        request,
        index: 0,
        answers: {},
        otherTextById: {},
      };
      askUserQuestionError = null;
      renderAskUserQuestion();
      return;
    }
    if (anyMsg.type === "blockUpsert") {
      const sessionId =
        typeof anyMsg.sessionId === "string" ? anyMsg.sessionId : null;
      if (!sessionId) return;
      if (!state.activeSession || state.activeSession.id !== sessionId) return;
      const block = anyMsg.block as ChatBlock;
      if (!block || typeof (block as any).id !== "string") return;
      const blocks = state.blocks || [];
      const idx = blocks.findIndex((b) => b && b.id === (block as any).id);
      if (idx >= 0) blocks[idx] = block;
      else blocks.push(block);
      blocksBySessionId.set(sessionId, blocks);
      touchBlockCache(sessionId);
      state = { ...(state as any), blocks } as ChatViewState;
      pendingBlocksState = state;
      scheduleBlocksRender();
      return;
    }
    if (anyMsg.type === "blockAppend") {
      const sessionId =
        typeof anyMsg.sessionId === "string" ? anyMsg.sessionId : null;
      const blockId =
        typeof anyMsg.blockId === "string" ? anyMsg.blockId : null;
      const field = typeof anyMsg.field === "string" ? anyMsg.field : null;
      const delta = typeof anyMsg.delta === "string" ? anyMsg.delta : null;
      const streaming =
        typeof anyMsg.streaming === "boolean" ? anyMsg.streaming : null;
      if (!sessionId || !blockId || !field || delta === null) return;
      if (!state.activeSession || state.activeSession.id !== sessionId) return;

      const b = (state.blocks || []).find((x) => x && x.id === blockId) as
        | ChatBlock
        | undefined;
      if (!b) return;

      if (field === "assistantText" && b.type === "assistant") {
        b.text += delta;
        if (streaming !== null) (b as any).streaming = streaming;
        blocksBySessionId.set(sessionId, state.blocks || []);
        touchBlockCache(sessionId);

        // Fast path: update the visible <pre> without a full render.
        const key = "b:" + blockId;
        const div = blockElByKey.get(key);
        if (div) {
          const pre = div.querySelector(
            `pre[data-k="body"]`,
          ) as HTMLPreElement | null;
          if (pre) {
            // If streaming has ended, force a full re-render so we switch from
            // <pre> to Markdown-rendered HTML.
            if (streaming !== false) {
              appendDeltaToPre(pre, delta);
              return;
            }
          }
        }

        pendingBlocksState = state;
        scheduleBlocksRender();
        return;
      }
      if (field === "commandOutput" && b.type === "command") {
        b.output += delta;
        blocksBySessionId.set(sessionId, state.blocks || []);
        touchBlockCache(sessionId);

        const id = "command:" + blockId;
        const det = blockElByKey.get(id);
        if (det && det.tagName.toLowerCase() === "details") {
          const pre = (det as HTMLElement).querySelector(
            `pre[data-k="body"]`,
          ) as HTMLPreElement | null;
          if (pre) {
            appendDeltaToPre(pre, delta);
            return;
          }
        }

        pendingBlocksState = state;
        scheduleBlocksRender();
        return;
      }
      if (field === "fileChangeDetail" && b.type === "fileChange") {
        b.detail += delta;
        blocksBySessionId.set(sessionId, state.blocks || []);
        touchBlockCache(sessionId);

        const id = "fileChange:" + blockId;
        const det = blockElByKey.get(id);
        if (det && det.tagName.toLowerCase() === "details") {
          const pre = (det as HTMLElement).querySelector(
            `pre[data-k="detail"]`,
          ) as HTMLPreElement | null;
          if (pre) {
            appendDeltaToPre(pre, delta);
            return;
          }
        }

        pendingBlocksState = state;
        scheduleBlocksRender();
        return;
      }
      return;
    }
    if (anyMsg.type === "fileSearchResult") {
      const sessionId =
        typeof anyMsg.sessionId === "string" ? anyMsg.sessionId : null;
      const query = typeof anyMsg.query === "string" ? anyMsg.query : null;
      const paths = Array.isArray(anyMsg.paths)
        ? anyMsg.paths.filter((p): p is string => typeof p === "string")
        : [];

      if (!sessionId || !query) return;
      if (!state.activeSession) return;
      if (state.activeSession.id !== sessionId) return;

      // Ignore stale results.
      const inFlight = fileSearchInFlight;
      if (
        !inFlight ||
        inFlight.sessionId !== sessionId ||
        inFlight.query !== query
      )
        return;

      fileSearch = { sessionId, query, paths };
      fileSearchInFlight = null;
      updateSuggestions();
      return;
    }
    if (anyMsg.type === "agentIndex") {
      const agents = Array.isArray(anyMsg.agents)
        ? anyMsg.agents.filter((a): a is string => typeof a === "string")
        : [];
      if (state.activeSession) {
        agentIndex = agents;
        agentIndexForSessionId = state.activeSession.id;
      } else {
        agentIndex = null;
        agentIndexForSessionId = null;
      }
      renderSuggest();
      return;
    }
    if (anyMsg.type === "skillIndex") {
      const sessionId =
        typeof anyMsg.sessionId === "string" ? anyMsg.sessionId : null;
      const rawSkills = Array.isArray(anyMsg.skills) ? anyMsg.skills : [];
      if (!sessionId) return;
      if (!state.activeSession) return;
      if (state.activeSession.id !== sessionId) return;

      const skills = rawSkills
        .map(
          (
            s,
          ): null | {
            name: string;
            description: string | null;
            scope: string;
            path: string;
          } => {
            if (!s || typeof s !== "object") return null;
            const o = s as Record<string, unknown>;
            const name = typeof o.name === "string" ? o.name : "";
            const scope = typeof o.scope === "string" ? o.scope : "";
            const path = typeof o.path === "string" ? o.path : "";
            const description =
              typeof o.description === "string" ? o.description : null;
            if (!name || !scope || !path) return null;
            return { name, description, scope, path };
          },
        )
        .filter((v): v is NonNullable<typeof v> => v !== null);

      skillIndex = skills;
      skillIndexForSessionId = sessionId;
      updateSuggestions();
      return;
    }
    if (anyMsg.type === "insertText") {
      const text = typeof anyMsg.text === "string" ? anyMsg.text : "";
      if (text) {
        const start = inputEl.selectionStart ?? inputEl.value.length;
        const end = inputEl.selectionEnd ?? inputEl.value.length;
        const before = inputEl.value.slice(0, start);
        const after = inputEl.value.slice(end);
        inputEl.value = before + text + after;
        const nextPos = start + text.length;
        inputEl.focus();
        inputEl.setSelectionRange(nextPos, nextPos);
        autosizeInput();
        updateSuggestions();
      }
      return;
    }
  });

  function scheduleFileSearch(sessionId: string, query: string): void {
    // Debounce and allow only one in-flight search, similar to TUI behavior.
    if (fileSearchTimer != null) {
      window.clearTimeout(fileSearchTimer);
      fileSearchTimer = null;
    }

    const existing =
      fileSearch &&
      fileSearch.sessionId === sessionId &&
      fileSearch.query === query
        ? fileSearch
        : null;
    if (existing) return;

    // If there is an in-flight query that's no longer a prefix of what the user typed,
    // mark it stale; the response will be ignored.
    if (fileSearchInFlight && !query.startsWith(fileSearchInFlight.query)) {
      fileSearchInFlight = null;
    }

    fileSearchTimer = window.setTimeout(() => {
      if (!state.activeSession || state.activeSession.id !== sessionId) return;
      fileSearchInFlight = { sessionId, query };
      vscode.postMessage({ type: "requestFileSearch", sessionId, query });
      fileSearchTimer = null;
    }, FILE_SEARCH_DEBOUNCE_MS);
  }

  function rankFilePaths(paths: string[], query: string): string[] {
    const q = query.toLowerCase();
    return paths
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
  }

  function rankDirPrefixes(paths: string[], query: string): string[] {
    const qRaw = query.replace(/\\/g, "/");
    const q = qRaw.toLowerCase();

    // When the query includes a '/', treat it as a path prefix and suggest the
    // next directory segment under that base. This enables drill-down after
    // selecting a directory (e.g. "@src/" -> "@src/foo/").
    if (qRaw.includes("/")) {
      const lastSlash = qRaw.lastIndexOf("/");
      const base = qRaw.endsWith("/") ? qRaw : qRaw.slice(0, lastSlash + 1);
      const leaf = qRaw.endsWith("/") ? "" : qRaw.slice(lastSlash + 1);
      const leafLower = leaf.toLowerCase();

      const dirs = new Set<string>();
      for (const p of paths) {
        if (!p.startsWith(base)) continue;
        const rest = p.slice(base.length);
        const seg = rest.split("/", 1)[0] || "";
        if (!seg) continue;
        if (leafLower && !seg.toLowerCase().startsWith(leafLower)) continue;
        // Only suggest directories when there is deeper content.
        if (!rest.includes("/")) continue;
        dirs.add(base + seg + "/");
      }
      return [...dirs]
        .map((p) => {
          const pl = p.toLowerCase();
          const depth = (p.match(/\//g) || []).length;
          const score =
            leafLower && pl.startsWith(q)
              ? 0
              : leafLower && pl.includes("/" + leafLower)
                ? 1
                : 2;
          return { p, score, depth, len: p.length };
        })
        .sort(
          (a, b) =>
            a.score - b.score ||
            a.depth - b.depth ||
            a.len - b.len ||
            a.p.localeCompare(b.p),
        )
        .slice(0, 30)
        .map((x) => x.p);
    }

    // No slash: suggest directories derived from matches (parent dir of each file).
    const dirs = new Set<string>();
    for (const p of paths) {
      const idx = p.lastIndexOf("/");
      if (idx < 0) continue;
      const dir = p.slice(0, idx + 1);
      if (dir) dirs.add(dir);
    }

    return [...dirs]
      .map((p) => {
        const pl = p.toLowerCase();
        const depth = (p.match(/\//g) || []).length;
        const score = q
          ? pl.startsWith(q)
            ? 0
            : pl.includes("/" + q)
              ? 1
              : 2
          : 0;
        return { p, score, depth, len: p.length };
      })
      .sort(
        (a, b) =>
          a.score - b.score ||
          a.depth - b.depth ||
          a.len - b.len ||
          a.p.localeCompare(b.p),
      )
      .slice(0, 30)
      .map((x) => x.p);
  }

  // Open links via the extension host.
  document.addEventListener("click", (e) => {
    const t = eventTargetEl(e.target);
    const me = e as MouseEvent;

    const urlLink = t
      ? (t.closest("[data-open-url]") as HTMLElement | null)
      : null;
    if (urlLink) {
      const url = urlLink.getAttribute("data-open-url") || "";
      if (url && (me.ctrlKey || me.metaKey)) {
        e.preventDefault();
        vscode.postMessage({ type: "openExternal", url });
        return;
      }
    }

    const fileLink = t
      ? (t.closest("[data-open-file]") as HTMLElement | null)
      : null;
    if (fileLink) {
      const file = fileLink.getAttribute("data-open-file") || "";
      const cwd = (
        fileLink.closest("[data-cwd]") as HTMLElement | null
      )?.getAttribute("data-cwd");
      if (file && (me.ctrlKey || me.metaKey)) {
        e.preventDefault();
        vscode.postMessage({ type: "openFile", path: file, cwd: cwd || null });
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

    // If the link is explicitly a file reference, treat it as such even if it
    // looks like it has a URI scheme (e.g. "README.md:10" would otherwise be
    // misclassified as scheme="readme.md").
    const explicitFileRefRe = new RegExp(
      String.raw`^(?:\.{0,2}\/)?[\p{L}\p{N}\p{M}_@.+-]+(?:\/[\p{L}\p{N}\p{M}_@.+-]+)*\.[A-Za-z0-9]{1,8}(?:(?::\d+(?::\d+)?)|(?:#L\d+(?:C\d+)?))?$`,
      "u",
    );
    if (explicitFileRefRe.test(decoded)) {
      const normalized = decoded.replace(/^\/+/, "");
      const cwd = (a.closest("[data-cwd]") as HTMLElement | null)?.getAttribute(
        "data-cwd",
      );
      e.preventDefault();
      vscode.postMessage({
        type: "openFile",
        path: normalized,
        cwd: cwd || null,
      });
      return;
    }

    const schemeMatch = decoded.match(/^([a-zA-Z][a-zA-Z0-9+.-]*):/);
    const scheme = schemeMatch ? schemeMatch[1]?.toLowerCase() : null;
    if (scheme) {
      if (scheme === "file") {
        const without = decoded.replace(/^file:(\/\/)?/, "");
        const normalized = without.replace(/^\/+/, "");
        const cwd = (
          a.closest("[data-cwd]") as HTMLElement | null
        )?.getAttribute("data-cwd");
        e.preventDefault();
        vscode.postMessage({
          type: "openFile",
          path: normalized,
          cwd: cwd || null,
        });
        return;
      }
      // Unknown/unsupported schemes are delegated to VS Code's openExternal.
      e.preventDefault();
      vscode.postMessage({ type: "openExternal", url: decoded });
      return;
    }

    // Treat "/path" as workspace-root relative (GitHub-style links).
    const normalized = decoded.replace(/^\/+/, "");
    const cwd = (a.closest("[data-cwd]") as HTMLElement | null)?.getAttribute(
      "data-cwd",
    );
    e.preventDefault();
    vscode.postMessage({
      type: "openFile",
      path: normalized,
      cwd: cwd || null,
    });
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

function workspaceOverridesSig(overrides: Record<string, number>): string {
  return Object.entries(overrides)
    .map(([k, v]) => `${k}\t${v}`)
    .sort((a, b) => a.localeCompare(b))
    .join("\n");
}

function workspaceTagFromUri(
  workspaceFolderUri: string,
  overrides: Record<string, number>,
): {
  label: string;
  color: string;
} {
  const override = overrides[workspaceFolderUri];
  const idx =
    typeof override === "number"
      ? Math.trunc(override)
      : fnv1a32(workspaceFolderUri) % WORKTREE_COLORS.length;

  if (idx < 0 || idx >= WORKTREE_COLORS.length) {
    throw new Error(`workspace color index out of range (idx=${idx})`);
  }

  const color = WORKTREE_COLORS[idx];
  if (!color) throw new Error(`workspace color missing (idx=${idx})`);
  const label = uriToBasename(workspaceFolderUri);
  return { label, color };
}

function uriToHashKey(uri: string): string {
  try {
    const u = new URL(uri);
    return decodeURIComponent(u.pathname || uri);
  } catch {
    return uri;
  }
}

function uriToBasename(uri: string): string {
  const key = uriToHashKey(uri);
  const parts = key.split("/").filter((p) => p.length > 0);
  return parts[parts.length - 1] ?? key;
}

function fnv1a32(input: string): number {
  let hash = 0x811c9dc5;
  for (let i = 0; i < input.length; i++) {
    hash ^= input.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
    hash >>>= 0;
  }
  return hash >>> 0;
}

main();
