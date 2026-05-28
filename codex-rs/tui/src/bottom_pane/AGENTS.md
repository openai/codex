# TUI bottom pane (state machines)

When changing the paste-burst or chat-composer state machines in this folder, keep the docs in sync:

- Update the relevant module docs (`chat_composer.rs` and/or `paste_burst.rs`) so they remain a
  readable, top-down explanation of the current behavior.
- Keep implementations/docstrings aligned unless a divergence is intentional and documented.

Practical check:

- After edits, sanity-check that docs mention only APIs/behavior that exist in code (especially the
  Enter/newline paths and `disable_paste_burst` semantics).
