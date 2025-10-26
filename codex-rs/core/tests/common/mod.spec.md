## Overview
`core/tests/common` houses fixtures, builders, and assertions that power Codex core integration and CLI tests. It normalizes configuration setup, SSE playback, and mock response handling so suites can focus on behavior assertions.

## Detailed Behavior
- `lib.rs` exports helpers for loading default configs, constructing SSE payloads, waiting for Codex events, and skipping sandbox-unsafe tests.
- `responses.rs` builds wiremock mocks and SSE helpers for the `/v1/responses` API, while validating request invariants to catch regressions early.
- `test_codex.rs` produces a fully configured `CodexConversation` backed by temp dirs and a mocked model provider.
- `test_codex_exec.rs` provides convenience builders for invoking the `codex-exec` binary under controlled environments.

## Broader Context
- Imported by `core/tests/all.rs` and other suites to simplify end-to-end validation across Codex conversation flows.
- Shares conventions with app-server test fixtures (e.g., SSE formatting, wiremock usage) to keep integration testing behavior consistent.

## Technical Debt
- Several utilities duplicate logic from production crates (SSE formatting, file polling). Consolidating them into reusable libraries would reduce divergence.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Deduplicate SSE/event helpers with production code to avoid drift across crates.
related_specs:
  - ./lib.rs.spec.md
  - ./responses.rs.spec.md
  - ./test_codex.rs.spec.md
  - ./test_codex_exec.rs.spec.md
