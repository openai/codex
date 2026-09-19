# Headers, primary, and secondary text

- **Headers:** Use `bold`. For markdown with various header levels, leave in the `#` signs.
- **Primary text:** Default.
- **Secondary text:** Use `style::secondary_text_style()` for shared footer descriptions and
  status lines. It resolves a readable muted foreground when the terminal palette is known
  and clears inherited dim/bold modifiers. Other secondary surfaces may still use `dim`.

- **Keyboard hints:** Use `key_hint` span helpers for compact, uniformly bold shortcut labels
  and readable secondary descriptions. Include `+`, `/`, and chord separators in the label's
  emphasis. The `?` shortcut reference uses readable accent-colored keys
  without bold. Separate composer footer actions with ` · `.

# Foreground colors

- **Default:** Most of the time, just use the default foreground color. `reset` can help get it back.
- **Active controls and input emphasis:** Use `style::accent_style()` or
  `style::accent_color_on(background)` for text on a painted surface.
- **Semantic statuses:** Use `style::status_style(StatusTone)`; terminal-owned ANSI
  colors remain the fallback for limited palettes.
- **Success and additions:** Use ANSI `green`.
- **Errors, failures and deletions:** Use ANSI `red`.
- **Codex:** Use ANSI `magenta`.

# Avoid

- Avoid unchecked custom foreground colors. Use `style::readable_color_on(color, background)`
  for theme-derived text, passing the background actually painted behind it. `None` means
  the terminal background; color reduction is included in the contrast check. When the
  background is unknown, supported theme colors are preserved rather than guessed.
  Generic code renderers preserve configured syntax foregrounds; diff rendering resolves
  them against its add/delete fills.
- Avoid ANSI `black` & `white` as foreground colors because the default terminal theme color will do a better job. (Use `reset` if you need to in order to get those.) The exception is if you need contrast rendering over a manually colored background.
- Avoid ANSI `blue` and `yellow` because for now the style guide doesn't use them. Prefer a foreground color mentioned above.

(There are some rules to try to catch this in `clippy.toml`.)
