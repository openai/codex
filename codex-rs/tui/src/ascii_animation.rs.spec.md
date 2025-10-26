## Overview
`ascii_animation` schedules and renders small looping ASCII animations used by onboarding popups and other TUI widgets.

## Detailed Behavior
- `AsciiAnimation` owns a `FrameRequester`, a set of animation variants (each a slice of frames), the active variant index, frame duration, and start timestamp.
- `new` initializes with the default animation set (`frames::ALL_VARIANTS`) and default tick (`FRAME_TICK_DEFAULT`).
- `schedule_next_frame` computes the remaining time in the current frame cycle and uses `FrameRequester::schedule_frame_in` to request a redraw; zero-duration ticks fall back to immediate scheduling.
- `current_frame` calculates the frame index based on elapsed milliseconds and returns the corresponding ASCII art string; empty variants yield an empty frame.
- `pick_random_variant` chooses a different animation variant (if multiple exist) using `rand::rng()`, scheduling an immediate frame to reflect the change.
- Exposes helper `request_frame` (direct schedule) and `frames` accessor for unit tests.

## Broader Context
- Widgets in `bottom_pane` and onboarding overlays embed `AsciiAnimation` to show activity indicators without reimplementing timing logic.

## Technical Debt
- None; module is self-contained and parameterized by `FrameRequester`, so additional animations can reuse it without changes.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./frames.rs.spec.md
  - ./tui.rs.spec.md
