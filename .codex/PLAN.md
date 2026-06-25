# Dead integration code cleanup Plan

Last updated: 2026-06-25

## Definition of Done
- [ ] All in-scope dead items found by scoped search/compiler evidence are removed.
- [ ] No live public, protocol, serialized, platform-specific, or test-only surface is removed.
- [ ] Formatting and relevant Rust checks/tests pass.
- [ ] Diff is independently reviewed and published as a draft PR with evidence.

## Milestones
1. Bootstrap worktree and map candidates.
2. Implement independent MCP, apps/connectors, and plugins/skills cleanup workstreams.
3. Integrate, format, test, review, and resolve regressions.
4. Commit, push, and open a draft PR.

## Workstreams
- MCP and MCP client/server plumbing.
- Apps and connectors.
- Plugins and skills.
- Cross-cutting integration and verification.

## Cycle Plan
### Cycle 0
- Goals: map candidates and gather reference/compiler evidence.
- Exit criteria: each workstream has concrete candidates with acceptance criteria.
- Notes: three parallel agents plus root orchestration.

### Cycle 1
- Goals: implement safe deletions and scoped tests.
- Exit criteria: clean integrated diff with targeted checks passing.
- Notes: reassign agents to verification/review after implementation.
