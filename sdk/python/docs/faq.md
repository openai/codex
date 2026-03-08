# FAQ

## Thread vs turn

- A `Thread` is conversation state.
- A `Turn` is one model execution inside that thread.
- Multi-turn chat means multiple turns on the same `Thread`.

## `run()` vs `stream()`

- `Turn.run()` is the easiest path. It consumes events until completion and returns `TurnResult`.
- `Turn.stream()` yields raw notifications (`Notification`) so you can react event-by-event.

Choose `run()` for most apps. Choose `stream()` for progress UIs, custom timeout logic, or custom parsing.

## Sync vs async clients

- `Codex` is the minimal sync SDK and best default.
- `AsyncAppServerClient` wraps the sync transport with `asyncio.to_thread(...)` for async-friendly call sites.

If your app is not already async, stay with `Codex`.

## `thread(...)` vs `thread_resume(...)`

- `codex.thread(thread_id)` only binds a local helper to an existing thread ID.
- `codex.thread_resume(thread_id, ...)` performs a `thread/resume` RPC and can apply overrides (model, instructions, sandbox, etc.).

Use `thread(...)` for simple continuation. Use `thread_resume(...)` when you need explicit resume semantics or override fields.

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
- With `stream()`, make sure you keep consuming notifications until completion.

## How do I retry safely?

Use `retry_on_overload(...)` for transient overload failures (`ServerBusyError`).

Do not blindly retry all errors. For `InvalidParamsError` or `MethodNotFoundError`, fix inputs/version compatibility instead.

## Common pitfalls

- Starting a new thread for every prompt when you wanted continuity.
- Forgetting to `close()` (or not using `with Codex() as codex:`).
- Ignoring `TurnResult.status` and `TurnResult.error`.
- Mixing SDK input classes with raw dicts incorrectly in minimal API paths.
