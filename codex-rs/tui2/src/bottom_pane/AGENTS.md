# TUI2 bottom pane (state machines)

When changing the paste-burst or chat-composer state machines in this folder, keep the docs in sync:

- Update the relevant module docs (`chat_composer.rs` and/or `paste_burst.rs`) so they remain a
  readable, top-down explanation of the current behavior.
- Update the narrative doc `docs/tui-chat-composer.md` whenever behavior/assumptions change (Enter
  handling, retro-capture, flush/clear rules, `disable_paste_burst`, non-ASCII/IME handling).
- Keep `codex-rs/tui` and `codex-rs/tui2` implementations/docstrings aligned unless the divergence
  is intentional and documented.

Practical check:

- After edits, sanity-check that docs mention only APIs/behavior that exist in code (especially the
  Enter/newline paths and `disable_paste_burst` semantics).
