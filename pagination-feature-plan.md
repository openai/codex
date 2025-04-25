# CLI Pagination Feature: Standalone PR Plan

## Context

This document is the dedicated Memory Bank for the CLI Pagination feature. It is intended to be used as the root context for a new branch and PR, separate from the MCP integration. All planning, design, and progress for pagination should be tracked here.

---

## Why Separate?

- Pagination is a core UI/agent infrastructure improvement, not MCP-specific.
- Decoupling enables parallel review, easier testing, and future extensibility.
- This Memory Bank is self-contained to avoid merge conflicts and to enable clean re-initialization when cutting a new feature branch.

---

## Goals

- Provide robust, reusable pagination for all CLI list-like commands (resources, templates, etc.).
- Ensure pagination works interactively with natural language and explicit commands ("next", "previous", etc.).
- Make pagination logic generic and easy to extend for future list endpoints (e.g., logs, history).
- Maintain full test coverage for paginated flows.

---

## Scope

- Refactor agent and UI layers for generic pagination support.
- Update all resource and template listing commands to use new pagination.
- CLI should clearly prompt/navigate for paginated results.
- Add/expand tests for paginated flows (mocked and integration where possible).
- Document pagination usage in CLI help/docs.

---

## Out of Scope

- MCP protocol changes (handled in a separate PR).
- Non-list commands (unless they produce paginated output).
- UI/UX redesign outside of pagination.

---

## Implementation Plan

### 1. **Design & Planning**

- [ ] Review all CLI commands that return lists (resources, templates, etc.).
- [ ] Identify shared pagination logic and extract to utilities/hooks as needed.
- [ ] Define/confirm the interface for paginated agent methods (tokens, sizes, etc.).

### 2. **Agent Layer**

- [ ] Refactor agent methods (e.g., `listResources`, `listResourceTemplates`) to support/require pagination tokens and sizes.
- [ ] Ensure stateful tracking of current page, next/prev tokens, etc.
- [ ] Add generic pagination state/logic to agent loop.

### 3. **UI Layer**

- [ ] Update CLI UI to prompt for and handle "next", "previous", and page navigation commands.
- [ ] Ensure clear output when more results are available or when at the start/end.
- [ ] Make pagination UI logic reusable for new list endpoints.

### 4. **Testing**

- [ ] Add/expand unit tests for paginated agent methods (mocked MCP client).
- [ ] Add/expand CLI/Ink tests for paginated flows (simulate user input for next/prev).
- [ ] Document test coverage and edge cases.

### 5. **Documentation**

- [ ] Update CLI help output to mention pagination for all relevant commands.
- [ ] Add Memory Bank notes on pagination design, limitations, and extension points.

---

## Progress Tracking

- Use this file to record milestones, blockers, and key decisions throughout the feature branch lifecycle.
- When merging back to main, preserve this file for historical context or merge its contents into the main Memory Bank as appropriate.

---

## Merging Memory Banks: Guidance

- **Manual Merge:** When merging feature branches, manually review and merge Memory Bank files (e.g., copy relevant sections from `pagination-feature-plan.md` into `progress.md`, `systemPatterns.md`, etc. in main).
- **Avoid Overwrite:** Do not overwrite main Memory Bank files with feature branch versions unless you have reviewed for conflicts and completeness.
- **Preserve History:** Consider keeping feature-specific Memory Bank files as historical artifacts if they contain valuable design rationale or lessons learned.

---

## Next Steps

- [ ] Cut a new branch from main: `git checkout -b feat/cli-pagination main`
- [ ] Use this file as your Memory Bank root for all pagination work.
- [ ] Begin with design review and agent refactor as outlined above.
