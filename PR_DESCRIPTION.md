# PR Title
TUI: support slash command args (starting with /plan)

# Summary
- Add a reusable slash-command args UI config (arg hints + inline element behavior).
- Implement /plan arg handling: popup shows [prompt], selecting /plan inserts the inline element, and /plan <prompt> submits in plan mode.
- Highlight typed /plan as soon as it has a trailing space, including after paste-burst flushes.

# Testing
- just fmt
- cargo test -p codex-tui
