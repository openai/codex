# Build a Python SDK for Codex CLI

This ExecPlan is a living document. The sections `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work proceeds. There is no PLANS.md file in this repository; all guidance comes from EXEC_PLAN.md.

## Purpose / Big Picture

Deliver a first-class Python SDK that mirrors the TypeScript SDK in `sdk/typescript`. Python developers should be able to embed the Codex CLI agent by instantiating a client, starting or resuming threads, and running turns synchronously or as a streamed generator of events. Success means someone can pip-install the package locally (editable install is fine), run the sample code from the README, and observe structured events and final responses without referring back to the TypeScript sources.

## Progress

- [x] (2025-12-05 18:31Z) Reviewed EXEC_PLAN.md, TypeScript SDK sources/tests, and exec event schema to scope the Python SDK.
- [x] (2025-12-05 18:44Z) Established Python package scaffold and parity API surface (modules, classes, type hints).
- [x] (2025-12-05 18:44Z) Implemented process runner, thread logic, schema handling, and streaming/parsing.
- [x] (2025-12-05 18:45Z) Added README, examples, and tests exercising options, streaming, cancellation, and schema handling.
- [ ] (2025-12-05 18:45Z) Ran unittest suite (skipped in sandbox due to loopback bind restrictions); need re-run in an environment that allows local HTTP servers to validate fully.

## Surprises & Discoveries

- pip install -e . with build isolation failed offline; reran with --no-build-isolation but installation was blocked by site-packages permissions. Worked around by running tests with PYTHONPATH instead of installation.
- Sandbox denied binding a loopback HTTP server, so tests that rely on SSE proxies were skipped in this environment. The proxy helper now skips cleanly when sockets are unavailable.

## Decision Log

- Decision: Follow the TypeScript SDK behavior and default flags, using the Codex CLI (`codex exec --experimental-json`) as the transport. Adopt a Pythonic surface (snake_case, context managers where useful) while keeping method names close to TS (`start_thread`, `resume_thread`, `run`, `run_streamed`).  
  Rationale: Minimizes divergence for users moving between languages while respecting Python conventions.  
  Date/Author: 2025-12-05 / assistant
- Decision: Skip proxy-backed tests when the sandbox forbids binding a loopback HTTP server.  
  Rationale: Allows local test invocation without hard failures in restricted environments while preserving coverage when sockets are available.  
  Date/Author: 2025-12-05 / assistant

## Outcomes & Retrospective

- To be filled after implementation and validation.

## Context and Orientation

- The TypeScript SDK lives in `sdk/typescript`. Key files: `src/codex.ts`, `src/thread.ts`, `src/exec.ts`, `src/events.ts`, `src/items.ts`, and `src/outputSchemaFile.ts`. Tests in `sdk/typescript/tests` exercise threading, streaming, options, env overrides, output schema handling, images, additional directories, and abort signals.
- The Codex CLI emits JSONL events described in `codex-rs/exec/src/exec_events.rs`. Events include `thread.started`, `turn.started`, `item.*`, `turn.completed`, `turn.failed`, and `error`, with item payloads such as `agent_message`, `command_execution`, `file_change`, `mcp_tool_call`, `web_search`, `todo_list`, and `error`.
- The CLI is invoked as `codex exec --experimental-json` with flags like `--model`, `--sandbox`, `--cd`, `--add-dir`, `--skip-git-repo-check`, `--output-schema`, and `--config` entries for `model_reasoning_effort`, `sandbox_workspace_write.network_access`, `features.web_search_request`, and `approval_policy`. Images are forwarded via repeated `--image` flags.
- The TypeScript SDK writes output schemas to a temp file and cleans them up after each turn. It aggregates text inputs separated by blank lines, forwards images, sets `CODEX_INTERNAL_ORIGINATOR_OVERRIDE` to `codex_sdk_ts`, and injects `OPENAI_BASE_URL` and `CODEX_API_KEY` into the child env unless overridden.
- Tests use a local Codex binary built from `codex-rs` (e.g., `codex-rs/target/debug/codex`) and a lightweight HTTP proxy in tests to capture `/responses` requests and stream SSE events. We should reuse this strategy with Python’s stdlib to avoid network calls.

## Plan of Work

First, scaffold a Python package under `sdk/python` with `pyproject.toml`, `README.md`, and `src/codex_sdk` module files. Mirror the TS module layout: `codex.py` (entry client), `thread.py` (thread state and run/run_streamed), `exec.py` (process runner), `types.py` (events/items dataclasses), and `schema_file.py` (temp schema handling). Export a `Codex` class that owns a `CodexExec` runner and produces `Thread` instances via `start_thread`/`resume_thread`. Thread options should include model, sandbox_mode, working_directory, skip_git_repo_check, model_reasoning_effort, network_access_enabled, web_search_enabled, approval_policy, and additional_directories; turn options should allow `output_schema` and `cancellation` (event or asyncio task cancellation).

Implement `CodexExec.run` to spawn the Codex CLI with the same flags as TS, accept a `signal`/cancel hook, wire stdin/stdout, and yield decoded lines. Build env injection mirroring TS: inherit `os.environ` unless overridden, set `CODEX_INTERNAL_ORIGINATOR_OVERRIDE=codex_sdk_py`, and overlay `OPENAI_BASE_URL`/`CODEX_API_KEY` when provided. Provide binary resolution similar to TS (`vendor/<target-triple>/codex/codex[.exe]`) with optional override path.

Implement event parsing into typed dataclasses, raising clear errors on JSON decode or unexpected structures. `Thread.run_streamed` should normalize inputs (string or list of `{type: "text"|"local_image"}` dicts), concatenate text segments with blank lines, collect image paths, and call `CodexExec.run`. As events stream, update `thread_id` on `thread.started`, forward each parsed event to the caller, and on completion capture `usage` and `final_response` (last `agent_message` text) while collecting items. `Thread.run` should drain the generator and either return `{items, final_response, usage}` or raise on `turn.failed` or stream-level errors. Ensure output schema temp dir is cleaned even on exceptions.

Add tests with `unittest` mirroring TS coverage: successful run collecting items/usage; repeated runs continue thread and include previous assistant output; resume_thread by id; options mapped to CLI flags; env override; output schema temp file creation and cleanup; input concatenation; image forwarding; additional_directories; working_directory with and without `skip_git_repo_check`; originator header; turn failure surfaces errors; streaming path yields events. Implement an HTTP proxy helper (akin to `responsesProxy.ts`) using `http.server` + SSE formatting to capture requests for assertions. Provide a simple abort/cancel test using `threading.Event` or `asyncio` cancellation to ensure process termination.

Document usage in `sdk/python/README.md` with quickstart, streaming example, structured output, image input, resume thread, working directory controls, and env overrides. Note Python version requirement (e.g., 3.9+), install steps (`pip install -e .`), and test command.

## Concrete Steps

- Work in `sdk/python`.
- Create packaging scaffold: `pyproject.toml` with `setuptools`, `src/codex_sdk/__init__.py`, and module files. Add `README.md` referencing examples below.
- Port core logic from TS to Python modules as described above.
- Write tests in `sdk/python/tests` using stdlib `unittest` and helper proxy server; point Codex path to `../codex-rs/target/debug/codex`.
- Commands to run (once implemented):
  - Build/install editable for development: `cd sdk/python && python -m pip install -e .`
  - Run tests: `cd sdk/python && python -m unittest discover -v`
  - Optional manual demo: `python -m examples.basic` once examples exist.
- Keep this section updated if commands change during implementation.

## Validation and Acceptance

Acceptance hinges on observable behavior: installing the package locally, running the quickstart script, and seeing a turn complete with streamed events and final response text. Automated validation: `python -m unittest discover -v` passes, including cases for options mapping, output schema lifecycle, image forwarding, and cancellation. Manual validation: start a thread, call `run` twice to confirm thread continuity, and `resume_thread` with saved id. The SDK should emit helpful errors on malformed events or CLI failures.

## Idempotence and Recovery

All commands are safe to re-run. Package builds are additive; re-running tests or installs is safe. Temp schema directories and proxy servers should be cleaned in `finally` blocks; retrying a failed test should not leak files or processes. If the CLI is missing, the SDK should raise a clear error rather than leaving partial state.

## Artifacts and Notes

- Keep key test transcripts or sample outputs short and inline in this plan as progress is made.
- Note deviations from TS behavior with rationale in the Decision Log.

## Interfaces and Dependencies

- Public classes/functions to expose in `codex_sdk`:
  - `Codex(codex_path_override: Optional[str] = None, base_url: Optional[str] = None, api_key: Optional[str] = None, env: Optional[Dict[str, str]] = None)`
  - `Codex.start_thread(options: ThreadOptions = None) -> Thread`
  - `Codex.resume_thread(thread_id: str, options: ThreadOptions = None) -> Thread`
  - `Thread.run(input: Input, turn_options: TurnOptions = None) -> TurnResult`
  - `Thread.run_streamed(input: Input, turn_options: TurnOptions = None) -> StreamedTurn` where `StreamedTurn.events` is an iterator/generator of `ThreadEvent`.
  - `Thread.id` property returning the current thread id (or None before first turn).
- Types:
  - `Input = Union[str, List[InputEntry]]` with `InputEntry` dicts `{ "type": "text", "text": str }` or `{ "type": "local_image", "path": str }`.
  - `ThreadEvent`/`ThreadItem` dataclasses mirroring `codex-rs/exec/src/exec_events.rs`.
  - `TurnResult` holding `items: List[ThreadItem]`, `final_response: str`, `usage: Optional[Usage]`.
  - `ThreadOptions` (model, sandbox_mode, working_directory, skip_git_repo_check, model_reasoning_effort, network_access_enabled, web_search_enabled, approval_policy, additional_directories) and `TurnOptions` (output_schema, signal/cancel hook).
- Dependencies: prefer Python stdlib; avoid third-party packages unless already vendored. Use `subprocess`, `json`, `tempfile`, `pathlib`, `typing`, `dataclasses`, and `asyncio` if needed for cancellation support.

---

Revision note: Initial version created to guide Python SDK implementation based on the TypeScript SDK, following EXEC_PLAN.md requirements.
Revision note: Updated after implementing the Python SDK, adding docs/tests, and documenting sandbox-related test skips (2025-12-05).
