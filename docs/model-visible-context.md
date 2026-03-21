# Model-visible context

This document defines the shared foundation for model-visible prompt context in
`codex-rs`.

The goal of this layer is simple: if the model should see some structured
context, represent it as a typed fragment instead of hand-assembling ad hoc
message payloads. That keeps rendering, role choice, and contextual-user
detection consistent before higher-level assembly code starts composing larger
prompt envelopes.

The shared foundation lives in
[`codex-rs/core/src/model_visible_context.rs`](../codex-rs/core/src/model_visible_context.rs).

Follow-up work will build on this foundation by introducing concrete fragment
types and wiring them into turn-state assembly, history parsing, and prompt
diffing. Those integrations are intentionally out of scope for this document.

## Core concepts

### Fragment role

Every named fragment type implements `ModelVisibleContextFragment` and declares
its role with `type Role`.

Use:

- `DeveloperContextRole` for developer guidance and policy text
- `ContextualUserContextRole` for contextual user-role state that should be
  parsed as context rather than literal user intent

Choosing the right role is part of the fragment contract, not a later callsite
decision.

### Fragment rendering

`ModelVisibleContextFragment` owns the text rendering for the fragment via
`render_text()`. Shared helpers convert that rendered text into the standard
model-visible payload shapes:

- `into_content_item()`
- `into_message()`
- `into_response_input_item()`

New model-visible context should use these conversions instead of
hand-constructing `ResponseItem::Message` payloads.

### Turn-state rebuild hook

Fragments that represent durable turn or session state can implement:

`ModelVisibleContextFragment::build(...)`

That hook receives:

- the current `TurnContext`
- an optional `reference_context_item`
- `TurnContextDiffParams`

The intended use is:

- compare current state against the persisted baseline in
  `reference_context_item`
- emit `Some(fragment)` only when the current prompt state needs to be injected
- leave the default `None` for fragments that are not rebuilt from turn state

This hook exists so later integrations can rebuild context correctly across
resume, compaction, backtracking, and fork.

### Contextual-user detection

Contextual-user fragments share the `user` role with real user messages, so
they need stable detection.

Preferred path:

- implement `contextual_user_markers()` when the fragment has fixed wrappers

Fallback path:

- override `matches_contextual_user_text()` when matching is genuinely custom

The foundation intentionally keeps that detection API close to fragment
definitions so later history-parsing code can rely on it consistently.

## Included shared helpers

The foundation module currently provides:

- `ModelVisibleContextRole`
- `DeveloperContextRole`
- `ContextualUserContextRole`
- `ContextualUserFragmentMarkers`
- `TurnContextDiffParams`
- `DeveloperTextFragment`
- `ContextualUserTextFragment`
- shared wrapper/tag constants for current contextual marker shapes

These helpers are intentionally generic. They should stay reusable across
future fragment definitions rather than reflecting any one prompt assembly path.

## Contributor guidance

When adding model-visible context:

1. Decide whether the model should see it at all.
2. If it should, prefer a typed fragment over a plain string.
3. Choose the role intentionally.
4. If the fragment is durable turn/session state, implement `build(...)`.
5. If the fragment is contextual-user, provide stable detection.
6. Use the shared conversion helpers instead of custom message assembly.

Developer-only one-off text can still be acceptable when it is truly isolated
and does not need contextual-user detection or turn-state reconstruction, but
that should be the exception rather than the default.
