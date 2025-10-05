# Codex SDK

Bring the power of the best coding agent to your application.

The TypeScript SDK is a light wrapper around the bundled `codex` binary. Internally it spawns the CLI and exchanges JSONL events over stdin/stdout, so anything you can do in the Codex CLI can also be driven from Node.js.

## Installation

```bash
npm install @openai/codex-sdk
```

Requires Node.js 18+ (uses `node:` stdlib modules and async generators).

## Quick start

```typescript
import { Codex } from "@openai/codex-sdk";

const codex = new Codex();
const thread = codex.startThread();
const turn = await thread.run("Diagnose the test failure and propose a fix");

console.log(turn.finalResponse);
console.log(turn.items);
```

Call `run()` repeatedly on the same `Thread` instance to continue that conversation.

```typescript
const nextTurn = await thread.run("Implement the fix");
```

### Streaming responses

`run()` buffers events until the turn finishes. To react to intermediate progress—tool calls, streaming responses, file diffs—use `runStreamed()`, which returns an async generator of structured events.

```typescript
const { events } = await thread.runStreamed("Diagnose the test failure and propose a fix");

for await (const event of events) {
  switch (event.type) {
    case "item.completed":
      console.log("item", event.item);
      break;
    case "turn.completed":
      console.log("usage", event.usage);
      break;
  }
}
```

### Resuming an existing thread

Threads are persisted in `~/.codex/sessions`. If you lose the in-memory `Thread` object, reconstruct it with `resumeThread()` and keep going.

```typescript
const savedThreadId = process.env.CODEX_THREAD_ID!;
const thread = codex.resumeThread(savedThreadId);
await thread.run("Implement the fix");
```

### Working directory controls

Codex runs in the current working directory by default. Override it or opt out of the Git guard using `ThreadOptions`.

```typescript
const thread = codex.startThread({
  workingDirectory: "/path/to/project",
  skipGitRepoCheck: true,
});
```

## Architecture overview

- `Codex` configures a `CodexExec`, finds the correct vendored binary per platform, and spawns it with JSON streaming enabled.
- `Thread` tracks an individual CLI session. It hands prompts to the binary, parses the JSONL event stream, and exposes both pull (`run`) and push (`runStreamed`) interfaces.
- Type definitions in `events.ts` and `items.ts` model every JSON payload emitted by Codex, so consumers can rely on static typing while pattern matching on streamed events.

## API reference

### `Codex`

| Member | Description |
| --- | --- |
| `new Codex(options?: CodexOptions)` | Constructs an SDK client and resolves the Codex binary path. |
| `startThread(options?: ThreadOptions): Thread` | Starts a brand-new conversation/turn sequence. |
| `resumeThread(id: string, options?: ThreadOptions): Thread` | Reattaches to an existing thread stored on disk. |

#### `CodexOptions`

| Option | Type | Default | Description |
| --- | --- | --- | --- |
| `codexPathOverride` | `string` | auto-detected | Absolute path to a Codex CLI binary. Skip vendored lookup when you manage the binary yourself. |
| `baseUrl` | `string` | undefined | Overrides the API endpoint. The SDK forwards it to the CLI via `OPENAI_BASE_URL`. |
| `apiKey` | `string` | undefined | Inline API key. Sent as `CODEX_API_KEY` to the child process; falls back to ambient environment variables otherwise. |

### `Thread`

| Member | Description |
| --- | --- |
| `thread.id` | The thread identifier (populated after the first `thread.started` event). |
| `run(input: string): Promise<RunResult>` | Sends a prompt, waits for completion, and returns `{ items, finalResponse, usage }`. Throws when the turn fails. |
| `runStreamed(input: string): Promise<RunStreamedResult>` | Sends a prompt and returns `{ events }`; iterate the async generator for live updates. |

`RunResult` is an alias for `Turn`:

```ts
type RunResult = {
  items: ThreadItem[];
  finalResponse: string;
  usage: Usage | null;
};
```

