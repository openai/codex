# Follow-Up Rendering Bug (UI)

We found that delegate follow-up prompts disappear when reopening the saved session even though the
run’s rollout contains them. Root cause:

- `App::activate_delegate_session` only calls `ChatWidget::hydrate_from_shadow` when
  `SessionHandle::history().is_empty()`.
- On follow-up, the handle still has prior history, so hydration is skipped and the old transcript
  remains.

## Next Steps
1. Adjust `activate_delegate_session` to refresh from the shadow snapshot even when history already
   exists (e.g., clear/replace history or always hydrate).
2. Add a regression test that currently fails: create a `SessionHandle` with non-empty history, mock
   an `ActiveDelegateSession` snapshot containing follow-up user and agent messages, invoke
   `activate_delegate_session`, and assert the transcript now includes the follow-up prompt.
3. After the fix, run `cargo test -p codex-tui` to confirm coverage.

## Useful References
- Rollout file with the missing follow-up:
  `ai-temp/example-codex-home/agents/critic/sessions/2025/10/21/rollout-2025-10-21T00-33-16-019a042f-3e55-7171-b9df-1690b9f905a0.jsonl`
- Entry points:
  - `codex-rs/tui/src/app.rs::activate_delegate_session`
  - `codex-rs/tui/src/chatwidget.rs::hydrate_from_shadow`
  - Shadow capture logic in `codex-rs/multi-agent/src/shadow/{mod,recorder}.rs`
