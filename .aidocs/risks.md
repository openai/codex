# Design Risks

- The acceptance boolean returned by `dispatch_command` and `dispatch_command_with_args` is now a behavioral contract, but Rust's type system does not encode why a branch returned `true` or `false`. A future command branch can accidentally record a rejected command if it returns `true` after showing an error or no-op message. Keep the contract documented on the dispatch methods and prefer focused tests for commands with unusual rejection paths.

- Pending slash-command history is a single slot on the composer. That is appropriate for synchronous input-result dispatch, but it relies on every dispatch wrapper resolving the slot immediately. If a future command path defers acceptance asynchronously, it should either keep the current synchronous acceptance decision or introduce an explicit pending-command token so one staged entry cannot be overwritten by a later input.

- The branch intentionally limits slash-command recall to local in-session history. Users may still expect accepted commands to appear in cross-session history because normal prompts do. If product direction changes, the persistence boundary should be revisited in the history layer rather than by leaking slash-command handling into persistent storage ad hoc.

- Popup-selected commands record the canonical command name instead of the user's partial filter text. That is the right recall behavior for commands like `/di` -> `/diff`, but it means recall does not preserve exactly what the user typed before selecting the popup row. Keep this distinction explicit in tests so future changes do not treat it as accidental normalization.

# Contracts

- Enforced by types: local recall entries use `HistoryEntry`, so staged slash commands carry the same text-element, attachment, mention-binding, and pending-paste fields as other local submissions.

- Enforced by tests: accepted bare commands, popup-selected commands, and inline commands with arguments are recorded only after the pending entry is explicitly committed.

- Social knowledge: `ChatWidget` is the authority for whether a slash command was accepted, and every command branch must return `true` only when the command should be recallable. Document this on the dispatch helpers and add tests when commands introduce non-obvious rejection paths.

# Debug Path

Start at the key event that submits the command. For typed bare commands, inspect `ChatComposer::try_dispatch_bare_slash_command`; for typed inline commands, inspect `ChatComposer::try_dispatch_slash_command_with_args`; for popup Enter or Tab, inspect `ChatComposer::handle_key_event_with_slash_popup`. Each accepted parser path should stage pending history before clearing the textarea.

Then follow the `InputResult` match in `ChatWidget`. The slash-command wrappers should call the dispatch method, record pending history on `true`, and discard it on `false`. Finally, use Up-arrow to exercise `ChatComposerHistory::navigate_up`; if the entry is local, it should be returned without an asynchronous persistent-history lookup.