### `ThreadOptions`

| Option | Type | Default | CLI flag / behavior |
| --- | --- | --- | --- |
| `model` | `string` | CLI default | Applied as `--model <value>` when invoking the binary. |
| `sandboxMode` | `"read-only" \| "workspace-write" \| "danger-full-access"` | inherits CLI default | Mirrors the CLI `--sandbox` flag. |
| `workingDirectory` | `string` | `process.cwd()` | Executed via `--cd <path>`. Directory must be a Git repo unless you skip the check. |
| `skipGitRepoCheck` | `boolean` | `false` | Adds `--skip-git-repo-check`, letting Codex run outside Git repos. |

`SandboxMode` enumerates the harness behavior and aligns with Codex CLI sandboxing. The SDK also exports `ApprovalMode` (`"never" | "on-request" | "on-failure" | "untrusted"`) for applications that need to reflect the CLI approval model when surfacing choices to users, even though it is not consumed directly by `ThreadOptions`.

### `Thread events`

Events are emitted as JSONL lines. The SDK parses them into the discriminated union `ThreadEvent`:

| Type | Key fields | Meaning |
| --- | --- | --- |
| `thread.started` | `thread_id` | First event of every new thread. Use the ID to resume later. |
| `turn.started` | – | Fired whenever a prompt begins executing. |
| `turn.completed` | `usage` | Indicates the turn finished successfully. Token counts are available in `usage`. |
| `turn.failed` | `error` | Terminal failure for the turn; `run()` translates this into a thrown `Error`. |
| `item.started` | `item` | Marks the beginning of a tool call, execution, or message. |
| `item.updated` | `item` | Provides incremental updates (e.g. streaming command output). |
| `item.completed` | `item` | Signals that the item is done. Agent message completions update `finalResponse`. |
| `error` | `message` | Fatal stream errors that do not fit the turn lifecycle. |

The `Usage` object exposes `input_tokens`, `cached_input_tokens`, and `output_tokens`.

### `Thread items`

Items reflect the agent’s visible actions. The union exported as `ThreadItem` covers:

| Item type | Fields |
| --- | --- |
| `agent_message` | `text` with the assistant reply (plain text or structured JSON). |
| `reasoning` | `text` summarizing Codex’s thinking. |
| `command_execution` | `command`, aggregated output, optional `exit_code`, and `status` (`in_progress`, `completed`, `failed`). |
| `file_change` | `changes[]` (each entry has `path` and `kind`), plus `status` to indicate patch application. |
| `mcp_tool_call` | `server`, `tool`, `status`. |
| `web_search` | `query`. |
| `todo_list` | `items[]` (each with `text` and `completed`). |
| `error` | `message` describing a recoverable issue surfaced to the user. |

`Thread` aggregates every `item.completed` payload into `Turn.items` so callers can examine the agent’s work once the turn finishes.

### Advanced process control

`CodexExec` (used internally by `Codex`) resolves a platform-specific binary shipped in `sdk/typescript/vendor/<triple>/codex/`. Supply `codexPathOverride` if you package the binary yourself.

Each prompt is forwarded to the CLI as stdin. The SDK sets environment overrides when applicable:

- `OPENAI_BASE_URL` when `CodexOptions.baseUrl` is provided.
- `CODEX_API_KEY` when `CodexOptions.apiKey` is provided (the CLI also reads shared credentials from disk or env if unset).

Both `run()` and `runStreamed()` automatically switch to resume mode once Codex emits `thread.started`, so reusing a `Thread` instance across turns automatically persists and reuses the same identifier.

## Type exports

```ts
import {
  Codex,
  Thread,
  type CodexOptions,
  type ThreadOptions,
  type ThreadEvent,
  type ThreadItem,
  type RunResult,
  type RunStreamedResult,
} from "@openai/codex-sdk";
```

Every type exported from `src/index.ts` can be imported from the package root, so you can narrow items and events with TypeScript exhaustiveness checking.
