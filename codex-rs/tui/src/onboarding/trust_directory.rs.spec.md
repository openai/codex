## Overview
`codex-tui::onboarding::trust_directory` presents the “Trust this directory?” step during onboarding. It explains why Codex needs trust (sandbox permissions) and records the user’s decision to trust or skip the workspace.

## Detailed Behavior
- `TrustDirectoryWidget` stores:
  - Current directory, Codex home path, whether it’s a Git repo, highlighted selection, and any validation error.
  - Optional `selection` (final decision) updated when the user confirms.
- Rendering:
  - Displays context about the workspace (path, Git status) and prompts the user to choose between trusting or not trusting.
  - Highlights the default (trust vs don’t trust) based on Git presence.
- Interaction:
  - Handles arrow keys/enter to toggle/confirm selection.
  - If the workspace is not a Git repo, default highlight is “Don’t trust”.
  - Reports completion state so `OnboardingScreen` can advance.
- Output:
  - `TrustDirectorySelection` enum captures `Trust` vs `DontTrust`.
  - On completion, the selection is returned via `OnboardingScreen::directory_trust_decision`, enabling the caller to mark the directory as trusted in config if requested.

## Broader Context
- Onboarding uses this widget only when the user hasn’t already trusted the directory. If the user chooses to trust, the main flow reloads config to persist the change (`set_hide_full_access_warning`, etc.).
- Provides additional safety prompts for non-Git directories to encourage caution.
- Context can't yet be determined for multi-directory prompts; current design focuses on a single workspace.

## Technical Debt
- None significant; the widget is self-contained.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./onboarding_screen.rs.spec.md
  - ./mod.rs.spec.md
