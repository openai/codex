## Overview
`codex-tui::onboarding::windows` shows WSL setup guidance to Windows users during onboarding. It highlights prerequisites, provides instructions, and records whether the user opted to install WSL before proceeding.

## Detailed Behavior
- `WindowsSetupWidget` renders:
  - Explanation of why WSL is required for Codex CLI.
  - Step-by-step instructions (`WSL_INSTRUCTIONS`) and a prompt to acknowledge.
  - Buttons/options to “Install WSL now” vs “Skip for now”.
- Interaction:
  - Handles key events to select and confirm installation vs skipping.
  - On confirmation, writes a flag indicating the user chose to install WSL (persisted by the caller) or opted to skip.
- Result:
  - Onboarding captures `windows_install_selected` and, if true, the main flow restores the terminal, prints WSL instructions to stdout, and exits so the user can run setup commands immediately.

## Broader Context
- Displayed when `run_ratatui_app` detects Windows platform and `windows_wsl_setup_acknowledged` is false in config.
- Works in tandem with onboarding’s config reload: after the user acknowledges (or chooses to trust), subsequent runs can skip the screen.
- Context can't yet be determined for additional Windows setup steps; the widget can be expanded for future requirements.

## Technical Debt
- None noted; functionality is straightforward.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./mod.rs.spec.md
  - ./onboarding_screen.rs.spec.md
