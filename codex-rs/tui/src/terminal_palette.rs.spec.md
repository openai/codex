## Overview
Utilities for working with terminal color capabilities. Determines the best approximations for RGB colors based on the terminal’s support level, caches default foreground/background colors by querying OSC codes, and exposes helpers used by styling modules.

## Detailed Behavior
- `best_color(target)`:
  - Uses `supports_color::on_cached` to inspect the current stdout capabilities.
  - If 16 M colors are available, returns `Color::Rgb(target)`.
  - If only 256-color mode is supported, searches the fixed xterm palette (indices ≥16) for the closest perceptual match via `color::perceptual_distance`, returning `Color::Indexed`.
  - Otherwise falls back to the default color.
- Default color cache:
  - `DefaultColors` holds foreground/background tuples.
  - `default_colors` delegates to `imp::default_colors`, which (on Unix) caches OSC response parsing results in a `Mutex<Cache<DefaultColors>>`; non-Unix builds return `None`.
  - `requery_default_colors` refreshes the cache.
  - `default_fg` / `default_bg` expose components.
- Unix implementation details:
  - Sends OSC 10/11 queries (`ESC]10;?\a` etc.), reads `/dev/tty` in non-blocking mode, and parses responses like `ESC]10;rgb:RR/GG/BB` into 8-bit RGB.
  - `parse_component` converts hex color components of varying bit lengths to 0–255.
  - Timeouts avoid blocking if the terminal does not respond.
- Xterm palette:
  - `xterm_fixed_colors` iterates over a table of 256 RGB values (skip first 16 theme-dependent entries) used to approximate colors under 256-color terminals.
  - `XTERM_COLORS` constants provide the actual RGB triplets.

## Broader Context
- `style.rs` and `shimmer.rs` depend on these helpers to pick backgrounds that blend with terminal themes.
- Provides a centralized color capability abstraction so widgets avoid embedding terminal detection logic.

## Technical Debt
- OSC queries rely on `/dev/tty` and may fail on systems without it (e.g., when stdout is piped). Capturing fallbacks or logging warnings could improve diagnostics.
- The 200 ms timeout is heuristic; exposing configuration or adaptive retries might help on slow terminals.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Provide better error reporting or logging when OSC queries fail so developers understand why default colors are missing.
related_specs:
  - style.rs.spec.md
  - shimmer.rs.spec.md
