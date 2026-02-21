# Research: Solutions for Repo Search and Chained-Invariant Findings

Date: 2026-02-12

This note proposes implementation-ready fixes for the four reported findings, with risk and validation guidance.

## 1) High: `query_project` can trigger repeated full rebuilds when embeddings are unavailable

### Confirmed root cause

1. `handle_query_project` first calls `refresh(..., EmbeddingMode::Required)` and on failure retries with `refresh(..., force_full=true, EmbeddingMode::Skip)`.
2. `refresh` stores `embedding_ready=false` in Skip mode.
3. Next search re-enters Required mode; `stored_ready=false` forces `force_full=true`, causing another full clear/rebuild cycle before failing again.

References: `codex-rs/mcp-server/src/query_project.rs:270`, `codex-rs/mcp-server/src/query_project.rs:559`, `codex-rs/mcp-server/src/query_project.rs:693`.

### Recommended fix

Implement an explicit embedding strategy selection before refresh:

1. Add `EmbeddingMode::Auto` (or equivalent helper) that resolves to:
   - `Skip` when embeddings are clearly unavailable (at minimum: missing `OPENAI_API_KEY`).
   - `Required` when embeddings appear available.
2. Use this strategy in `query_project` so missing-key paths do not attempt Required mode at all.
3. Keep `stored_ready=false` in lexical mode; when embeddings become available later, the first Required refresh should do one full rebuild and restore vector state.

This removes repeated full-rebuild churn while preserving automatic recovery once embeddings become available.

### Optional hardening (for transient network failures)

1. Persist a short cooldown metadata field (for example `embedding_retry_after_unix`) after embedding transport failures.
2. During cooldown, force `Skip` mode to avoid repeated full rebuild attempts.
3. Retry Required mode after cooldown expiry.

## 2) Medium: `repo_index_refresh` hard-fails without `OPENAI_API_KEY`

### Confirmed root cause

1. `handle_repo_index_refresh` always uses `EmbeddingMode::Required`.
2. `embed_texts` hard-requires `OPENAI_API_KEY`.

References: `codex-rs/mcp-server/src/query_project.rs:213`, `codex-rs/mcp-server/src/query_project.rs:1267`.

### Recommended fix

Align refresh behavior with hybrid search degradation:

1. Make `repo_index_refresh` use the same auto strategy as above.
2. Default behavior should be:
   - Build/refresh lexical index even without embeddings.
   - Return success with structured status indicating lexical-only mode.
3. Add an explicit strict option in params (for example `require_embeddings: bool`, default `false`) for callers that want hard-failure semantics.

This keeps the tool usable for ChatGPT/session-auth users without API keys while preserving an opt-in strict path.

### Suggested response payload extension

Return an embedding status object in both repo tools:

1. `mode_used`: `required|skip`
2. `ready`: `true|false`
3. `reason`: optional (`missing_api_key`, `transport_error`, etc.)

This improves debuggability and client UX.

## 3) Medium: chained-request invariant checks are too relaxed

### Confirmed root cause

All call/output symmetry checks are gated behind `if !is_chained_request`, so chained requests skip both:

1. output->call matching checks
2. call->output symmetry checks

Reference: `codex-rs/core/tests/common/responses.rs:1241`.

### Recommended fix (minimum safe change)

In `validate_request_body_invariants`:

1. Keep chained relaxation only for output->call checks (because calls may be inherited through `previous_response_id`).
2. Always enforce call->output symmetry for call items present in the current request payload.

This restores protection against malformed chained payloads that introduce new calls without outputs.

### Recommended fix (stronger, preferred)

Make validation stateful across requests captured by the same `ResponseMock`:

1. Track known `function_call`, `local_shell_call`, and `custom_tool_call` IDs from earlier requests.
2. For chained requests, allow outputs that match either:
   - a call in the current request, or
   - a known call from prior requests.
3. Still require every call present in the current request to have a same-request output.

This preserves valid chaining while detecting orphan outputs and missing outputs robustly.

## 4) Low: MCP docs do not reflect the added repo tools

### Confirmed root cause

Docs describe only `codex` and `codex-reply` tool responses, but server `tools/list` includes two additional tools.

References:

1. `codex-rs/docs/codex_mcp_interface.md:128`
2. `codex-rs/mcp-server/src/message_processor.rs:315`

### Recommended fix

Update `codex-rs/docs/codex_mcp_interface.md`:

1. Document all four tools: `codex`, `codex-reply`, `query_project`, `repo_index_refresh`.
2. Clarify response-shape differences:
   - `codex`/`codex-reply` include `threadId` in `structuredContent`.
   - repo tools return search/refresh payloads in `content` and mirrored `structuredContent` (no `threadId`).
3. Add compact JSON examples for both repo tools.

## Validation plan

1. Add unit/integration coverage in `codex-mcp-server` for:
   - missing-key hybrid search does not full-rebuild repeatedly
   - index refresh degrades to lexical mode by default
   - strict `require_embeddings=true` still fails without embeddings
2. Add tests in `core/tests/common/responses.rs` for chained invariants:
   - chained output referencing prior call passes
   - chained request with new call and missing output fails
   - chained orphan output with unknown call ID fails (stateful version)
3. Update docs and add/adjust MCP interface tests if snapshot/schema checks exist.

## Implementation order

1. Ship strategy unification (`Auto` embedding behavior) for both repo tools.
2. Restore chained invariant protections (minimum safe change first, then stateful check if needed).
3. Update MCP docs to match tool surface and response shapes.
