# Codex Python SDK PRD

## Goal
Build a full-featured Python SDK that mirrors the TypeScript Codex SDK behavior, providing both synchronous and asyncio APIs. The SDK wraps the `codex` CLI and exposes structured events and turn results while remaining Pythonic in naming and ergonomics.

## Scope
- Sync SDK: `codex_sdk.Codex` + `Thread` with `run` and `run_streamed`.
- Async SDK: `codex_sdk.asyncio.AsyncCodex` + `AsyncThread` with async equivalents.
- Output schema support via Pydantic (convert to JSON schema) and plain JSON schema dicts.
- Runtime checks:
  - Missing Codex CLI -> actionable installation guidance from the reference README.
  - Missing ChatGPT sign-in or API key -> actionable authentication guidance.
- Samples and tests consistent with the TypeScript SDK.

## Non-goals
- Re-implementing Codex agent logic (handled by CLI).
- Building a separate transport or direct API client.

## Public API (Pythonic)
- `Codex(...)` / `AsyncCodex(...)`
  - `start_thread(...)` -> new thread
  - `resume_thread(thread_id, ...)` -> resume
- `Thread` / `AsyncThread`
  - `run(input, turn_options=...)` -> returns `Turn`
  - `run_streamed(input, turn_options=...)` -> returns `{ events: generator }`

## Inputs
- `input: str | list[UserInput]`
  - `UserInput`: `{ type: "text", text: str }` or `{ type: "local_image", path: str }`
  - Text entries concatenated with blank lines.

## Options
- Codex options: `codex_path_override`, `base_url`, `api_key`, `env`.
- Thread options: `model`, `sandbox_mode`, `working_directory`, `skip_git_repo_check`,
  `model_reasoning_effort`, `network_access_enabled`, `web_search_enabled`,
  `approval_policy`, `additional_directories`.
- Turn options: `output_schema`, `signal`.

## Error Handling
- `CodexNotInstalledError`: indicates missing CLI and provides install steps.
- `AuthRequiredError`: indicates missing ChatGPT login or API key.
- `AbortError`: raised when an abort signal is triggered.
- `ThreadRunError`: generic run failures.

## Implementation Notes
- Use subprocess for sync and asyncio subprocess for async.
- Parse JSONL from stdout into Python dicts (typed via `TypedDict` hints).
- Always clean up temporary schema files after use.
- Preserve TS behavior for originator header via env `CODEX_INTERNAL_ORIGINATOR_OVERRIDE`.

## Tests
- Event streaming, thread id updates, and turn completion.
- Argument forwarding (model, sandbox, additional dirs, images, output schema).
- Environment override behavior.
- Abort handling (pre-abort and mid-stream).
- Input normalization (text + image segments).

## Dependencies
- Runtime: `pydantic`.
- Dev/test: `pytest`, `pytest-asyncio`.

## Checklist
- [x] SDK sync API implemented.
- [x] SDK async API implemented.
- [x] Output schema (Pydantic + dict) supported.
- [x] CLI install/auth checks with guidance.
- [x] Samples aligned with TS SDK (incl. Pydantic schema sample).
- [x] Tests implemented and passing.
- [x] README updated with usage.
