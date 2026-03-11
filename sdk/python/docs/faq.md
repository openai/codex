# FAQ

## Thread vs turn

- A `Thread` is conversation state.
- A `Turn` is one model execution inside that thread.
- Multi-turn chat means multiple turns on the same `Thread`.

## `run()` vs `stream()`

- `Turn.run()` / `AsyncTurn.run()` is the easiest path. It consumes events until completion and returns `TurnResult`.
- `Turn.stream()` / `AsyncTurn.stream()` yields raw notifications (`Notification`) so you can react event-by-event.

Choose `run()` for most apps. Choose `stream()` for progress UIs, custom timeout logic, or custom parsing.

## Sync vs async clients

- `Codex` is the sync public API.
- `AsyncCodex` is an async replica of the same public API shape.

If your app is not already async, stay with `Codex`.

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
- `sandboxPolicy` -> `sandbox_policy`

## Why only `thread_start(...)` and `thread_resume(...)`?

The public API keeps only explicit lifecycle calls:

- `thread_start(...)` to create new threads
- `thread_resume(thread_id, ...)` to continue existing threads

This avoids duplicate ways to do the same operation and keeps behavior explicit.

## Why does constructor fail?

`Codex()` is eager: it starts transport and calls `initialize` in `__init__`.

Common causes:

- bundled runtime binary missing for your OS/arch under `src/codex_app_server/bin/*`
- local auth/session is missing
- incompatible/old app-server

Maintainers can refresh bundled binaries with:

```bash
cd sdk/python
python scripts/update_sdk_artifacts.py --channel stable --bundle-all-platforms
```

## Why does a turn "hang"?

A turn is complete only when `turn/completed` arrives for that turn ID.

- `run()` waits for this automatically.
- With `stream()`, keep consuming notifications until completion.

## How do I retry safely?

Use `retry_on_overload(...)` for transient overload failures (`ServerBusyError`).

Do not blindly retry all errors. For `InvalidParamsError` or `MethodNotFoundError`, fix inputs/version compatibility instead.

## Common pitfalls

- Starting a new thread for every prompt when you wanted continuity.
- Forgetting to `close()` (or not using context managers).
- Ignoring `TurnResult.status` and `TurnResult.error`.
- Mixing SDK input classes with raw dicts incorrectly.
