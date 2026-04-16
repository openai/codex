# Browser System Instructions Injection

## Problem

The browser harness can run a real Codex turn, but the `wasm32` build does not
load project docs from `AGENTS.md` today. In the browser, the application often
knows important environment-specific policy that the harness does not know:

- code runs inside AppKernel;
- browser APIs such as OPFS and network access are available;
- only a specific OPFS path layout is allowed;
- only specific GitHub URL prefixes are allowed; and
- the agent is expected to fetch, cache, and search source code itself.

Those details need to reach the model as prompt-layer instructions even when the
harness cannot discover them from the filesystem.

## Goals

- Let browser applications inject session-scoped prompt instructions.
- Keep prompt roles distinct:
  - base instructions remain the base/system layer;
  - developer instructions carry application or runtime policy;
  - user instructions carry AGENTS/project-doc style content.
- Keep responsibility boundaries clean:
  - the application decides where instructions come from;
  - the harness only accepts and applies resolved instruction text.

## Non-Goals

- Teach the wasm harness to discover `AGENTS.md` on its own.
- Move application-specific policy such as AppKernel, OPFS layout, or GitHub
  allowlists into `codex-core`.
- Replace Codex's built-in base prompt.

## Existing State

`codex-core` already has the right internal prompt channels:

- `Config.base_instructions`
- `Config.developer_instructions`
- `Config.user_instructions`

The browser adapter does not expose them. `BrowserCodex` currently accepts an
API key and a code executor, then builds a session config internally with a
fixed cwd. The wasm project-doc loader is also stubbed out, so no `AGENTS.md`
content is discovered automatically.

## Proposed API

Add a browser-session configuration object that the application can set before
starting a turn:

```ts
type BrowserSessionOptions = {
  cwd?: string;
  instructions?: {
    base?: string;
    developer?: string;
    user?: string;
  };
};
```

Expose it through the browser harness as a session-scoped setter:

```ts
const codex = new BrowserCodex(apiKey);
codex.setSessionOptions({
  cwd: "/workspace",
  instructions: {
    developer: "...AppKernel/OPFS/network/GitHub policy...",
    user: "...resolved AGENTS.md text...",
  },
});
```

## Prompt Layer Mapping

### `instructions.developer`

Use for environment and policy details supplied by the browser application, for
example:

- code runs inside AppKernel;
- OPFS and network APIs are available;
- approved OPFS path layout;
- approved GitHub URL prefixes; and
- fetch/cache/search expectations.

This maps to `Config.developer_instructions`.

### `instructions.user`

Use for application-resolved project docs such as `AGENTS.md`.

This maps to `Config.user_instructions`.

### `instructions.base`

Use only as an escape hatch when the embedding application intentionally wants
to replace the base instruction bundle for the session.

This maps to `Config.base_instructions`.

Most callers should leave this unset so Codex keeps its built-in base prompt.

## Session Lifetime

These values are session-scoped, not turn-scoped.

If `cwd` or any injected instruction changes, the harness should discard the
current browser session and create a new one on the next turn. This avoids
mixing old prompt state with new environment policy.

## API Boundary

### Application responsibilities

- discover `AGENTS.md` or any equivalent project-doc source;
- decide the approved OPFS layout;
- decide the approved GitHub URL prefixes;
- decide whether AppKernel/network guidance should be present; and
- assemble those values into prompt text.

### Harness responsibilities

- accept already-resolved session options;
- validate and store them;
- apply them to the Codex session config;
- ensure the configured cwd is used consistently for the session and for turn
  submission; and
- recreate the session when session options change.

## Implementation Notes

- Keep the API additive. Existing callers that only set the API key and code
  executor should continue to work.
- Reject unknown fields in the session-options payload so applications do not
  silently think a field is applied when it is ignored.
- Apply the configured cwd both when building the browser config and when
  calling `Op::UserTurn`.

## Initial Scope

Implement the new `BrowserCodex` session-options setter in `wasm-harness`,
apply it to config creation, and update the browser demo to act like an
embedding application by supplying explicit developer instructions for the
browser/AppKernel environment.
