## Overview
`codex-tui::onboarding::onboarding_screen` implements the interactive onboarding flow. It sequences step widgets (Windows setup, welcome, login selection, trust directory) and handles keyboard/paste events until the user completes the onboarding wizard.

## Detailed Behavior
- `OnboardingScreenArgs` informs which steps to include (based on config flags), carries login status, auth manager, and the current `Config`.
- `OnboardingScreen::new` builds a vector of `Step` enums:
  - `Windows`: `WindowsSetupWidget` prompting WSL installation when relevant.
  - `Welcome`: `WelcomeWidget` introduction (optionally showing “already logged in” hints).
  - `Auth`: `AuthModeWidget` allowing ChatGPT or API key login (respecting forced login methods).
  - `TrustDirectory`: `TrustDirectoryWidget` to confirm trusting the current workspace (preselects trust when repo is under Git).
- Step management:
  - `current_steps` and `current_steps_mut` iterate over steps based on `StepState` (`Hidden`, `InProgress`, `Complete`) so only the active step receives events/updates.
  - Tracks completion via `is_done`, `windows_install_selected`, and `directory_trust_decision`.
- Event handling:
  - Implements `KeyboardHandler` for key events/paste, forwarding to active step widget (which updates its internal state).
  - `render` draws each visible step, clearing the screen when needed.
  - `run_onboarding_app` (later in file) runs a mini event loop using `Tui::event_stream`, handling `TuiEvent`s until `OnboardingScreen::is_done()` or the user exits.
- Result:
  - Returns `OnboardingResult` containing `directory_trust_decision` and whether the user opted to install WSL. Callers use this to reload config or exit with instructions.

## Broader Context
- `run_ratatui_app` invokes `run_onboarding_app` before launching the main chat interface. Step widgets reside in `auth.rs`, `trust_directory.rs`, `windows.rs`, and `welcome.rs`, allowing focused rendering/logic per step.
- The onboarding flow integrates with `AuthManager` to initiate login flows and uses the frame requester for redraws.
- Context can't yet be determined for additional onboarding steps; the `Step` enum provides a straightforward place to add new experiences.

## Technical Debt
- Step orchestration mixes state logic with widget instantiation. Encapsulating steps behind a trait or state machine would simplify addition/removal of steps.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Introduce a trait-based step interface so onboarding progression is less ad-hoc and easier to extend.
related_specs:
  - ./mod.rs.spec.md
  - ./auth.rs.spec.md
  - ./trust_directory.rs.spec.md
