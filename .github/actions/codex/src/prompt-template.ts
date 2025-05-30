/*
 * Utilities to render Codex prompt templates.
 *
 * A template is a Markdown (or plain-text) file that may contain one or more
 * placeholders of the form `{CODEX_ACTION_<NAME>}`. At runtime these
 * placeholders are substituted with dynamically generated content. Each
 * placeholder is resolved **exactly once** even if it appears multiple times
 * in the same template.
 */

import { readFile } from "fs/promises";

import { EnvContext } from "./env-context";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Lazily caches parsed `$GITHUB_EVENT_PATH` contents keyed by the file path so
 * we only hit the filesystem once per unique event payload.
 */
const githubEventDataCache: Map<string, Promise<any>> = new Map();

function getGitHubEventData(ctx: EnvContext): Promise<any> {
  const eventPath = ctx.get("GITHUB_EVENT_PATH");
  let cached = githubEventDataCache.get(eventPath);
  if (!cached) {
    cached = readFile(eventPath, "utf8").then((raw) => JSON.parse(raw));
    githubEventDataCache.set(eventPath, cached);
  }
  return cached;
}

async function runCommand(args: Array<string>): Promise<string> {
  const result = Bun.spawnSync(args, {
    stdout: "pipe",
    stderr: "pipe",
  });

  if (result.success) {
    return result.stdout.toString();
  }

  console.error(`Error running ${JSON.stringify(args)}: ${result.stderr}`);
  return "";
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

// Regex that captures the variable name without the surrounding { } braces.
const VAR_REGEX = /\{(CODEX_ACTION_[A-Z0-9_]+)\}/g;

// Cache individual placeholder values so each one is resolved at most once per
// process even if many templates reference it.
const placeholderCache: Map<string, Promise<string>> = new Map();

/**
 * Parse a template string, resolve all placeholders and return the rendered
 * result.
 */
export async function renderPromptTemplate(
  template: string,
  ctx: EnvContext,
): Promise<string> {
  // ---------------------------------------------------------------------
  // 1) Gather all *unique* placeholders present in the template.
  // ---------------------------------------------------------------------
  const variables = new Set<string>();
  for (const match of template.matchAll(VAR_REGEX)) {
    variables.add(match[1]);
  }

  // ---------------------------------------------------------------------
  // 2) Kick off (or reuse) async resolution for each variable.
  // ---------------------------------------------------------------------
  for (const variable of variables) {
    if (!placeholderCache.has(variable)) {
      placeholderCache.set(variable, resolveVariable(variable, ctx));
    }
  }

  // ---------------------------------------------------------------------
  // 3) Await completion so we can perform a simple synchronous replace below.
  // ---------------------------------------------------------------------
  const resolvedEntries: [string, string][] = [];
  for (const [key, promise] of placeholderCache.entries()) {
    resolvedEntries.push([key, await promise]);
  }
  const resolvedMap = new Map<string, string>(resolvedEntries);

  // ---------------------------------------------------------------------
  // 4) Replace each occurrence.  We use replace with a callback to ensure
  //    correct substitution even if variable names overlap (they shouldn't,
  //    but better safe than sorry).
  // ---------------------------------------------------------------------
  return template.replace(VAR_REGEX, (_, varName: string) => {
    return resolvedMap.get(varName) ?? "";
  });
}

export async function ensureBaseAndHeadCommitsForPRAreAvailable(
  ctx: EnvContext,
): Promise<{ baseSha: string; headSha: string } | null> {
  const prShas = await getPrShas(ctx);
  if (prShas == null) {
    console.warn("Unable to resolve PR branches");
    return null;
  }

  const event = await getGitHubEventData(ctx);
  const pr = event.pull_request;
  if (!pr) {
    console.warn("event.pull_request is not defined - unexpected");
    return null;
  }

  const workspace = ctx.get("GITHUB_WORKSPACE");

  // Refs (branch names)
  const baseRef: string | undefined = pr.base?.ref;
  const headRef: string | undefined = pr.head?.ref;

  // Clone URLs
  const baseRemoteUrl: string | undefined = pr.base?.repo?.clone_url;
  const headRemoteUrl: string | undefined = pr.head?.repo?.clone_url;

  if (!baseRef || !headRef || !baseRemoteUrl || !headRemoteUrl) {
    console.warn(
      "Missing PR ref or remote URL information - cannot fetch commits",
    );
    return null;
  }

  // Ensure we have the base branch.
  await runCommand([
    "git",
    "-C",
    workspace,
    "fetch",
    "--no-tags",
    "origin",
    baseRef,
  ]);

  // Ensure we have the head branch.
  if (headRemoteUrl === baseRemoteUrl) {
    // Same repository – the commit is available from `origin`.
    await runCommand([
      "git",
      "-C",
      workspace,
      "fetch",
      "--no-tags",
      "origin",
      headRef,
    ]);
  } else {
    // Fork – make sure a `pr` remote exists that points at the fork. Attempting
    // to add a remote that already exists causes git to error, so we swallow
    // any non-zero exit codes from that specific command.
    await runCommand([
      "git",
      "-C",
      workspace,
      "remote",
      "add",
      "pr",
      headRemoteUrl,
    ]);

    // Whether adding succeeded or the remote already existed, attempt to fetch
    // the head ref from the `pr` remote.
    await runCommand([
      "git",
      "-C",
      workspace,
      "fetch",
      "--no-tags",
      "pr",
      headRef,
    ]);
  }

  return prShas;
}

// ---------------------------------------------------------------------------
// Internal helpers – still exported for use by other modules.
// ---------------------------------------------------------------------------

export async function resolvePrDiff(ctx: EnvContext): Promise<string> {
  const prShas = await ensureBaseAndHeadCommitsForPRAreAvailable(ctx);
  if (prShas == null) {
    console.warn("Unable to resolve PR branches");
    return "";
  }

  const workspace = ctx.get("GITHUB_WORKSPACE");
  const { baseSha, headSha } = prShas;
  return runCommand([
    "git",
    "-C",
    workspace,
    "diff",
    "--color=never",
    `${baseSha}..${headSha}`,
  ]);
}

// ---------------------------------------------------------------------------
// Placeholder resolution
// ---------------------------------------------------------------------------

async function resolveVariable(name: string, ctx: EnvContext): Promise<string> {
  switch (name) {
    case "CODEX_ACTION_ISSUE_TITLE": {
      const event = await getGitHubEventData(ctx);
      const issue = event.issue ?? event.pull_request;
      return issue?.title ?? "";
    }

    case "CODEX_ACTION_ISSUE_BODY": {
      const event = await getGitHubEventData(ctx);
      const issue = event.issue ?? event.pull_request;
      return issue?.body ?? "";
    }

    case "CODEX_ACTION_GITHUB_EVENT_PATH": {
      return ctx.get("GITHUB_EVENT_PATH");
    }

    case "CODEX_ACTION_BASE_REF": {
      const event = await getGitHubEventData(ctx);
      return event?.pull_request?.base?.ref ?? "";
    }

    case "CODEX_ACTION_HEAD_REF": {
      const event = await getGitHubEventData(ctx);
      return event?.pull_request?.head?.ref ?? "";
    }

    case "CODEX_ACTION_PR_DIFF": {
      return resolvePrDiff(ctx);
    }

    // -------------------------------------------------------------------
    // Add new template variables here.
    // -------------------------------------------------------------------

    default: {
      // Unknown variable – leave it blank to avoid leaking placeholders to the
      // final prompt.  The alternative would be to `fail()` here, but silently
      // ignoring unknown placeholders is more forgiving and better matches the
      // behaviour of typical template engines.
      console.warn(`Unknown template variable: ${name}`);
      return "";
    }
  }
}

async function getPrShas(
  ctx: EnvContext,
): Promise<{ baseSha: string; headSha: string } | null> {
  const event = await getGitHubEventData(ctx);
  const pr = event.pull_request;
  if (!pr) {
    console.warn("event.pull_request is not defined");
    return null;
  }

  // Prefer explicit SHAs if available to avoid relying on local branch names.
  const baseSha: string | undefined = pr.base?.sha;
  const headSha: string | undefined = pr.head?.sha;

  if (!baseSha || !headSha) {
    console.warn("one of base or head is not defined on event.pull_request");
    return null;
  }

  return { baseSha, headSha };
}
