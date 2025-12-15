# Implementation Plan

- [x] 1. Create spec documents and align scope _Requirements: 1.1_
- [x] 2. Add config surface for Graphiti _Requirements: 1.1, 1.2, 2.4, 3.1, 4.1, 4.2_
- [x] 3. Implement Graphiti REST client _Requirements: 2.1, 3.1, 5.1_
- [x] 4. Implement memory service (gating, queue, retry) _Requirements: 1.2, 1.3, 1.4, 2.1, 2.2, 2.3_
- [x] 5. Wire recall + ingest into the turn loop _Requirements: 2.1, 3.2, 3.4_
- [x] 6. Add CLI commands (test/status/promote/purge) _Requirements: 4.3, 5.1, 5.2, 5.3_
- [x] 7. Add docs + demo guide _Requirements: 5.1_
- [x] 8. Add tests (mock Graphiti, assert recall+ingest) _Requirements: 2.1, 3.2, 3.4_
- [x] 9. Add git metadata support + tests _Requirements: 6.3_
- [x] 10. Run fmt/clippy/tests and ship PR _Requirements: all_

## Extension: Identity, ownership context, and automation

- [x] 11. Add config for identity + automation + docs _Requirements: 7.1, 7.2, 7.3, 8.1, 8.2_
- [x] 12. Derive Global group id from `graphiti.user_scope_key` _Requirements: 7.2_
- [x] 13. Ingest ownership context system message per group _Requirements: 7.1, 7.3_
- [x] 14. Implement `graphiti.recall.scopes_mode=auto` _Requirements: 8.1_
- [x] 15. Implement Memory Directives auto-promotion _Requirements: 8.2, 8.3, 8.4_
- [x] 16. Add tests for identity + automation _Requirements: 7.1, 7.2, 8.1, 8.2, 8.3, 8.4_
- [x] 17. Update demo guide with auto-mode + directives _Requirements: 5.1, 8.1, 8.2_

## Extension: Cross-client shared group ids

- [x] 18. Switch to canonical `graphiti_*` group ids _Requirements: 2.4, 3.1, 7.2, 9.1_
  - [x] 18.1 Derive `workspace` key from repo identity _Requirements: 9.2_
  - [x] 18.2 Auto-detect `github_login:<login>` user key _Requirements: 7.3, 9.3_
  - [x] 18.3 Include legacy Codex group ids in recall _Requirements: 9.4_
- [x] 19. Update docs/demo to explain shared memory _Requirements: 5.1, 9.4_

## Implementation Notes

- Automatic behavior is always best-effort and fail-open.
- Default scopes: ingest `session` + `workspace`; recall disabled until explicitly enabled.
- Group ids use hashed strategy by default to prevent leaking identifying strings.
- Git metadata (branch/commit/dirty) is computed at session init with tight timeouts to avoid impacting turns.

## Testing Priority

1. Unit-test the Graphiti client DTOs and error handling.
2. Integration-test codex-core turn loop injection + ingestion calls with a mock Graphiti server.
3. Smoke-test `codex graphiti test-connection` against a real service endpoint in dev.

## Backward Compatibility

- All new behavior is behind config flags and trusted-project gating.
- No existing config keys are modified.

## Current Status Summary

- Phase: implementation (canonical ids complete).
- Completed: core + CLI implementation, docs, demo, and tests.
- PR (fork): https://github.com/yulongbai-nov/codex/pull/1
- Completed (extension): identity + ownership context + auto recall + Memory Directives.
- Next tasks: address PR feedback and merge.
