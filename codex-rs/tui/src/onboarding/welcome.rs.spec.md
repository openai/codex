## Overview
Renders the onboarding welcome step and drives the background ASCII animation. The widget shows a looping animation (when space allows) plus a welcome banner. It also exposes a keyboard handler so developers can shuffle animation variants with `Ctrl + .`.

## Detailed Behavior
- `WelcomeWidget` stores whether the user is logged in (used to signal onboarding state) and an `AsciiAnimation`.
- `new` wires the widget to a `FrameRequester` so the animation can request redraws.
- `WidgetRef` impl:
  - Clears the render area, schedules the animation’s next frame, and skips drawing when the viewport is smaller than `MIN_ANIMATION_HEIGHT`×`MIN_ANIMATION_WIDTH`.
  - When large enough, it blits the current animation frame, adds a blank line, and appends a welcome sentence with `Codex` bolded.
  - Wraps the output without trimming spaces to preserve animation alignment.
- `KeyboardHandler` listens for `Ctrl + .` key presses (press or repeat) and asks the animation to pick a random variant, logging a warning for easier diagnostics.
- `StepStateProvider` returns `Hidden` when the user is already logged in and `Complete` otherwise, allowing onboarding flows to skip or advance past the welcome screen.
- Tests confirm that:
  - The first render produces animation output across the configured area.
  - `Ctrl + .` switches to a different animation frame when multiple variants exist.

## Broader Context
- Integrated into the onboarding flow defined in `onboarding_screen.rs`, which coordinates steps like login, tutorial, and feature highlights.
- Reuses the shared `AsciiAnimation` infrastructure so the welcome screen’s visuals follow the same scheduling and resource loading conventions as other animated widgets.

## Technical Debt
- The min-size guards and animation scheduling rely on heuristics; the widget does not adapt content for smaller terminals beyond hiding the animation, potentially leaving large empty areas.
- Keyboard handler only recognizes `Ctrl + .`; providing a menu option or on-screen hint could improve discoverability.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Add alternative cues or menu actions to surface animation variant switching to users.
related_specs:
  - onboarding/onboarding_screen.rs.spec.md
  - ascii_animation.rs.spec.md
