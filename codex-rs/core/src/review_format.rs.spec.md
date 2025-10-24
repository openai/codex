## Overview
`core::review_format` renders review findings into plain-text blocks suitable for notifications or CLI output. It keeps formatting UI-agnostic so downstream surfaces can add styling without rewriting layout logic.

## Detailed Behavior
- `format_location` composes `<path>:<start>-<end>` using the `ReviewFinding` code location.
- `format_review_findings_block` builds a multi-line string:
  - Adds a blank line and a header (`Review comment:` or `Full review comments:` based on item count).
  - For each finding, inserts a blank line, a bullet (or checkbox if a selection mask is provided), the title/location, and then indents each line of the body by two spaces.
  - When `selection` is present, indices beyond the mask default to selected (`[x]`).
- The output is intentionally plain text so CLI, TUI, or notifier integrations can handle coloring/styling separately.

## Broader Context
- Review-mode tasks (`tasks/review.rs`) format findings before surfacing them to the user or sending notifications (e.g., `user_notification`). Tests for reviewers rely on this formatting to validate UX.
- Context can't yet be determined for localization (e.g., alternative headers); future expansions might parameterize copy or integrate with templating.

## Technical Debt
- Formatting rules are hard-coded; if other surfaces need markdown/HTML versions, extracting a structured representation or providing customizable templates would reduce duplication.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Expose a structured representation (e.g., preformatted sections) so different renderers can format review findings without reimplementing the transformation.
related_specs:
  - ./tasks/review.rs.spec.md
  - ./user_notification.rs.spec.md
