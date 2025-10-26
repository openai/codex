## Overview
Generates animated shimmer spans for loading indicators and placeholders. The effect sweeps a highlight across text by blending terminal palette colors over time, falling back to modifier-based emphasis when true-color output is unavailable.

## Detailed Behavior
- `PROCESS_START` is a `OnceLock<Instant>` capturing the process start (first call) so all shimmer animations stay phase-aligned.
- `elapsed_since_start` returns `Duration` since that instant, used to drive animation.
- `shimmer_spans(text)`:
  - Splits `text` into characters and bails out for empty input.
  - Defines a sweep `period` covering the string plus padding at both ends; normalizes elapsed seconds into a position along the band every ~2 seconds.
  - Detects true-color support via `supports_color`; sets `band_half_width` to control gradient width.
  - For each character, computes distance from the sweep center and derives an intensity `t` using a cosine transition to keep edges smooth.
  - When true color is available, blends the default foreground (`terminal_palette::default_fg`) toward the default background for highlight intensity, applying bold. Without RGB support, it chooses modifiers (dim/normal/bold) to approximate the effect.
  - Returns `Vec<Span<'static>>` styled per character.
- `color_for_level` encapsulates the fallback style mapping by intensity threshold.

## Broader Context
- Used by async-loading widgets (palette, project scanning, history fetchers) to display animated placeholder banners consistent with the rest of the TUI palette.
- Depends on `terminal_palette` values so the shimmer respects user-selected themes or computed foreground/background colors.

## Technical Debt
- Animation period and band width are hard-coded; hooks for customization could let different widgets adjust speed or emphasis.
- Per-character allocation may be heavy for long strings; caching rendered spans or using a streaming interface might reduce churn.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Expose configuration knobs for sweep duration and band size to tailor animations per widget.
related_specs:
  - terminal_palette.rs.spec.md
  - color.rs.spec.md
