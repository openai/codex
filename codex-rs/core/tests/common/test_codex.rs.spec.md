## Overview
`test_codex` constructs fully configured `CodexConversation` instances for integration tests, wiring mock model providers and temporary directories into a repeatable harness.

## Detailed Behavior
- `TestCodexBuilder` collects configuration mutators so tests can tweak settings before instantiation.
- `build` creates a fresh temp Codex home, while `resume` resumes from an existing rollout archive by invoking `ConversationManager::resume_conversation_from_rollout`.
- `prepare_config`:
  - Clones the OpenAI model provider settings, overriding the base URL to point at the supplied `wiremock::MockServer`.
  - Sets `config.cwd` to a new temp directory and, on Linux, points sandbox binaries at the `codex` CLI via `cargo_bin`.
  - Applies queued config mutators.
- Returns `TestCodex` containing the shared home directory, working directory, the `CodexConversation`, and the `SessionConfiguredEvent` for assertions.
- `test_codex()` seeds a fresh builder with no mutators.

## Broader Context
- Used across core integration suites to spin up conversations quickly, often combined with `responses` mocks to drive model interactions.
- Complements `test_codex_exec`, which targets the CLI binary variant.

## Technical Debt
- Builder relies on `assert_cmd::Command::cargo_bin("codex")`, which assumes the binary is built; failing builds yield opaque errors. Additional diagnostics could streamline failures.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
