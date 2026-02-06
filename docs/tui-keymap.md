# TUI Keymap Implementation Reference

This document is the long-term implementation reference for Codex TUI keybindings.
It describes how keymap configuration is resolved at runtime, which safety
contracts are intentionally strict, and how to test behavior end to end.

## Scope and boundaries

This keymap system is action-based and context-aware. It supports user rebinding
for the TUI without requiring source code edits.

Responsibilities:

1. Resolve config values into runtime key bindings.
2. Apply deterministic precedence.
3. Reject ambiguous bindings in dispatch scopes where collisions are unsafe.
4. Preserve explicit safety semantics for approval elicitation and onboarding.

Non-responsibilities:

1. It does not choose which screen should handle an event.
2. It does not persist config.
3. It does not guarantee terminal modifier reporting consistency; defaults may
   include compatibility variants for that reason.

## Source-of-truth map

- Runtime resolution: `codex-rs/tui/src/keymap.rs`
- Onboarding flow-level routing and quit guard:
  `codex-rs/tui/src/onboarding/onboarding_screen.rs`
- Approval/MCP elicitation option semantics:
  `codex-rs/tui/src/bottom_pane/approval_overlay.rs`
- Generic list popup navigation semantics:
  `codex-rs/tui/src/bottom_pane/list_selection_view.rs`
- User-facing default template:
  `https://github.com/openai/codex/blob/main/docs/default-keymap.toml`
- User-facing config overview: `docs/config.md`

## Config contract

`[tui.keymap]` is action-to-binding mapping by context.

```toml
[tui.keymap]
preset = "latest"

[tui.keymap.global]
submit = "enter"

[tui.keymap.composer]
submit = ["enter", "ctrl-j"]
```

Rules:

1. Mapping direction is `action -> key_or_keys`.
2. Values support:
   1. `action = "key-spec"`
   2. `action = ["key-spec-1", "key-spec-2"]`
   3. `action = []` for explicit unbind.
3. Unknown contexts, actions, or key identifiers fail validation.
4. Aliases are normalized (for example `escape -> esc`, `pgdn -> page-down`).
5. Key identifiers are lowercase and use `-` separators.

## Contexts and actions

Supported contexts:

1. `global`
2. `chat`
3. `composer`
4. `editor`
5. `pager`
6. `list`
7. `approval`
8. `onboarding`

Action inventory by context is documented in `docs/config.md` and the template
at `https://github.com/openai/codex/blob/main/docs/default-keymap.toml`.

## Presets and compatibility policy

Preset semantics:

1. `latest` is an alias to the newest shipped preset.
2. `v1` is the current frozen baseline.
3. Today, `latest -> v1`.

User guidance:

1. Pin `preset = "v1"` for stable behavior over time.
2. Use `preset = "latest"` to adopt new defaults when `latest` moves.

Developer policy:

1. Do not mutate old preset defaults after release.
2. Add a new version (for example `v2`) for behavior changes.
3. Update docs and migration notes whenever `latest` changes.

Compatibility detail:

Some actions intentionally ship with multiple default bindings because terminals
can report modifier combinations differently. Examples include `?` vs `shift-?`
and certain `ctrl` chords with optional `shift`.

## Resolution and precedence

Resolution order (highest first):

1. Context binding (`tui.keymap.<context>.<action>`)
2. Global fallback (`tui.keymap.global.<action>`) for chat/composer fallback
   actions only
3. Preset default binding

If no binding matches, normal unhandled-key fallback behavior applies.

## Conflict validation model

Validation is dispatch-order aware, not globally uniform.

Current conflict passes in `RuntimeKeymap::validate_conflicts` enforce:

1. App-level uniqueness for app actions and app-level chat controls.
2. App/composer shadowing prevention, because app handlers execute before
   forwarding to composer handlers.
3. Composer-local uniqueness for submit/queue/shortcut-toggle.
4. Context-local uniqueness in editor, pager, list, approval, and onboarding.

Intentionally allowed:

1. Same key across different contexts that are not co-evaluated in a way that
   can cause unsafe shadowing.
2. Shared defaults where runtime context gating keeps semantics unambiguous.

## Safety invariants

### MCP elicitation cancel semantics

For MCP elicitation prompts, `Esc` is always treated as `cancel`.

Implementation contract:

1. `Esc` is always included in cancel shortcuts.
2. User-defined `approval.cancel` shortcuts are merged into cancel.
3. Any overlap is removed from `approval.decline` in elicitation mode.

Rationale: dismissal must remain a safe abort path and never silently map to
"continue without requested info".

### Onboarding API-key text-entry guard

