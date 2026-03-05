# Model-visible context fragments

Codex injects model-visible context through two envelopes:

- the developer envelope, rendered as a single `developer` message
- the contextual-user envelope, rendered as a single `user` message whose contents are contextual state rather than real user intent

Both envelopes use the same internal fragment contract in `codex-rs/core`.

## Blessed path

When adding new model-visible context:

1. Define a typed fragment type.
2. Implement `ModelVisibleContextFragment` for it.
3. Set the fragment `type Role` to the correct developer or contextual-user role.
4. Give it a `ModelVisibleContextEnvelope` with the right marker behavior.
5. If the fragment is derived from `TurnContext` and participates in turn-to-turn diffing, also implement `TurnBackedContextFragment`.
6. Push the fragment through the shared envelope builders in initial-context or settings-update assembly.

Do not hand-build developer or contextual-user `ResponseItem`s in new code unless there is a strong reason to bypass the fragment path.

The role lives in the fragment's associated `type Role`. `ModelVisibleContextEnvelope` only carries marker/tag metadata.

## Choosing an envelope

Use the developer envelope for developer-role guidance:

- permissions / approval policy instructions
- collaboration-mode developer guidance
- model switch and realtime notices
- personality guidance
- subagent roster and subagent notifications
- other developer-only instructions

`DeveloperInstructions` remains the standard string-backed fragment for most developer text. It already participates in the shared fragment system.

Use the contextual-user envelope for contextual state or runtime markers that should not count as real user turns:

- AGENTS / user instructions
- plugin instructions
- environment context
- skill instructions
- user shell command records
- turn-aborted markers

Contextual-user fragments must have stable markers because history parsing uses those markers to distinguish contextual state from real user intent.

## Turn-backed fragments

If a fragment is derived from durable turn/session state, keep its extraction, diffing, and rendering logic together by implementing `TurnBackedContextFragment`.

That trait is the blessed path for fragments that need to:

- build full initial context from the current turn state
- compute settings-update diffs from persisted previous state to current turn state

`EnvironmentContext` is the canonical example. Future turn-backed contextual fragments should follow the same pattern instead of introducing one-off extraction or diff helpers.

## History behavior

Developer fragments do not need contextual-user marker matching because they are already separable by message role.

Contextual-user fragments do need marker matching because they share the `user` role with real user turns, and history parsing / truncation must avoid treating injected context as actual user input.
