# Model-visible context

This document describes how model-visible prompt context is represented in the
current `codex-rs` codebase, and how to add new context without breaking
resume, compaction, backtracking, or history parsing.

The key modules are:

- [`codex-rs/core/src/model_visible_context.rs`](/Users/ccunningham/code/codex-worktree-tria/codex-rs/core/src/model_visible_context.rs):
  shared fragment abstractions, roles, marker helpers, and diff params
- [`codex-rs/core/src/model_visible_fragments.rs`](/Users/ccunningham/code/codex-worktree-tria/codex-rs/core/src/model_visible_fragments.rs):
  concrete fragment definitions, the central registry, shared contextual-user
  detection, and turn-state fragment assembly
- [`codex-rs/core/src/context_manager/updates.rs`](/Users/ccunningham/code/codex-worktree-tria/codex-rs/core/src/context_manager/updates.rs):
  the shared developer/contextual-user envelope builders and steady-state
  per-turn diff assembly
- [`codex-rs/core/src/codex.rs`](/Users/ccunningham/code/codex-worktree-tria/codex-rs/core/src/codex.rs):
  initial-context assembly and the normal runtime path that decides whether to
  inject full context or only diffs

## Core model

Model-visible prompt context falls into three buckets:

1. Turn-state fragments.
   These are derived from current `TurnContext` state. Most represent durable
   turn/session state that must survive history-mutating flows such as resume,
   compaction, backtracking, and fork by being rebuilt from current state plus
   an optional persisted baseline. Some, such as ephemeral context, are
   intentionally current-turn-scoped: they rebuild from the active
   `TurnContext`, but are not persisted in the durable baseline for older
   turns.
2. Registered runtime fragments.
   These are not derived from `TurnContext` diffs, but they are still modeled
   as typed fragments because they are emitted into model-visible history and,
   for contextual-user fragments, must still be recognized during later history
   parsing.
3. Developer-only one-off text injections.
   These are not currently represented as dedicated registered fragments. They
   are developer-role text added in a few call sites where no contextual-user
   detection or turn-state diffing is needed.

The single most important distinction is whether the model-visible state is:

- turn/session state that should be rebuilt from `TurnContext`
- or a one-off event/message that is only relevant because it just happened

That determines whether the fragment needs `build(...)`.

## Two-envelope rule

Turn-state context assembly always produces exactly two envelopes:

- one `developer` message
- one contextual `user` message

Those envelopes are assembled by the shared builders in
[`updates.rs`](/Users/ccunningham/code/codex-worktree-tria/codex-rs/core/src/context_manager/updates.rs).
Adjacent text content items are explicitly separated with `\n\n` because, from
the model's perspective, adjacent text `ContentItem`s inside one message are
effectively concatenated.

This two-envelope rule applies to turn-state context assembly:

