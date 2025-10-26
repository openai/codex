## Overview
`feedback_view` presents a modal asking whether to upload Codex logs before filing a bug report. It uses `ListSelectionView` to render the options and inserts follow-up history cells with instructions or error messages.

## Detailed Behavior
- `FeedbackView::show` constructs a `SelectionViewParams` with a `FeedbackHeader` explaining log upload implications and paths.
- Actions:
  - “Yes” uploads the log snapshot via `CodexLogSnapshot::upload_to_sentry`, reporting success or failure in the transcript and providing a pre-filled GitHub issue URL.
  - “No” emits history lines directing the user to open an issue manually, including thread ID.
  - “Cancel” dismisses the dialog without side effects.
- `FeedbackHeader` implements `Renderable`, formatting multi-line instructions and the log file path.

## Broader Context
- Triggered by the `/feedback` slash command so users can share diagnostics with the Codex team.

## Technical Debt
- None; the modal delegates to `ListSelectionView` for navigation and cleanly encapsulates upload logic.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./list_selection_view.rs.spec.md
  - ../chatwidget.rs.spec.md
