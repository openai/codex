## Overview
`codex-tui::onboarding::auth` handles login during onboarding. It renders the authentication step, tracks login state (ChatGPT, API key, device code), and surfaces errors or status messages while the login flow runs.

## Detailed Behavior
- Core structs:
  - `AuthModeWidget`: Ratatui widget for selecting login method, initiating login server/device flows, and updating the onboarding state.
  - `SignInState`: shared state (via `Arc<RwLock<_>>`) representing the current stage (pick mode, waiting for browser/device flow, success/failure).
- Functionality:
  - Displays login options, highlighting the forced method when configured (e.g., forced API login disables ChatGPT option).
  - Initiates `run_login_server` (browser login) or `run_device_code_login` asynchronously; updates `SignInState` with progress/errors.
  - Tracks existing login status (`LoginStatus`) to shortcut if already authenticated.
- Rendering:
  - Renders status messages (waiting on browser/device code, success, errors) with color cues.
  - Provides key handling for selecting modes and canceling operations.
- Interaction:
  - Notifies the onboarding screen to continue when login is complete or skip when forced login prevents selection.
  - Ensures frame updates via `FrameRequester`.

## Broader Context
- Called from `OnboardingScreen::new` when `show_login_screen` is true. Integrates with `AuthManager` to persist login tokens and respects forced login configuration flags (`ForcedLoginMethod`).
- Shares style conventions with other onboarding widgets (welcome, trust directory).
- Context can't yet be determined for additional auth providers; existing structure accommodates additional modes via new `SignInState` variants.

## Technical Debt
- `AuthModeWidget` mixes UI rendering with async login orchestration; refactoring into a controller + view would separate concerns and aid testing.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Extract login orchestration into a dedicated async task/controller so the widget focuses on rendering.
related_specs:
  - ./onboarding_screen.rs.spec.md
  - ./mod.rs.spec.md