- [`Codex::build_initial_context(...)`](/Users/ccunningham/code/codex-worktree-tria/codex-rs/core/src/codex.rs#L3342)
- [`build_settings_update_items(...)`](/Users/ccunningham/code/codex-worktree-tria/codex-rs/core/src/context_manager/updates.rs#L105)

It does not mean every model-visible message in the system is forced into those
two envelopes. Runtime/session-prefix messages may still be emitted as separate
messages when the event itself is what needs to be recorded, and some
initial-only contextual/developer additions are appended to the envelopes
outside registry-driven `build(...)` assembly.

## Current architecture

### 1. Shared fragment trait

Every named fragment type implements
[`ModelVisibleContextFragment`](/Users/ccunningham/code/codex-worktree-tria/codex-rs/core/src/model_visible_context.rs#L180).

That trait owns:

- `type Role`
- `render_text()`
- optional `build(...)` / `build_many(...)` for turn-state fragments
- optional contextual-user detection via `contextual_user_markers()` or
  `matches_contextual_user_text()`
- standard conversions such as `into_message()` and `into_response_input_item()`

Roles are:

- `DeveloperContextRole`
- `ContextualUserContextRole`

### 2. Central registry

Current fragment types are registered exactly once in
[`REGISTERED_MODEL_VISIBLE_FRAGMENTS`](/Users/ccunningham/code/codex-worktree-tria/codex-rs/core/src/model_visible_fragments.rs#L123).

That registry is used for:

- contextual-user fragment detection during history parsing
- turn-state fragment assembly for both envelopes by calling `build(...)`

Registration is therefore an integration step, not just bookkeeping. Defining
the type is not enough; the fragment is not part of the system until it is
registered.

### 3. Build semantics

[`ModelVisibleContextFragment::build(...)`](/Users/ccunningham/code/codex-worktree-tria/codex-rs/core/src/model_visible_context.rs#L187)
is the canonical hook for the common zero-or-one turn-state case.

When one turn-state source intentionally renders multiple model-visible
content items, override
[`ModelVisibleContextFragment::build_many(...)`](/Users/ccunningham/code/codex-worktree-tria/codex-rs/core/src/model_visible_context.rs#L199)
instead. The default `build_many(...)` implementation just lifts
`build(...) -> Option<Self>` into `Vec<Self>`.

It receives:

- the current `TurnContext`
- `reference_context_item: Option<&TurnContextItem>`
- `TurnContextDiffParams`

`reference_context_item` is the persisted durable baseline already represented
in model-visible history. A turn-state fragment should compare against that
baseline to avoid emitting duplicate prompt state. When there is no baseline
(`None`), the fragment should decide whether to emit full current context or
fall back to `TurnContextDiffParams` for other relevant previous-turn/session
state such as `previous_turn_settings`.

Turn-state fragments return:

- `Some(fragment)` when model-visible state should be injected
- `None` when the current state is already represented and no update is needed

Fragments that are not turn-state-derived leave `build(...)` as the default
`None`.

## Initial context vs steady-state diffs

The code intentionally treats these differently.

### Full initial context

[`Codex::build_initial_context(...)`](/Users/ccunningham/code/codex-worktree-tria/codex-rs/core/src/codex.rs#L3342)
always builds turn-state fragments with `reference_context_item = None`. In
other words, it asks every turn-state fragment for the full current prompt
state, not a diff.

That is the right behavior for:

- first real user turn in a thread
- any path that must re-establish canonical prompt context from scratch

There is also
[`build_initial_context_without_reference_context_item(...)`](/Users/ccunningham/code/codex-worktree-tria/codex-rs/core/src/codex.rs#L3445),
which currently delegates to the same full-context behavior and exists to make
the “ignore any existing baseline” intent explicit in compaction rebuild paths.

### Steady-state turn updates

[`build_settings_update_items(...)`](/Users/ccunningham/code/codex-worktree-tria/codex-rs/core/src/context_manager/updates.rs#L105)
passes the current `reference_context_item` into the registry-driven turn-state
builder loop, so registered turn-state fragments can emit only the minimal
developer/contextual-user diffs for the next turn.

### Normal runtime path

[`record_context_updates_and_set_reference_context_item(...)`](/Users/ccunningham/code/codex-worktree-tria/codex-rs/core/src/codex.rs#L3479)
uses this rule:

- if there is no baseline, inject full initial context
- otherwise inject only turn-state diffs

After each real user turn it persists the latest `TurnContextItem` and advances
the in-memory baseline even if no model-visible diff message was emitted. That
is what keeps later diffing aligned with current state.

## Contextual-user detection

Contextual-user fragments share the `user` role with real user messages, so
history parsing must be able to distinguish:

- contextual state
- true user intent

That is why contextual-user fragments need stable detection.

Preferred path:

- implement `contextual_user_markers()` when the fragment has stable fixed
  wrappers

Fallback path:

- override `matches_contextual_user_text()` when detection is genuinely custom
  (for example the current AGENTS.md wrapper)

The shared detection entrypoint is
[`is_contextual_user_fragment(...)`](/Users/ccunningham/code/codex-worktree-tria/codex-rs/core/src/model_visible_fragments.rs#L843).
It first checks the current registry, then applies a very small legacy shim for
historical user-role wrappers that were shipped previously but are no longer
current fragment types.

Current legacy shim:

- old user-role `<subagent_notification>...</subagent_notification>` messages

Those legacy checks should stay bounded and local to detection. Do not create
fake current fragment types just to recognize old persisted history.

## Current fragment inventory

### Registered turn-state developer fragments

These implement `build(...)` and participate in both full initial context and
steady-state diffs:

- `ModelInstructionsUpdateFragment`
- `PermissionsUpdateFragment`
- `CustomDeveloperInstructionsUpdateFragment`
- `CollaborationModeUpdateFragment`
- `RealtimeUpdateFragment`
- `PersonalityUpdateFragment`

### Registered runtime developer fragments

These are typed and registered, but not built from `TurnContext` diffs:

- `SubagentRosterContext`
- `SubagentNotification`

### Registered turn-state contextual-user fragments

These implement `build(...)` or `build_many(...)` and participate in both full
initial context and steady-state diffs:

- `UserInstructionsFragment`
- `AgentsMdInstructions`
- `JsReplInstructionsFragment`
- `SkillsSectionFragment`
- `ChildAgentsInstructionsFragment`
- `EnvironmentContext`
- `EphemeralContextFragment`

Some of these are true steady-state diff fragments (`UserInstructionsFragment`,
`AgentsMdInstructions`, `EnvironmentContext`). `EphemeralContextFragment`
intentionally uses `build_many(...)` because one turn can inject multiple
ephemeral context content items. Others intentionally rebuild only when there
is no baseline and therefore behave as initial-context fragments expressed
through the same `build(...)` hook (`JsReplInstructionsFragment`,
`SkillsSectionFragment`, `ChildAgentsInstructionsFragment`).

### Registered runtime contextual-user fragments

These are typed and registered for rendering/detection, but not built from
`TurnContext` diffs:

- `SkillInstructions`
- `PluginInstructions`
- `UserShellCommandFragment`
- `TurnAbortedMarker`

`PluginInstructions` is currently appended to the initial contextual-user
envelope during full initial-context assembly. `SkillInstructions`,
`UserShellCommandFragment`, and `TurnAbortedMarker` are currently emitted as
standalone contextual-user messages.

## Current non-registry developer text injections

The current codebase still has a few developer-role prompt injections that are
not dedicated registered fragments. These are accurate descriptions of current
behavior, not the preferred path for new work.

Current examples include:

- memory-tool initial developer instructions
- apps guidance in the initial developer envelope
- commit-attribution guidance in the initial developer envelope
- explicit plugin-mention developer hints
- a few one-off runtime developer messages such as stop-hook and hook-provided
  additional context

These are acceptable because they are developer-role only and do not need
contextual-user detection or turn-state diff reconstruction. But for new
model-visible context, prefer a dedicated typed fragment unless the message is
truly an isolated one-off developer event.

## Choosing the right representation

### Use a registered turn-state fragment when:

- the context is derived from `TurnContext` / durable session state
- resume, fork, compaction, or backtracking should be able to rebuild it
- it belongs in one of the two turn-state envelopes

Examples:

- permissions policy
- collaboration mode
- realtime start/end state
- custom user instructions
- AGENTS.md instructions
- JS REPL guidance
- skills catalog guidance
- child-AGENTS guidance
- environment context

### Use a registered runtime fragment when:

- the context is model-visible
- it is not derived from `TurnContext` diffs
- it still benefits from standardized rendering and, for contextual-user
  fragments, shared detection

These fragments may either:

- be appended to an envelope in a non-diff initial-context path
- or be emitted as standalone runtime/session-prefix messages

Examples:

- turn-aborted marker
- user shell command record
- skill injection
- plugin session instructions
- subagent roster / subagent notification

### Use a plain developer text wrapper only when:

- the message is developer-role only
- it is a narrow one-off or initial-only addition
- it does not need contextual-user detection
- it does not need to participate in turn-state diff reconstruction

If you are tempted to use a plain `String` or `DeveloperTextFragment`, stop and
verify that this is really just an isolated developer-only event and not new
shared prompt state.

## Environment context rule

Use `<environment_context>` only for environment facts derived from
`TurnContext` that may need turn-to-turn refresh semantics.

Current fields are:

- `cwd`
- `shell`
- optional `current_date`
- optional `timezone`
- optional network allow/deny summaries

Do not put other guidance in `<environment_context>`, especially:

- permissions/policy instructions
- skills or plugins
- AGENTS instructions
- subagent notices

Those should stay in their own fragment types.

## Blessed path for new work

When adding new model-visible context:

1. Decide whether the model should see it at all.
2. Decide whether it is durable turn/session state or a one-off runtime event.
3. If it should be represented as a named fragment, define a typed fragment in
   [`model_visible_fragments.rs`](/Users/ccunningham/code/codex-worktree-tria/codex-rs/core/src/model_visible_fragments.rs).
4. Implement `ModelVisibleContextFragment`.
5. Set `type Role` correctly.
6. If it is turn-state context, implement `build(...)`.
   Override `build_many(...)` instead when one turn-state source should emit
   multiple content items.
7. If it is contextual-user, provide stable detection with
   `contextual_user_markers()` or custom `matches_contextual_user_text()`.
8. Register it exactly once in `REGISTERED_MODEL_VISIBLE_FRAGMENTS`, in prompt
   order.
9. Use the shared envelope builders or fragment conversions rather than
   hand-constructing model-visible `ResponseItem::Message` payloads in new
   turn-state paths.

Rule of thumb:

- “This is durable prompt state” => registered typed fragment, usually with
  `build(...)` or, for zero-or-many cases, `build_many(...)`
- “This is a one-off contextual/runtime marker” => registered typed fragment,
  usually without `build(...)`
- “This is an isolated developer-only text event” => plain developer text is
  sometimes acceptable, but it is the exception, not the default
