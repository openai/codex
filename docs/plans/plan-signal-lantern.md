# Signal Lantern — Feature Integration Plan

## Objective
- Add accurate metadata for prompt suggestions so users can see whether a suggestion is LLM-generated and whether it used full conversation context vs only the last assistant response; surface this in TUI and TUI2.

## Scope
- In scope:
  - Extend the prompt suggestion event payload to include origin/context metadata.
  - Generate metadata in core based on actual prompt suggestion inputs.
  - Display metadata in TUI and TUI2 prompt suggestion UI (and wherever suggestion status is surfaced).
  - Update protocol types/schemas and any docs that explain prompt suggestions.
- Out of scope:
  - Changing prompt suggestion generation behavior, sampling rate, or model selection.
  - Adding new UI panes or altering other unrelated UI flows.
  - Non-CLI clients unless required by protocol compatibility.

## Assumptions
- Prompt suggestions are always LLM-generated (no human/heuristic source exists today), so the metadata value for origin is derived from the generation path rather than user input.
- “Full conversation context” means the suggestion input was derived from session history (retained user turns) rather than only the last assistant response; this is determined by whether `turn_context.history_depth` is set at generation time.
- Metadata should be attached at the protocol event level so both TUI and TUI2 consume the same source of truth.
- No legacy compatibility: existing consumers must adopt the new event shape; any fallback behavior is only for transitional defaulting during the migration period.

## Lenses Applied (Start + Ongoing)
- First-principles: capture metadata at the generation source (core), not inferred in UI.
- No-legacy-compat: update protocol + consumers together; do not preserve old shapes beyond temporary defaults.
- Line-entropy-audit: prefer small, centralized structs and reuse existing UI display pipelines; avoid redundant metadata calculations in multiple layers.

## Findings
- Target codebase:
  - Prompt suggestions are generated in `codex-rs/core/src/prompt_suggestions.rs` and emitted as `EventMsg::PromptSuggestion`.
  - The protocol event is defined in `codex-rs/protocol/src/protocol.rs` as `PromptSuggestionEvent { suggestion }`.
  - TUI and TUI2 consume the event in `codex-rs/tui/src/chatwidget.rs` and `codex-rs/tui2/src/chatwidget.rs`, then render in `codex-rs/tui/src/bottom_pane/prompt_suggestions_view.rs` and `codex-rs/tui2/src/bottom_pane/prompt_suggestions_view.rs`.
  - Prompt suggestion inputs use session history when `turn_context.history_depth` is set; otherwise only the last assistant response is used.
- Existing UI hints already show status/autorun but do not expose origin or context.

## Proposed Integration
### Architecture Fit
- Add metadata to `PromptSuggestionEvent` at the protocol boundary and pass it through existing event handling into prompt suggestion views.

### Data & State
- Extend `PromptSuggestionEvent` with fields such as:
  - `origin: PromptSuggestionOrigin` (e.g., `Llm`).
  - `context: PromptSuggestionContext` (e.g., `LastAssistant` vs `History { depth: u32 }`).
- Store the metadata alongside the suggestion in chat widget state and pass it into the prompt suggestion view.

### APIs & Contracts
- Update protocol schema/TS/JSON schema to include the new fields.
- Maintain defaults for legacy deserialization (e.g., `#[serde(default)]`) for a controlled rollout, but treat UI logic as dependent on the new fields.

### UI/UX
- Show metadata near the suggestion header, e.g., “Origin: LLM” and “Context: Full (last N user turns)” or “Context: Last assistant response”.
- Keep wording consistent in TUI and TUI2; ensure metadata visibility even when auto-run is enabled.

### Background Jobs & Async
- No new background jobs; metadata is attached at generation time.

### Config & Feature Flags
- No new feature flags; reuse existing prompt suggestion flags.

### Observability & Error Handling
- If metadata is missing, fall back to “Unknown” and log at debug level (no UI crash).

## Files To Touch
- `codex-rs/protocol/src/protocol.rs`
- `codex-rs/core/src/prompt_suggestions.rs`
- `codex-rs/tui/src/chatwidget.rs`
- `codex-rs/tui/src/bottom_pane/prompt_suggestions_view.rs`
- `codex-rs/tui2/src/chatwidget.rs`
- `codex-rs/tui2/src/bottom_pane/prompt_suggestions_view.rs`
- `docs/tui-chat-composer.md` (or another prompt suggestions doc if more accurate)

## Work Plan
1. Extend `PromptSuggestionEvent` with origin/context metadata; update schema/TS derives and defaults.
2. Emit accurate metadata in core based on `turn_context.history_depth` and generation path.
3. Thread metadata through TUI and TUI2 chat widget state into prompt suggestion views.
4. Render metadata in TUI/TUI2 prompt suggestions view with consistent labels.
5. Update docs to describe the metadata semantics and display.

## Risks & Mitigations
- Risk: Protocol change impacts other consumers.
  Mitigation: Add `serde(default)` for new fields and update all in-repo consumers.
- Risk: UI displays misleading context when history depth is overridden per turn.
  Mitigation: Derive context from the actual `turn_context` used in generation, not from global config.
- Risk: Extra UI noise.
  Mitigation: Keep metadata compact (single line) and only in suggestions view.

## Open Questions
- Should metadata be exposed elsewhere beyond the prompt suggestions view (e.g., status line or logs)?

## Test & Validation Plan
- Unit tests:
  - Add/extend tests for prompt suggestion event parsing with new fields in protocol tests if present.
- Integration tests:
  - Ensure suggestion events include metadata in core (mock event emission).
- Manual verification:
  - Trigger suggestions with history depth on/off and confirm context label toggles correctly in TUI and TUI2.

## Rollout
- Ship as a protocol + UI update in the same release; no legacy compatibility path beyond deserialization defaults.

## Approval
- Status: Pending
- Notes: Assumptions recorded; defaults only for safe rollout.
