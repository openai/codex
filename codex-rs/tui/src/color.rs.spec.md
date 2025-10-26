## Overview
`color` provides small color-utility helpers for the TUI. It determines perceived brightness, blends colors, and computes perceptual distance to choose contrasting palettes.

## Detailed Behavior
- `is_light(bg)` calculates luminance using the standard weighted RGB formula and returns `true` when the background is light.
- `blend(fg, bg, alpha)` linearly interpolates between foreground/background colors using an alpha factor.
- `perceptual_distance(a, b)` converts sRGB values to Lab space (via XYZ) and computes Euclidean distance (CIE76). Used to compare palette choices.

## Broader Context
- Utilities support theming decisions in modules like `style`, `terminal_palette`, and status widgets.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./style.rs.spec.md
  - ./terminal_palette.rs.spec.md
