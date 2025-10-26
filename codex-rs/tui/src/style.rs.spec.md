## Overview
Defines styling helpers for user-authored messages, adapting background colors to the current terminal palette so user bubbles remain readable against light or dark themes.

## Detailed Behavior
- `user_message_style` fetches the terminal’s default background via `terminal_palette::default_bg` and delegates to `user_message_style_for`.
- `user_message_style_for` returns a default style when the background is unknown; otherwise sets the background to `user_message_bg`.
- `user_message_bg` chooses a base color (`black` for light backgrounds, `white` for dark backgrounds), blends it 10 % toward the terminal background using `color::blend`, and wraps the result in `ratatui::Color::Rgb`. The helper tolerates Clippy’s disallowed method lint because `Color::Rgb` is the intended output.

## Broader Context
- Used when rendering user messages in history cells (`history_cell::PlainHistoryCell`) and transcript overlays so user turns stand out while respecting terminal themes.
- Relies on shared color utilities (`is_light`, `best_color`) to pick accessible shades across different palettes.

## Technical Debt
- Only handles user message background; other style aspects (foreground text, borders) remain fixed. Incorporating contrast calculations for text colors could improve accessibility.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Evaluate contrast ratios for the chosen backgrounds and adjust or invert text color when needed for accessibility.
related_specs:
  - terminal_palette.rs.spec.md
  - history_cell.rs.spec.md
