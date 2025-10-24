## Overview
`codex-tui::onboarding` orchestrates the first-run experience: login prompts, trust-directory confirmation, Windows WSL guidance, and welcome screens. It determines which screens to show, runs the onboarding flow, and exposes utilities referenced by `run_ratatui_app`.

## Detailed Behavior
- Re-exports:
  - `TrustDirectorySelection` enum and `WSL_INSTRUCTIONS` message.
  - `onboarding_screen::run_onboarding_app` and its args struct (`OnboardingScreenArgs`).
- Submodules:
  - `auth.rs`: logic for login prompts and status display during onboarding.
  - `trust_directory.rs`: handles directory trust decisions (approve or skip).
  - `windows.rs`: surfaces WSL setup guidance for Windows users.
  - `welcome.rs`: renders the welcome card and general onboarding UI.
  - `onboarding_screen.rs`: ties these components together into a full-screen Ratatui app that collects user decisions before launching the main UI.
- Global constants provide messaging (e.g., `WSL_INSTRUCTIONS`) used when users opt into Windows setup.

## Broader Context
- `run_ratatui_app` checks `should_show_onboarding` and, if needed, calls `run_onboarding_app` to gather user input before proceeding. The onboarding screen can modify configuration (e.g., mark directory trusted) or instruct the user to install WSL, returning control to the main app loop with updated config.
- Onboarding integrates with `AuthManager` to support login from within the flow.
- Context can't yet be determined for future onboarding steps; additional modules can be layered into this structure as needed.

## Technical Debt
- None currently noted; module separation keeps onboarding logic contained.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./onboarding/onboarding_screen.rs.spec.md
  - ./lib.rs.spec.md
