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

- Phase: implementation.
- Completed: core + CLI implementation, docs, demo, and tests.
- PR: https://github.com/openai/codex/pull/8026
- Next tasks: address review feedback and merge.