During API-key entry, printable `onboarding.quit` bindings are suppressed only
when the API-key field already has text.

Implementation contract:

1. Guard applies only when API-key entry mode is active.
2. Guard applies only to printable char keys without control/alt modifiers.
3. Guard applies only when input is non-empty.
4. Control/alt quit chords are never suppressed by this guard.

Rationale: keep text-entry safe once typing has begun while preserving an
intentional printable-quit path on empty input.

## Dispatch model and handler boundaries

High-level behavior:

1. App-level event handling runs before some lower-level handlers.
2. Composer behavior depends on both app routing and composer-local checks.
3. Onboarding screen routing applies flow-level rules before delegating to step
   widgets.
4. Approval overlay and list selection use context-specific bindings resolved by
   `RuntimeKeymap`.

When changing dispatch order, re-evaluate conflict validation scopes in
`keymap.rs` and associated tests.

## Diagnostics contract

Validation errors should be actionable and include:

1. Problem summary.
2. Exact config path.
3. Why the value is invalid or ambiguous.
4. Concrete remediation step.

Categories currently covered:

1. Invalid key specification.
2. Unknown action/context mapping.
3. Same-scope ambiguity.
4. Shadowing collisions in dispatch-coupled scopes.

## Debug path

When keybindings do not behave as expected, trace in this order:

1. Verify config normalization and schema validation in
   `codex-rs/core/src/config/tui_keymap.rs`.
2. Verify resolved runtime bindings and conflict checks in
   `codex-rs/tui/src/keymap.rs` (`from_config`, `validate_conflicts`).
3. Verify handler-level dispatch order in:
   1. `codex-rs/tui/src/app.rs` for app/chat/composer routing.
   2. `codex-rs/tui/src/pager_overlay.rs` for pager controls.
   3. `codex-rs/tui/src/bottom_pane/approval_overlay.rs` for approval safety
      behavior.
   4. `codex-rs/tui/src/onboarding/onboarding_screen.rs` for onboarding quit
      guard behavior.
4. Reproduce with explicit bindings in `~/.codex/config.toml` and compare
   against:
   1. `docs/default-keymap.toml`
   2. `docs/keymap-action-matrix.md`

## Testing notes

### Commands

Run from `codex-rs/`:

1. `just fmt`
2. `cargo test -p codex-tui --lib`
3. Optional full crate run (includes integration tests):
   `cargo test -p codex-tui`
4. Optional focused runs while iterating:
   `cargo test -p codex-tui --lib keymap::tests`

If `cargo test -p codex-tui` fails because the `codex` binary cannot be found
in local `target/`, run `--lib` for keymap behavior checks and then validate the
integration target in an environment where workspace binaries are available.

For intentional UI/text output changes in `codex-tui`:

1. `cargo insta pending-snapshots -p codex-tui`
2. `cargo insta show -p codex-tui <path/to/snapshot.snap.new>`
3. `cargo insta accept -p codex-tui` only when the full snapshot set is
   expected.

### Behavior coverage checklist

Use this checklist before landing keymap behavior changes:

1. Precedence: context override beats global fallback and preset defaults.
2. Unbind behavior: `action = []` actually removes the binding.
3. Conflict rejection:
   1. Same-context duplicates fail.
   2. App/composer shadowing fails for submit, queue, and toggle-shortcuts.
4. Approval safety:
   1. `Esc` resolves elicitation to cancel.
   2. Decline shortcuts never contain cancel overlaps in elicitation mode.
5. Onboarding safety:
   1. Printable quit key is suppressed when API-key input is active and
      non-empty.
   2. Printable quit key is not suppressed when input is empty.
   3. Control/alt quit chords are not suppressed.
6. Footer/help hints continue to reflect effective primary bindings.
7. `https://github.com/openai/codex/blob/main/docs/default-keymap.toml`,
   `docs/config.md`, and `docs/example-config.md` stay aligned with runtime
   action names and defaults.

### Manual sanity checks

1. Start onboarding and enter API-key mode.
2. Bind `onboarding.quit` to a printable key.
3. Verify that key quits when input is empty, then types once text exists.
4. Verify `ctrl-c` or another control quit chord still exits.
5. Trigger an MCP elicitation request and verify `Esc` cancels, not declines.

## Documentation maintenance

When adding/changing keymap API surface:

1. Update runtime definitions and defaults in `codex-rs/tui/src/keymap.rs`.
2. Update `docs/default-keymap.toml`.
3. Update `docs/config.md` and `docs/example-config.md` snippets.
4. Update this file with behavioral or safety contract changes.
5. Add/update regression tests in `codex-rs/tui`.
