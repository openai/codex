# FAQ

## Is the Python SDK stable?

`openai-codex` publishes stable releases. Install the latest one with
`pip install openai-codex`.

## Why does the SDK install a runtime package?

The SDK version tracks the corresponding Codex CLI release. Each SDK release
pins and installs its matching runtime dependency automatically.

## Thread vs turn

- A `Thread` is conversation state.
- A `Turn` is one model execution inside that thread.
- Multi-turn chat means multiple turns on the same `Thread`.

## `run()` vs `stream()`

- `Thread.run(...)` starts a turn and returns `TurnResult`.
- `TurnHandle.run()` / `AsyncTurnHandle.run()` consumes events for an existing turn handle and returns the same `TurnResult` shape.
- `TurnHandle.stream()` / `AsyncTurnHandle.stream()` yields raw notifications (`Notification`) so you can react event-by-event.

Choose `run()` for most apps. Choose `stream()` for progress UIs, custom timeout logic, or custom parsing.

## Sync vs async clients

- `Codex` is the sync public API.
- `AsyncCodex` is an async replica of the same public API shape.
- Prefer `async with AsyncCodex()` for async code. It is the standard path for
  explicit startup/shutdown, and `AsyncCodex` initializes lazily on context
  entry or first awaited API use.

If your app is not already async, stay with `Codex`.

## How do I log in?

- `login_api_key(...)` authenticates immediately with an API key.
- `login_chatgpt()` starts browser login and returns a handle with `auth_url`.
- `login_chatgpt_device_code()` starts device-code login and returns a handle
  with `verification_url` and `user_code`.
- Interactive handles expose `wait()` for the matching
  `account/login/completed` notification and `cancel()` to stop that attempt.
- `account()` reads the current account state, and `logout()` clears it.

## Public kwargs are snake_case

Public API keyword names are snake_case. The SDK still maps them to wire camelCase under the hood.

If you are migrating older code, update these names:

- `approvalPolicy` -> `approval_policy`
- `baseInstructions` -> `base_instructions`
- `developerInstructions` -> `developer_instructions`
- `modelProvider` -> `model_provider`
- `modelProviders` -> `model_providers`
- `sortKey` -> `sort_key`
- `sourceKinds` -> `source_kinds`
- `outputSchema` -> `output_schema`

## How do I choose sandbox access?

Use the same `sandbox=` keyword for threads and turns:

```python
from openai_codex import Sandbox

thread = codex.thread_start(sandbox=Sandbox.workspace_write)
result = thread.run("Review only.", sandbox=Sandbox.read_only)
```

The presets are:

- `Sandbox.read_only`: read files without allowing writes.
- `Sandbox.workspace_write`: the normal default for projects with a recorded trust decision; read files and write inside the workspace and configured writable roots.
- `Sandbox.full_access`: run without filesystem access restrictions.

When `sandbox=` is omitted, Codex uses its configured default. A turn
sandbox override applies to that turn and subsequent turns.

## Why only `thread_start(...)` and `thread_resume(...)`?

The public API keeps only explicit lifecycle calls:

- `thread_start(...)` to create new threads
- `thread_resume(thread_id, ...)` to continue existing threads

This avoids duplicate ways to do the same operation and keeps behavior explicit.

## Why does constructor fail?

`Codex()` is eager: it starts transport and calls `initialize` in `__init__`.

Common causes:

- installation is incomplete and the pinned `openai-codex-cli-bin` dependency is missing
- local `codex_bin` override points to a missing file
- a custom local Codex executable does not support the SDK operation being used

## Why does a turn "hang"?

A turn is complete only when `turn/completed` arrives for that turn ID.

- `run()` waits for this automatically.
- With `stream()`, keep consuming notifications until completion.

## How do I retry safely?

Use `retry_on_overload(...)` for transient overload failures (`ServerBusyError`).

Do not blindly retry all errors. For `InvalidParamsError` or
`MethodNotFoundError`, fix the input or use the runtime pinned by the SDK.

## Common pitfalls

- Starting a new thread for every prompt when you wanted continuity.
- Forgetting to `close()` (or not using context managers).
- Reading `Turn.items` from live start/completed payloads instead of using `TurnResult.items`.
- Mixing SDK input classes with raw dicts incorrectly.
