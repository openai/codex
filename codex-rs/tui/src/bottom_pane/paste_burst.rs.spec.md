## Overview
`paste_burst` detects rapid-fire character input that likely originates from clipboard pastes, buffering the text so it appears atomically instead of flickering character-by-character. It also guards against accidental newline submission during bursts.

## Detailed Behavior
- Constants define thresholds for burst detection (`PASTE_BURST_MIN_CHARS`, inter-char interval, and suppress window).
- `PasteBurst` tracks timing (`last_plain_char_time`), burst counters, buffered text, pending first characters, and active windows.
- Decision flow:
  - `on_plain_char(ch, now)` updates burst counters and returns a `CharDecision` telling the caller to begin buffering (with retro grab or from pending char), append to the buffer, or retain the first char temporarily.
  - `flush_if_due(now)` emits buffered paste text, flushes held single characters, or returns `None` when still waiting.
  - `append_newline_if_active(now)` treats newline as buffered text during an active burst.
  - `newline_should_insert_instead_of_submit(now)` indicates whether Enter should insert a newline rather than submitting (active burst or within suppress window).
  - `extend_window`, `begin_with_retro_grabbed`, `append_char`, `drain_buffer` support the composer’s timing loop.
- `FlushResult` distinguishes between pasted strings, typed characters, and no-op flushes, while `CharDecision::BeginBuffer { retro_chars }` lets the caller retroactively capture already-inserted characters.

## Broader Context
- `ChatComposer` consults `PasteBurst` when processing key events and scheduling redraws, ensuring pastes appear smoothly and preserving multi-line paste semantics.

## Technical Debt
- Heuristics are tuned manually; consider making thresholds configurable or telemetry-driven if paste detection needs to adapt to varied environments.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Explore adaptive thresholds or logging to refine paste detection across different terminal speeds.
related_specs:
  - ./chat_composer.rs.spec.md
