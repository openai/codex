# Headers, primary, and secondary text

- **Headers:** Use `bold`. For markdown with various header levels, leave in the `#` signs.
- **Primary text:** Default.
- **Secondary text:** Use `dim`.

# Foreground colors

- **Default:** Most of the time, just use the default foreground color. `reset` can help get it back.
- **Selection:** Use ANSI `blue`. (Ed & AE want to make this cyan too, but we'll do that in a followup since it's riskier in different themes.)
- **User input tips and status indicators:** Use ANSI `cyan`.
- **Success and additions:** Use ANSI `green`.
- **Errors, failures and deletions:** Use ANSI `red`.
- **Codex:** Use ANSI `magenta`.

# Avoid

- Avoid custom colors because there's no guarantee that they'll contrast well or look good on various terminal color themes.
- Avoid ANSI `black`, `white`, `yellow` as foreground colors because the terminal theme will do a better job. (Use `reset` if you need to in order to get those.) The exception is if you need contrast rendering over a manually colored background.

(There are some rules to try to catch this in `clippy.toml`.)
